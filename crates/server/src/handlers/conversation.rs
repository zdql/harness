// ---------------------------------------------------------------------------
// handlers::conversation — Conversation CRUD + agent send handlers
// ---------------------------------------------------------------------------

use std::sync::Arc;

use crate::ServerState;
use crate::rpc::methods::conversation::{
    ConversationSummary, CreateParams, CreateResult, GetParams, GetResult, ListParams, ListResult,
    MessageEntry, SendParams, SendResult, SetModelParams, SetModelResult, SwitchParams,
    SwitchResult, ToolCallInfo,
};
use agent::conversation::{self, Conversation, summarize};
use storage::ConversationStore;
use storage::fs::FsStore;

fn store() -> Result<FsStore, String> {
    FsStore::new().map_err(|e| format!("failed to open store: {e}"))
}

/// Handle `conversation.create` — create a new empty conversation.
pub fn create(_params: CreateParams) -> Result<CreateResult, String> {
    let store = store()?;
    let id = uuid();

    // Use the agent's Conversation type so metadata is consistent
    let settings = storage::settings::read();
    let mut conv = Conversation::new(&id);
    if let Some(ref model) = settings.model {
        conv = conv.with_model(model);
    }
    conversation::save(&store, &conv).map_err(|e| format!("failed to save: {e}"))?;

    // Set as active conversation
    let mut settings = storage::settings::read();
    settings.conversation = Some(id.clone());
    storage::settings::write(&settings).map_err(|e| format!("failed to write settings: {e}"))?;

    Ok(CreateResult { id })
}

/// Handle `conversation.list` — list conversations with >1 message, most recent first.
pub fn list(_params: ListParams) -> Result<ListResult, String> {
    let store = store()?;
    let ids = store.list().map_err(|e| format!("failed to list: {e}"))?;

    let mut conversations = Vec::new();
    for id in ids {
        let meta = store
            .load_metadata(&id)
            .map_err(|e| format!("failed to load metadata: {e}"))?;
        let msgs = store
            .load_messages(&id)
            .map_err(|e| format!("failed to load messages: {e}"))?;

        // Skip conversations that never had a real exchange
        if msgs.len() <= 1 {
            continue;
        }

        let title = meta["title"].as_str().map(String::from);
        let updated_at = meta["updated_at"].as_i64().unwrap_or(0);
        let subagent_count = count_subagents(&id);
        conversations.push(ConversationSummary {
            id,
            title,
            updated_at,
            subagent_count,
        });
    }

    Ok(ListResult { conversations })
}

/// Handle `conversation.switch` — set the active conversation.
pub fn switch(params: SwitchParams) -> Result<SwitchResult, String> {
    let store = store()?;
    store
        .load_metadata(&params.id)
        .map_err(|e| format!("conversation not found: {e}"))?;

    let mut settings = storage::settings::read();
    settings.conversation = Some(params.id.clone());
    storage::settings::write(&settings).map_err(|e| format!("failed to write settings: {e}"))?;

    Ok(SwitchResult { id: params.id })
}

/// Handle `conversation.get` — load a conversation's messages for display.
pub fn get(params: GetParams) -> Result<GetResult, String> {
    let store = store()?;
    let conv = conversation::load(&store, &params.id)
        .map_err(|e| format!("failed to load conversation: {e}"))?;

    let messages = conv
        .messages
        .iter()
        .filter_map(|msg| match msg {
            agent::llm::ChatCompletionMessage::User(u) => {
                let text = match &u.content {
                    agent::llm::UserContent::String(s) => s.clone(),
                    _ => return None,
                };
                Some(MessageEntry {
                    role: "user".to_string(),
                    content: text,
                    tool_name: None,
                    tool_args: None,
                })
            }
            agent::llm::ChatCompletionMessage::Assistant(a) => {
                // If this assistant message has tool calls but no text, skip it
                // (it's an intermediate step, the tool results follow)
                let text = match a.content.as_ref()? {
                    agent::llm::AssistantContent::String(s) => s.clone(),
                    _ => return None,
                };
                if text.is_empty() {
                    return None;
                }
                Some(MessageEntry {
                    role: "assistant".to_string(),
                    content: text,
                    tool_name: None,
                    tool_args: None,
                })
            }
            agent::llm::ChatCompletionMessage::Tool(t) => {
                let content = match &t.content {
                    agent::llm::StringOrTextParts::String(s) => s.clone(),
                    _ => return None,
                };
                Some(MessageEntry {
                    role: "tool".to_string(),
                    content,
                    tool_name: None,
                    tool_args: None,
                })
            }
            _ => None,
        })
        .collect();

    Ok(GetResult {
        id: conv.id,
        title: conv.title,
        messages,
    })
}

