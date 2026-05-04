// ---------------------------------------------------------------------------
// start_subagent — non-blocking tool that spawns a child `agent::run` loop
//
// See `crate::subagents` for the full design. This tool:
//   1. Checks depth + concurrency caps.
//   2. Generates a subagent id.
//   3. Emits `SubagentStarted` on the spawning agent's event sink.
//   4. Spawns a tokio task that runs the child agent to completion and then
//      (a) emits `SubagentCompleted` and (b) sends a `SubagentResult` back
//      on the parent's inbox channel.
//   5. Returns immediately with `{subagent_id, status: "started"}` so the
//      parent never blocks on the child.
// ---------------------------------------------------------------------------

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};
use tokio::sync::mpsc;

use crate::agent::AgentEvent;
use crate::conversation::{self, Conversation};
use crate::llm::{ChatCompletionTool, FunctionDefinition};
use crate::prompts::{MAX_CONCURRENT_SUBAGENTS_PER_PARENT, MAX_SUBAGENT_DEPTH};
use crate::subagents::{
    self, RegistryEntry, SubagentContext, SubagentResult, SubagentSender, SubagentStatus,
};

use super::{Tool, ToolError};

// ---------------------------------------------------------------------------
// Tool
// ---------------------------------------------------------------------------

pub struct StartSubagentTool {
    tx: SubagentSender,
    pending: Arc<AtomicUsize>,
    concurrent: Arc<AtomicUsize>,
    ctx: SubagentContext,
}

impl StartSubagentTool {
    pub fn new(
        tx: SubagentSender,
        pending: Arc<AtomicUsize>,
        concurrent: Arc<AtomicUsize>,
        ctx: SubagentContext,
    ) -> Self {
        Self {
            tx,
            pending,
            concurrent,
            ctx,
        }
    }
}

#[derive(Deserialize)]
struct Args {
    task: String,
    #[serde(default)]
    context: String,
}

#[async_trait]
impl Tool for StartSubagentTool {
    fn name(&self) -> &str {
        "start_subagent"
    }

    fn definition(&self) -> ChatCompletionTool {
        ChatCompletionTool::Function {
            function: FunctionDefinition {
                name: "start_subagent".to_string(),
                description: Some(
                    "Spawn a subagent to work on a focused subtask in parallel. Returns \
                     immediately with the subagent id — you are NOT blocked on the subagent. \
                     The subagent runs with the same tools and system prompt as you, but \
                     starts with a fresh context: only the `task` and `context` strings you \
                     pass it. Its final reply arrives later as a user message prefixed \
                     `SUBAGENT <id> COMPLETED: ...` (or `FAILED`). Use subagents when a \
                     subtask is self-contained, would require many tool calls whose \
                     intermediate output you don't need to see, or would otherwise bloat \
                     your context. Don't use subagents for trivial tasks — the overhead \
                     isn't worth it."
                        .to_string(),
                ),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "task": {
                            "type": "string",
                            "description": "Detailed description of the task for the subagent, including success criteria and any constraints."
                        },
                        "context": {
                            "type": "string",
                            "description": "Background and context the subagent needs to do the task (file paths, prior findings, relevant snippets). The subagent will not see your conversation history — put everything it needs here."
                        }
                    },
                    "required": ["task"],
                    "additionalProperties": false
                })),
                strict: Some(false),
            },
        }
    }

    async fn call(&self, arguments: &str) -> Result<JsonValue, ToolError> {
        let args: Args =
            serde_json::from_str(arguments).map_err(|e| ToolError(format!("bad args: {e}")))?;

        // --- Caps -----------------------------------------------------------
        if self.ctx.depth >= MAX_SUBAGENT_DEPTH {
            return Ok(json!({
                "status": "rejected",
                "error": format!(
                    "max subagent depth ({}) reached — this agent cannot spawn further subagents",
                    MAX_SUBAGENT_DEPTH
                ),
            }));
        }
        if self.concurrent.load(Ordering::Acquire) >= MAX_CONCURRENT_SUBAGENTS_PER_PARENT {
            return Ok(json!({
                "status": "rejected",
                "error": format!(
                    "max concurrent subagents ({}) reached — wait for one to complete before spawning another",
                    MAX_CONCURRENT_SUBAGENTS_PER_PARENT
                ),
            }));
        }

        // --- Allocate & bookkeep --------------------------------------------
        let subagent_id = gen_id();
        self.pending.fetch_add(1, Ordering::AcqRel);
        self.concurrent.fetch_add(1, Ordering::AcqRel);

        // Emit "started" on the spawning agent's sink.
        if let Some(ref sink) = self.ctx.parent_event_sink {
            let _ = sink.send(AgentEvent::SubagentStarted {
                subagent_id: subagent_id.clone(),
                task: args.task.clone(),
            });
        }

        // --- Spawn ----------------------------------------------------------
        let ctx = self.ctx.clone();
        let tx = self.tx.clone();
        let concurrent = Arc::clone(&self.concurrent);
        let spawning_sink = self.ctx.parent_event_sink.clone();
        let registry = Arc::clone(&self.ctx.registry);
        let registry_for_remove = Arc::clone(&self.ctx.registry);
        let task_text = args.task.clone();
        let context_text = args.context.clone();
        let id_for_task = subagent_id.clone();
        let id_for_registry = subagent_id.clone();
        let parent_id = self.ctx.parent_id.clone();
        let depth = self.ctx.depth;
        let task_for_registry = args.task.clone();

        let handle = tokio::spawn(async move {
            let outcome =
                run_subagent(ctx, id_for_task.clone(), task_text.clone(), context_text).await;

            let (status, output) = match outcome {
                Ok(reply) => (SubagentStatus::Completed, reply),
                Err(e) => (SubagentStatus::Failed, e),
            };

            concurrent.fetch_sub(1, Ordering::AcqRel);
            registry_for_remove.remove(&id_for_task);

            // Emit SubagentCompleted on the SPAWNING agent's sink so the UI
            // can close out its "subagent running" indicator immediately.
            if let Some(sink) = spawning_sink {
                let _ = sink.send(AgentEvent::SubagentCompleted {
                    subagent_id: id_for_task.clone(),
                    status: status.as_str().to_string(),
                    output: output.clone(),
                });
            }

            let _ = tx.send(SubagentResult {
                id: id_for_task,
                task: task_text,
                status,
                output,
            });
        });

        registry.insert(RegistryEntry {
            id: id_for_registry,
            parent_id,
            task: task_for_registry,
            depth,
            started_at: std::time::SystemTime::now(),
            handle,
        });

        Ok(json!({
            "subagent_id": subagent_id,
            "status": "started",
        }))
    }
}

