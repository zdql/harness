// One streamed assistant turn:
//   1. Build the request from the current conversation.
//   2. Stream chunks, emitting reasoning/content deltas to `events`.
//   3. Append the assembled assistant message to `conv` and persist.

use futures_util::StreamExt;

use crate::conversation::{self, Conversation};
use crate::llm::{
    AssistantContent, AssistantMessage, ChatBackend, ChatCompletionMessage, ChatCompletionTool,
    ChatError, CreateChatCompletionRequest, Reasoning, StreamAccumulator, StringOrTextParts,
    SystemMessage, ToolCall,
};
use crate::prompts;
use storage::ConversationStore;

use super::events::{AgentEvent, EventSink, emit};
use super::types::RunError;

/// What one streamed assistant turn produced. The assistant message has
/// already been pushed to the conversation and persisted.
pub(crate) struct StreamedTurn {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
}

pub(crate) async fn stream_assistant_message<S: ConversationStore>(
    client: &dyn ChatBackend,
    store: &S,
    conv: &mut Conversation,
    tool_defs: &[ChatCompletionTool],
    reasoning: Option<&Reasoning>,
    events: Option<&EventSink>,
    scratch_dir: Option<&std::path::Path>,
) -> Result<StreamedTurn, RunError<S::Error>> {
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
            Some(tool_defs.to_vec())
        },
        reasoning: reasoning.cloned(),
        ..Default::default()
    };

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
    let text = assembled.content;
    let tool_calls = assembled.tool_calls;
    let reasoning_details = if assembled.reasoning_details.is_empty() {
        None
    } else {
        Some(assembled.reasoning_details)
    };
    let tool_calls_field = if tool_calls.is_empty() {
        None
    } else {
        Some(tool_calls.clone())
    };

    conv.push_message(ChatCompletionMessage::Assistant(AssistantMessage {
        content: if text.is_empty() {
            None
        } else {
            Some(AssistantContent::String(text.clone()))
        },
        name: None,
        refusal: assembled.refusal,
        tool_calls: tool_calls_field,
        function_call: None,
        audio: None,
        reasoning_details,
    }));
    conversation::save(store, conv).map_err(RunError::Storage)?;

    Ok(StreamedTurn { text, tool_calls })
}
