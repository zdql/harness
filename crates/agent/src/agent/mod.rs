// ---------------------------------------------------------------------------
// The agent loop.
//
// Every iteration of `run_loop` is one LLM turn:
//   1. Drain finished subagents into the conversation as user messages.
//   2. Compute this turn's token budget + tool definitions.
//   3. Stream one assistant message (reasoning + content deltas + tool calls).
//   4. If the assistant produced no tool calls → finalize (Done or Suspended).
//   5. Execute the tool batch in parallel; append a tool message per result,
//      guarding each against the context budget.
//   6. Compact the conversation if it's growing past the budget.
// ---------------------------------------------------------------------------

use std::sync::Arc;

use crate::context::ContextBudget;
use crate::conversation::{self, Conversation, compaction};
use crate::llm::{ChatBackend, ChatCompletionMessage, Reasoning, StringOrTextParts, ToolMessage};
use crate::subagents::SubagentInbox;
use crate::tools::{ToolRegistry, handler};
use storage::ConversationStore;

mod events;
mod stream;
mod suspension;
mod types;

pub use events::{AgentEvent, EventSink};
pub use suspension::inject_subagent_result;
pub use types::{RunError, RunOutcome, RunResult, ToolCallInfo};

use events::emit;
use stream::stream_assistant_message;
use suspension::{drain_inbox_into_conv, finalize_or_suspend};

/// Append a user message, then enter the agent loop.
pub async fn run<S: ConversationStore>(
    client: &dyn ChatBackend,
    store: &S,
    conv: &mut Conversation,
    tools: Arc<ToolRegistry>,
    input: &str,
    reasoning: Option<Reasoning>,
    events: Option<&EventSink>,
    subagent_inbox: Option<&mut SubagentInbox>,
    scratch_dir: Option<&std::path::Path>,
) -> Result<RunOutcome, RunError<S::Error>> {
    conv.push_user(input);
    conversation::save(store, conv).map_err(RunError::Storage)?;
    run_loop(
        client,
        store,
        conv,
        tools,
        reasoning,
        events,
        subagent_inbox,
        scratch_dir,
    )
    .await
}

/// Continue the agent loop after a subagent result has been injected. Unlike
/// `run`, this does NOT push a new user message — the caller must have called
/// `inject_subagent_result` first.
pub async fn resume<S: ConversationStore>(
    client: &dyn ChatBackend,
    store: &S,
    conv: &mut Conversation,
    tools: Arc<ToolRegistry>,
    reasoning: Option<Reasoning>,
    events: Option<&EventSink>,
    subagent_inbox: Option<&mut SubagentInbox>,
    scratch_dir: Option<&std::path::Path>,
) -> Result<RunOutcome, RunError<S::Error>> {
    run_loop(
        client,
        store,
        conv,
        tools,
        reasoning,
        events,
        subagent_inbox,
        scratch_dir,
    )
    .await
}

async fn run_loop<S: ConversationStore>(
    client: &dyn ChatBackend,
    store: &S,
    conv: &mut Conversation,
    tools: Arc<ToolRegistry>,
    reasoning: Option<Reasoning>,
    events: Option<&EventSink>,
    mut subagent_inbox: Option<&mut SubagentInbox>,
    scratch_dir: Option<&std::path::Path>,
) -> Result<RunOutcome, RunError<S::Error>> {
    let mut executed: Vec<ToolCallInfo> = Vec::new();

    loop {
        // 1. Pull in any subagents that finished since the last turn.
        drain_inbox_into_conv(store, conv, &mut subagent_inbox).map_err(RunError::Storage)?;

        // 2. Per-turn budget + tool definitions, used for both the LLM call
        //    and per-tool-result truncation below.
        let budget = ContextBudget::for_model(conv.model.as_deref());
        let tool_defs = tools.definitions();

        // 3. Stream one assistant turn.
        let turn = stream_assistant_message(
            client,
            store,
            conv,
            &tool_defs,
            reasoning.as_ref(),
            events,
            scratch_dir,
        )
        .await?;

        // 4. No tool calls → done, or suspended if subagents are still in flight.
        if turn.tool_calls.is_empty() {
            let pending = subagent_inbox.as_ref().map(|ib| ib.pending()).unwrap_or(0);
            return Ok(finalize_or_suspend(turn.text, executed, pending));
        }

        // 5. Run all tool calls in parallel, appending each result as a tool
        //    message. Emission and persistence stay here — `handler` only runs
        //    the batch.
        for call in &turn.tool_calls {
            emit(
                events,
                AgentEvent::ToolCallStart {
                    name: call.function.name.clone(),
                    arguments: call.function.arguments.clone(),
                },
            );
        }
        let batch = handler::execute_batch(Arc::clone(&tools), &turn.tool_calls).await;
        for (call, raw_result) in batch {
            let result = budget.guard_tool_result(&conv.messages, &tool_defs, &raw_result);
            emit(
                events,
                AgentEvent::ToolCallEnd {
                    name: call.function.name.clone(),
                    arguments: call.function.arguments.clone(),
                    result: result.clone(),
                },
            );
            executed.push(ToolCallInfo {
                name: call.function.name.clone(),
                arguments: call.function.arguments.clone(),
                result: result.clone(),
            });
            conv.push_message(ChatCompletionMessage::Tool(ToolMessage {
                content: StringOrTextParts::String(result),
                tool_call_id: call.id.clone(),
            }));
            conversation::save(store, conv).map_err(RunError::Storage)?;
        }

        // 6. Compact if the conversation is approaching the budget.
        if compaction::needs_compaction(&budget, &conv.messages, &tool_defs) {
            let _ = compaction::compact(client, store, conv).await;
        }
    }
}
