use std::sync::atomic::Ordering;
use std::sync::Arc;

use futures_util::StreamExt;

use crate::context::ContextBudget;
use crate::conversation::{self, compaction, Conversation};
use crate::llm::{
    AssistantContent, AssistantMessage, ChatClient, ChatCompletionMessage, ChatError,
    CreateChatCompletionRequest, Reasoning, StreamAccumulator, StringOrTextParts, SystemMessage,
};
use crate::prompts;
use crate::subagents::{SubagentInbox, SubagentResult};
use crate::tools::{handler, ToolRegistry};
use storage::ConversationStore;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum RunError<SE: std::error::Error> {
    Chat(ChatError),
    Storage(SE),
}

impl<SE: std::error::Error> std::fmt::Display for RunError<SE> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Chat(e) => write!(f, "{e}"),
            Self::Storage(e) => write!(f, "Storage: {e}"),
        }
    }
}

impl<SE: std::error::Error> std::error::Error for RunError<SE> {}

impl<SE: std::error::Error> From<ChatError> for RunError<SE> {
    fn from(e: ChatError) -> Self {
        Self::Chat(e)
    }
}

// ---------------------------------------------------------------------------
// Core agent loop
// ---------------------------------------------------------------------------

/// Info about a tool call that was executed during the agent loop.
#[derive(Debug, Clone)]
pub struct ToolCallInfo {
    pub name: String,
    pub arguments: String,
    pub result: String,
}

/// The result of an agent run: final text reply + any tool calls that were executed.
#[derive(Debug)]
pub struct RunResult {
    pub reply: String,
    pub tool_calls: Vec<ToolCallInfo>,
}

// ---------------------------------------------------------------------------
// Streaming events
//
// Emitted in real time during the agent loop so callers can render progress
// (thinking spinner, tool call previews, subagent activity) before the final
// response is ready.
// ---------------------------------------------------------------------------

/// An event emitted during the agent loop. Wire shape is a serde-tagged enum
/// so the server can forward these as JSON-RPC notifications directly.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentEvent {
    /// LLM request is in-flight.
    LlmStart,
    /// LLM request returned.
    LlmEnd,
    /// Incremental reasoning / thinking text from a thinking model.
    /// Frontends can render this live as the model "thinks".
    ReasoningDelta { text: String },
    /// Incremental assistant-visible content text. Sent as the final reply
    /// streams in so the UI can render it progressively.
    ContentDelta { text: String },
    /// A tool is about to execute.
    ToolCallStart { name: String, arguments: String },
    /// A tool finished executing.
    ToolCallEnd { name: String, result: String },
    /// A subagent has been spawned from this agent's `start_subagent` tool.
    SubagentStarted { subagent_id: String, task: String },
    /// A subagent spawned from this agent has finished.
    SubagentCompleted {
        subagent_id: String,
        status: String,
        output: String,
    },
    /// An event emitted by a subagent (or one of its own subagents). Nested
    /// `SubagentEvent`s form a tree mirroring the spawn hierarchy.
    SubagentEvent {
        subagent_id: String,
        inner: Box<AgentEvent>,
    },
}

/// A channel for streaming [`AgentEvent`]s out of the agent loop.
pub type EventSink = tokio::sync::mpsc::UnboundedSender<AgentEvent>;

fn emit(sink: Option<&EventSink>, event: AgentEvent) {
    if let Some(s) = sink {
        // Receiver-closed is non-fatal — the frontend may have disconnected
        // mid-run, but the agent loop should still complete.
        let _ = s.send(event);
    }
}