/// Handle `conversation.setModel` — update the model used by an existing
/// conversation. Empty string clears the per-conversation override and lets
/// the next send fall back to the global `settings.model`.
pub fn set_model(params: SetModelParams) -> Result<SetModelResult, String> {
    let store = store()?;
    let mut conv = conversation::load(&store, &params.id)
        .map_err(|e| format!("failed to load conversation: {e}"))?;

    conv.model = if params.model.trim().is_empty() {
        None
    } else {
        Some(params.model.trim().to_string())
    };

    store
        .save_metadata(&conv.id, &conv.metadata())
        .map_err(|e| format!("failed to save metadata: {e}"))?;

    Ok(SetModelResult {
        id: conv.id,
        model: conv.model,
    })
}

/// Handle `conversation.send` — send a user message through the agent loop.
///
/// Returns the assistant's reply immediately. If subagents are still running,
/// `suspended: true` is set in the result and a background task continues
/// the loop — streaming events via the event sink and emitting a final
/// `ContinuationDone` event when all subagents finish.
pub async fn send(
    params: SendParams,
    state: &ServerState,
    events: Option<agent::agent::EventSink>,
) -> Result<SendResult, String> {
    let store = store()?;

    let mut conv = conversation::load(&store, &params.id)
        .map_err(|e| format!("failed to load conversation: {e}"))?;

    let settings = storage::settings::read();

    if conv.model.is_none() {
        conv.model = settings
            .model
            .clone()
            .or_else(|| Some(agent::prompts::DEFAULT_MODEL.to_string()));
    }

    let reasoning = agent::prompts::resolve_reasoning(
        settings.reasoning_effort.as_deref(),
        settings.reasoning_summary.as_deref(),
    );

    let scratch_dir = scratch_dir_for(&conv.id);
    let scratch_dir = match std::fs::create_dir_all(&scratch_dir) {
        Ok(()) => Some(scratch_dir),
        Err(e) => {
            eprintln!("warning: failed to create scratch dir: {e}");
            None
        }
    };

    let subagent_root = subagent_root_for(&conv.id);
    let ctx = agent::subagents::SubagentContext {
        chat_client: Arc::clone(&state.chat_client),
        base_tools: Arc::clone(&state.tools),
        parent_event_sink: events.clone(),
        depth: 0,
        parent_id: None,
        model: conv.model.clone(),
        reasoning: reasoning.clone(),
        subagent_root,
        scratch_dir: scratch_dir.clone(),
        registry: Arc::clone(agent::subagents::SubagentRegistry::global()),
    };
    let (mut inbox, tools) = agent::subagents::equip(ctx);

    let outcome = agent::agent::run(
        state.chat_client.as_ref(),
        &store,
        &mut conv,
        tools.clone(),
        &params.message,
        reasoning.clone(),
        events.as_ref(),
        Some(&mut inbox),
        scratch_dir.as_deref(),
    )
    .await
    .map_err(|e| format!("agent error: {e}"))?;

    maybe_summarize(Arc::clone(&state.chat_client), &conv);

    match outcome {
        agent::agent::RunOutcome::Done(result) => Ok(SendResult {
            reply: result.reply,
            tool_calls: map_tool_calls(result.tool_calls),
            suspended: false,
        }),
        agent::agent::RunOutcome::Suspended { result, .. } => {
            let reply = result.reply.clone();
            let tc = map_tool_calls(result.tool_calls);

            // Spawn a background watcher: wait for subagent results, inject
            // them, and re-enter the agent loop. Events keep flowing through
            // the same event sink so the frontend sees live progress.
            let client = Arc::clone(&state.chat_client);
            let event_sink = events.clone();
            tokio::spawn(async move {
                if let Err(e) = subagent_continuation_loop(
                    client,
                    conv,
                    tools,
                    inbox,
                    reasoning,
                    event_sink,
                    scratch_dir,
                )
                .await
                {
                    eprintln!("continuation error: {e}");
                }
            });

            Ok(SendResult {
                reply,
                tool_calls: tc,
                suspended: true,
            })
        }
    }
}

