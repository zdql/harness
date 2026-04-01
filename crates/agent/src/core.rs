use crate::conversation::{self, Conversation};
use crate::llm::{
    AssistantContent, AssistantMessage, ChatClient, ChatCompletionMessage, ChatError,
    CreateChatCompletionRequest,
};
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
// Core run loop — single turn
// ---------------------------------------------------------------------------

/// Append a user message, call the LLM, append the assistant reply, save, and
/// return the assistant's text content.
pub async fn run<S: ConversationStore>(
    client: &ChatClient,
    store: &S,
    conv: &mut Conversation,
    input: &str,
) -> Result<String, RunError<S::Error>> {
    // 1. Push user message & persist it.
    conv.push_user(input);
    conversation::save(store, conv).map_err(RunError::Storage)?;

    // 2. Build request from full message history.
    let request = CreateChatCompletionRequest {
        model: conv.model.clone(),
        messages: conv.messages.clone(),
        ..Default::default()
    };

    // 3. Call LLM.
    let response = client.create_chat_completion(&request).await?;

    // 4. Extract reply text and push assistant message.
    let choice = response.choices.first();
    let text = choice
        .and_then(|c| c.message.content.as_deref())
        .unwrap_or("")
        .to_string();

    conv.push_message(ChatCompletionMessage::Assistant(AssistantMessage {
        content: Some(AssistantContent::String(text.clone())),
        name: None,
        refusal: choice.and_then(|c| c.message.refusal.clone()),
        tool_calls: None,
        function_call: None,
        audio: None,
    }));

    // 5. Persist the assistant message.
    conversation::save(store, conv).map_err(RunError::Storage)?;

    Ok(text)
}