/// Append a user message, then loop: call the LLM, execute any tool calls,
/// feed results back, and repeat until the model produces a final text answer.
///
/// If `subagent_inbox` is `Some`, the loop drains pending subagent results
/// into the conversation as user messages at each iteration boundary, and —
/// on exit — waits for any still-in-flight subagents before returning.
///
/// If `events` is `Some`, streams [`AgentEvent`]s to the sink as the loop
/// progresses.
pub async fn run<S: ConversationStore>(
    client: &ChatClient,
    store: &S,
    conv: &mut Conversation,
    tools: Arc<ToolRegistry>,
    input: &str,
    reasoning: Option<Reasoning>,
    events: Option<&EventSink>,
    mut subagent_inbox: Option<&mut SubagentInbox>,
    scratch_dir: Option<&std::path::Path>,
) -> Result<RunResult, RunError<S::Error>> {
    // 1. Push user message & persist.
    conv.push_user(input);
    conversation::save(store, conv).map_err(RunError::Storage)?;

    let mut executed_tool_calls: Vec<ToolCallInfo> = Vec::new();

    loop {
        // Drain any completed subagents into the conversation before every
        // LLM call so the model sees their results on its next turn.
        if let Some(ref mut inbox) = subagent_inbox {
            for result in inbox.try_drain() {
                inject_subagent_result(store, conv, &inbox.pending, result)
                    .map_err(RunError::Storage)?;
            }
        }

        // 2. Build request with tool definitions.
        //    Prepend the system prompt so the model always sees it first.
        let tool_defs = tools.definitions();
        let mut messages = Vec::with_capacity(1 + conv.messages.len());
        messages.push(ChatCompletionMessage::System(SystemMessage {
            content: StringOrTextParts::String(prompts::build_system_prompt(scratch_dir)),
            name: None,
        }));
        messages.extend(conv.messages.iter().cloned());

        let request = CreateChatCompletionRequest {
            model: conv.model.clone(),
            messages,
            tools: if tool_defs.is_empty() {
                None
            } else {
                Some(tool_defs)
            },
            reasoning: reasoning.clone(),
            ..Default::default()
        };

        // 3. Call LLM (streaming). Fold chunks into a StreamAccumulator and
        //    emit ReasoningDelta / ContentDelta events as text arrives so the
        //    frontend can render "thinking" and the final reply live.
        emit(events, AgentEvent::LlmStart);
        let mut stream = client.create_chat_completion_stream(&request).await?;
        let mut acc = StreamAccumulator::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            let appended = acc.push_chunk(&chunk);
            if !appended.reasoning.is_empty() {
                emit(
                    events,
                    AgentEvent::ReasoningDelta {
                        text: appended.reasoning,
                    },
                );
            }
            if !appended.content.is_empty() {
                emit(
                    events,
                    AgentEvent::ContentDelta {
                        text: appended.content,
                    },
                );
            }
            if let Some(msg) = appended.error {
                return Err(RunError::Chat(ChatError::Stream(msg)));
            }
        }
        emit(events, AgentEvent::LlmEnd);

        let assembled = acc.into_message();

        // 4. Extract text and tool calls from the assembled response.
        let text = assembled.content;
        let reasoning_details = if assembled.reasoning_details.is_empty() {
            None
        } else {
            Some(assembled.reasoning_details)
        };
        let tool_calls = if assembled.tool_calls.is_empty() {
            None
        } else {
            Some(assembled.tool_calls)
        };

        // 5. Push assistant message into conversation. `reasoning_details`
        //    must round-trip back to the model on the next turn — Anthropic
        //    thinking + tool use rejects requests that drop them.
        conv.push_message(ChatCompletionMessage::Assistant(AssistantMessage {
            content: if text.is_empty() {
                None
            } else {
                Some(AssistantContent::String(text.clone()))
            },
            name: None,
            refusal: assembled.refusal,
            tool_calls: tool_calls.clone(),
            function_call: None,
            audio: None,
            reasoning_details,
        }));
        conversation::save(store, conv).map_err(RunError::Storage)?;

        // 6. If no tool calls, we're either done OR we should wait for
        //    in-flight subagents before deciding to exit.
        let calls = match tool_calls {
            Some(ref calls) if !calls.is_empty() => calls,
            _ => {
                // Check for pending subagents.
                if let Some(ref mut inbox) = subagent_inbox {
                    if inbox.pending() > 0 {
                        // Block on the next result, then loop back so the
                        // model can respond to it.
                        if let Some(result) = inbox.recv().await {
                            inject_subagent_result(store, conv, &inbox.pending, result)
                                .map_err(RunError::Storage)?;
                        }
                        continue;
                    }
                }
                return Ok(RunResult {
                    reply: text,
                    tool_calls: executed_tool_calls,
                });
            }
        };

        // 7. Execute the batch of tool calls concurrently, then persist
        //    results in the original call order so tool_call_id pairing stays
        //    aligned with the assistant message.
        let budget = ContextBudget::for_model(conv.model.as_deref());
        let tool_defs_for_budget = tools.definitions();

        // Emit all start events up front — the frontend can render every
        // running tool call before any of them return.
        for call in calls {
            emit(
                events,
                AgentEvent::ToolCallStart {
                    name: call.function.name.clone(),
                    arguments: call.function.arguments.clone(),
                },
            );
        }

        let batch_results = handler::execute_batch(Arc::clone(&tools), calls).await;

        for (call, raw_result) in batch_results {
            let result = budget.guard_tool_result(
                &conv.messages,
                &tool_defs_for_budget,
                &raw_result,
            );

            emit(
                events,
                AgentEvent::ToolCallEnd {
                    name: call.function.name.clone(),
                    result: result.clone(),
                },
            );

            executed_tool_calls.push(ToolCallInfo {
                name: call.function.name.clone(),
                arguments: call.function.arguments.clone(),
                result: result.clone(),
            });

            conv.push_message(ChatCompletionMessage::Tool(crate::llm::ToolMessage {
                content: StringOrTextParts::String(result),
                tool_call_id: call.id.clone(),
            }));
            conversation::save(store, conv).map_err(RunError::Storage)?;
        }

        // 8. Check whether context compaction is needed before the next round.
        let budget = ContextBudget::for_model(conv.model.as_deref());
        let tool_defs_for_check = tools.definitions();
        if compaction::needs_compaction(&budget, &conv.messages, &tool_defs_for_check) {
            // Best-effort: if compaction fails we just continue with the full
            // context and hope the next round stays within limits.
            let _ = compaction::compact(client, store, conv).await;
        }

        // Loop back to step 2 — the model will see the tool results and continue.
    }
}

// ---------------------------------------------------------------------------
// Subagent result injection
// ---------------------------------------------------------------------------

/// Append a finished subagent's result to the conversation as a user message
/// and decrement the pending counter.
fn inject_subagent_result<S: ConversationStore>(
    store: &S,
    conv: &mut Conversation,
    pending: &std::sync::atomic::AtomicUsize,
    result: SubagentResult,
) -> Result<(), S::Error> {
    let body = format!(
        "SUBAGENT {} {}: {}",
        result.id,
        result.status.as_str(),
        result.output,
    );
    conv.push_user(&body);
    conversation::save(store, conv)?;
    pending.fetch_sub(1, Ordering::AcqRel);
    Ok(())
}