// ---------------------------------------------------------------------------
// Child agent execution
// ---------------------------------------------------------------------------

async fn run_subagent(
    parent_ctx: SubagentContext,
    subagent_id: String,
    task: String,
    context_text: String,
) -> Result<String, String> {
    use storage::fs::FsStore;

    // 1. Set up event forwarding: the child has its own sink; every event it
    //    emits is wrapped as `SubagentEvent { subagent_id, inner }` and
    //    forwarded to the spawning agent's sink.
    let (child_event_tx, mut child_event_rx) = mpsc::unbounded_channel::<AgentEvent>();
    if let Some(parent_sink) = parent_ctx.parent_event_sink.clone() {
        let id_for_wrap = subagent_id.clone();
        tokio::spawn(async move {
            while let Some(inner) = child_event_rx.recv().await {
                let _ = parent_sink.send(AgentEvent::SubagentEvent {
                    subagent_id: id_for_wrap.clone(),
                    inner: Box::new(inner),
                });
            }
        });
    }

    // 2. Storage: this subagent's own conversation files live under
    //    `parent_ctx.subagent_root`. Its grandchildren (if any) live under
    //    `<subagent_root>/<this_id>/subagent/`.
    let store = FsStore::with_dir(parent_ctx.subagent_root.clone())
        .map_err(|e| format!("failed to open subagent store: {e}"))?;

    // 3. Conversation.
    let mut conv = Conversation::new(&subagent_id);
    if let Some(ref m) = parent_ctx.model {
        conv = conv.with_model(m);
    }

    // 4. Initial user message: task + context.
    let input = if context_text.trim().is_empty() {
        format!("# Task\n\n{}", task)
    } else {
        format!(
            "# Task\n\n{}\n\n# Context\n\n{}",
            task.trim(),
            context_text.trim()
        )
    };

    // 5. Equip this child with ITS OWN subagent inbox so grandchildren's
    //    results route back to this child (not up to the grandparent).
    let nested_ctx = SubagentContext {
        chat_client: Arc::clone(&parent_ctx.chat_client),
        base_tools: Arc::clone(&parent_ctx.base_tools),
        parent_event_sink: Some(child_event_tx.clone()),
        depth: parent_ctx.depth + 1,
        parent_id: Some(subagent_id.clone()),
        model: parent_ctx.model.clone(),
        reasoning: parent_ctx.reasoning.clone(),
        subagent_root: parent_ctx.subagent_root.join(&subagent_id).join("subagent"),
        scratch_dir: parent_ctx.scratch_dir.clone(),
        registry: Arc::clone(&parent_ctx.registry),
    };
    let (mut inbox, tools) = subagents::equip(nested_ctx);

    // 6. Run the child agent loop to completion. Subagents block on their
    //    own children (grandchildren of the parent) — only the top-level
    //    agent suspends.
    let reasoning = parent_ctx.reasoning.clone();
    let scratch = parent_ctx.scratch_dir.clone();
    let mut outcome = crate::agent::run(
        parent_ctx.chat_client.as_ref(),
        &store,
        &mut conv,
        tools.clone(),
        &input,
        reasoning.clone(),
        Some(&child_event_tx),
        Some(&mut inbox),
        scratch.as_deref(),
    )
    .await
    .map_err(|e| format!("subagent agent error: {e}"))?;

    // If the child was suspended (its own sub-subagents are pending), block
    // here until they all finish. This is intentional: subagents always
    // return a complete result to their parent.
    loop {
        match outcome {
            crate::agent::RunOutcome::Done(result) => {
                let _ = conversation::save(&store, &conv);
                return Ok(result.reply);
            }
            crate::agent::RunOutcome::Suspended { .. } => {
                if let Some(sub_result) = inbox.recv().await {
                    crate::agent::inject_subagent_result(
                        &store,
                        &mut conv,
                        &inbox.pending,
                        sub_result,
                    )
                    .map_err(|e| format!("failed to inject sub-subagent result: {e}"))?;
                }
                outcome = crate::agent::resume(
                    parent_ctx.chat_client.as_ref(),
                    &store,
                    &mut conv,
                    tools.clone(),
                    reasoning.clone(),
                    Some(&child_event_tx),
                    Some(&mut inbox),
                    scratch.as_deref(),
                )
                .await
                .map_err(|e| format!("subagent resume error: {e}"))?;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// id generation
// ---------------------------------------------------------------------------

fn gen_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("sub-{:x}-{:x}", d.as_secs(), d.subsec_nanos())
}
