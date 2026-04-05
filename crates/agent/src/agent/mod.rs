use crate::context::ContextBudget;
use crate::conversation::{self, compaction, Conversation};
use crate::llm::{
    AssistantContent, AssistantMessage, ChatClient, ChatCompletionMessage, ChatError,
    CreateChatCompletionRequest, StringOrTextParts, SystemMessage, ToolMessage,
};
use crate::prompts;
use crate::tools::ToolRegistry;
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
// Emitted in real time during `run_with_events` so callers can render
// progress (thinking spinner, tool call previews) before the final response
// is ready.
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
    /// A tool is about to execute.
    ToolCallStart { name: String, arguments: String },
    /// A tool finished executing.
    ToolCallEnd { name: String, result: String },
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
/// Thin wrapper over [`run_with_events`] with no event streaming — keeps the
/// existing callsites (tests, etc.) unchanged.
pub async fn run<S: ConversationStore>(
    client: &ChatClient,
    store: &S,
    conv: &mut Conversation,
    tools: &ToolRegistry,
    input: &str,
) -> Result<RunResult, RunError<S::Error>> {
    run_with_events(client, store, conv, tools, input, None).await
}

/// Same as [`run`], but streams [`AgentEvent`]s to the given sink as the
/// loop progresses. Callers use this to drive a "thinking" indicator and
/// render tool calls before the final response lands.
pub async fn run_with_events<S: ConversationStore>(
    client: &ChatClient,
    store: &S,
    conv: &mut Conversation,
    tools: &ToolRegistry,
    input: &str,
    events: Option<&EventSink>,
) -> Result<RunResult, RunError<S::Error>> {
    // 1. Push user message & persist.
    conv.push_user(input);
    conversation::save(store, conv).map_err(RunError::Storage)?;

    let mut executed_tool_calls: Vec<ToolCallInfo> = Vec::new();

    loop {
        // 2. Build request with tool definitions.
        //    Prepend the system prompt so the model always sees it first.
        let tool_defs = tools.definitions();
        let mut messages = Vec::with_capacity(1 + conv.messages.len());
        messages.push(ChatCompletionMessage::System(SystemMessage {
            content: StringOrTextParts::String(prompts::SYSTEM_PROMPT.to_string()),
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
            ..Default::default()
        };

        // 3. Call LLM.
        emit(events, AgentEvent::LlmStart);
        let response = client.create_chat_completion(&request).await?;
        emit(events, AgentEvent::LlmEnd);

        let choice = response.choices.first();
        let resp_msg = choice.map(|c| &c.message);

        // 4. Extract text and tool calls from the response.
        let text = resp_msg
            .and_then(|m| m.content.as_deref())
            .unwrap_or("")
            .to_string();

        let tool_calls = resp_msg.and_then(|m| m.tool_calls.clone());

        // 5. Push assistant message into conversation.
        conv.push_message(ChatCompletionMessage::Assistant(AssistantMessage {
            content: if text.is_empty() {
                None
            } else {
                Some(AssistantContent::String(text.clone()))
            },
            name: None,
            refusal: resp_msg.and_then(|m| m.refusal.clone()),
            tool_calls: tool_calls.clone(),
            function_call: None,
            audio: None,
        }));
        conversation::save(store, conv).map_err(RunError::Storage)?;

        // 6. If no tool calls, we're done — return the final text.
        let calls = match tool_calls {
            Some(ref calls) if !calls.is_empty() => calls,
            _ => return Ok(RunResult { reply: text, tool_calls: executed_tool_calls }),
        };

        // 7. Execute each tool call and push tool result messages.
        let budget = ContextBudget::for_model(conv.model.as_deref());
        let tool_defs_for_budget = tools.definitions();

        for call in calls {
            emit(
                events,
                AgentEvent::ToolCallStart {
                    name: call.function.name.clone(),
                    arguments: call.function.arguments.clone(),
                },
            );

            let raw_result = match tools.call(&call.function.name, &call.function.arguments) {
                Ok(val) => val.to_string(),
                Err(e) => format!("{{\"error\": \"{e}\"}}"),
            };

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

            conv.push_message(ChatCompletionMessage::Tool(ToolMessage {
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
