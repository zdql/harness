use crate::conversation::{self, Conversation};
use crate::llm::{
    AssistantContent, AssistantMessage, ChatClient, ChatCompletionMessage, ChatError,
    CreateChatCompletionRequest, StringOrTextParts, ToolMessage,
};
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

/// Append a user message, then loop: call the LLM, execute any tool calls,
/// feed results back, and repeat until the model produces a final text answer.
pub async fn run<S: ConversationStore>(
    client: &ChatClient,
    store: &S,
    conv: &mut Conversation,
    tools: &ToolRegistry,
    input: &str,
) -> Result<String, RunError<S::Error>> {
    // 1. Push user message & persist.
    conv.push_user(input);
    conversation::save(store, conv).map_err(RunError::Storage)?;

    loop {
        // 2. Build request with tool definitions.
        let tool_defs = tools.definitions();
        let request = CreateChatCompletionRequest {
            model: conv.model.clone(),
            messages: conv.messages.clone(),
            tools: if tool_defs.is_empty() {
                None
            } else {
                Some(tool_defs)
            },
            ..Default::default()
        };

        // 3. Call LLM.
        let response = client.create_chat_completion(&request).await?;

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
            _ => return Ok(text),
        };

        // 7. Execute each tool call and push tool result messages.
        for call in calls {
            let result = match tools.call(&call.function.name, &call.function.arguments) {
                Ok(val) => val.to_string(),
                Err(e) => format!("{{\"error\": \"{e}\"}}"),
            };

            conv.push_message(ChatCompletionMessage::Tool(ToolMessage {
                content: StringOrTextParts::String(result),
                tool_call_id: call.id.clone(),
            }));
            conversation::save(store, conv).map_err(RunError::Storage)?;
        }

        // Loop back to step 2 — the model will see the tool results and continue.
    }
}