/// Background loop that waits for subagent results, injects them into the
/// conversation, and re-enters the agent loop until all subagents are done.
///
/// Each iteration reloads the conversation from storage before injecting the
/// next result. That's load-bearing: while we're suspended, the user may have
/// sent another message through a fresh `conversation.send` (which writes to
/// the same on-disk conversation). Reloading lets the watcher's resumed LLM
/// turn see those messages instead of operating on a stale in-memory copy.
async fn subagent_continuation_loop(
    client: Arc<dyn agent::llm::ChatBackend>,
    initial_conv: Conversation,
    tools: Arc<agent::tools::ToolRegistry>,
    mut inbox: agent::subagents::SubagentInbox,
    reasoning: Option<agent::llm::Reasoning>,
    events: Option<agent::agent::EventSink>,
    scratch_dir: Option<std::path::PathBuf>,
) -> Result<(), String> {
    let conv_id = initial_conv.id.clone();
    drop(initial_conv);

    loop {
        let sub_result = match inbox.recv().await {
            Some(r) => r,
            None => return Ok(()),
        };

        let store = store()?;
        // Pull the latest on-disk state so any user messages sent during the
        // suspension window are visible to the resumed LLM call.
        let mut conv = conversation::load(&store, &conv_id)
            .map_err(|e| format!("conv reload failed: {e}"))?;

        agent::agent::inject_subagent_result(
            &store,
            &mut conv,
            inbox.pending_counter(),
            sub_result,
        )
        .map_err(|e| format!("inject error: {e}"))?;

        let outcome = agent::agent::resume(
            client.as_ref(),
            &store,
            &mut conv,
            tools.clone(),
            reasoning.clone(),
            events.as_ref(),
            Some(&mut inbox),
            scratch_dir.as_deref(),
        )
        .await
        .map_err(|e| format!("resume error: {e}"))?;

        match outcome {
            agent::agent::RunOutcome::Done(result) => {
                if let Some(ref sink) = events {
                    let _ = sink.send(agent::agent::AgentEvent::ContinuationDone {
                        reply: result.reply,
                    });
                }
                return Ok(());
            }
            agent::agent::RunOutcome::Suspended { .. } => {
                // Still more subagents pending — loop back to wait.
            }
        }
    }
}

fn maybe_summarize(client: Arc<dyn agent::llm::ChatBackend>, conv: &Conversation) {
    if conv.needs_summarization() {
        let conv_clone = conv.clone();
        tokio::spawn(async move {
            match summarize::summarize(client.as_ref(), &conv_clone).await {
                Ok(title) => {
                    if let Ok(fs) = FsStore::new() {
                        if let Ok(mut fresh) = conversation::load(&fs, &conv_clone.id) {
                            fresh.title = Some(title);
                            fresh.title_set_at_message_count = fresh.messages.len();
                            let _ = fs.save_metadata(&fresh.id, &fresh.metadata());
                        }
                    }
                }
                Err(e) => eprintln!("summarize failed: {e}"),
            }
        });
    }
}

fn map_tool_calls(tcs: Vec<agent::agent::ToolCallInfo>) -> Vec<ToolCallInfo> {
    tcs.into_iter()
        .map(|tc| ToolCallInfo {
            name: tc.name,
            arguments: tc.arguments,
            result: tc.result,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn subagent_root_for(conv_id: &str) -> std::path::PathBuf {
    conv_dir_for(conv_id).join("subagent")
}

fn scratch_dir_for(conv_id: &str) -> std::path::PathBuf {
    conv_dir_for(conv_id).join("tmp")
}

fn conv_dir_for(conv_id: &str) -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home)
        .join(".agent-harness")
        .join("conversations")
        .join(conv_id)
}

/// Recursively count all `.json` files under the subagent directory for a conversation.
fn count_subagents(conv_id: &str) -> usize {
    let root = subagent_root_for(conv_id);
    if !root.is_dir() {
        return 0;
    }

    let mut count = 0;
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    count += 1;
                }
            }
        }
    }
    count
}

fn uuid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{:x}-{:x}", d.as_secs(), d.subsec_nanos())
}
