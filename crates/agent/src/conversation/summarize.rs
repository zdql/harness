// ---------------------------------------------------------------------------
// conversation::summarize — Generate a one-sentence title for a conversation
//
// Fires a cheap LLM completion to distill the conversation into a short title.
// Designed to run in the background so it never blocks the main send path.
// ---------------------------------------------------------------------------

use crate::llm::{
    ChatBackend, ChatCompletionMessage, CreateChatCompletionRequest, StringOrTextParts,
    SystemMessage, UserContent, UserMessage,
};

use super::Conversation;

/// Build a summarization prompt from the conversation's messages.
pub fn build_messages(conv: &Conversation) -> Vec<ChatCompletionMessage> {
    let system = ChatCompletionMessage::System(SystemMessage {
        content: StringOrTextParts::String(
            "You are a concise title generator. Given a conversation, \
             respond with ONLY a short title (3-8 words). No quotes, \
             no punctuation at the end, no explanation."
                .to_string(),
        ),
        name: None,
    });

    // Take up to 20 most recent messages to keep the request small.
    let recent: Vec<_> = conv.messages.iter().rev().take(20).rev().cloned().collect();

    // Render the conversation as a single user message for the summarizer.
    let mut transcript = String::new();
    for msg in &recent {
        match msg {
            ChatCompletionMessage::User(u) => {
                let text = match &u.content {
                    UserContent::String(s) => s.as_str(),
                    _ => continue,
                };
                transcript.push_str(&format!("User: {text}\n"));
            }
            ChatCompletionMessage::Assistant(a) => {
                if let Some(ref c) = a.content {
                    let text = match c {
                        crate::llm::AssistantContent::String(s) => s.as_str(),
                        _ => continue,
                    };
                    transcript.push_str(&format!("Assistant: {text}\n"));
                }
            }
            _ => {}
        }
    }

    let user = ChatCompletionMessage::User(UserMessage {
        content: UserContent::String(format!(
            "Generate a short title for this conversation:\n\n{transcript}"
        )),
        name: None,
    });

    vec![system, user]
}

/// Call the LLM to generate a title for the conversation.
/// Uses a cheap/fast model to minimize cost and latency.
pub async fn summarize(client: &dyn ChatBackend, conv: &Conversation) -> Result<String, String> {
    let messages = build_messages(conv);

    let request = CreateChatCompletionRequest {
        model: Some(crate::prompts::BACKGROUND_MODEL.to_string()),
        messages,
        max_tokens: Some(30),
        temperature: Some(0.0),
        ..Default::default()
    };

    let response = client
        .create_chat_completion(&request)
        .await
        .map_err(|e| format!("summarize error: {e}"))?;

    let title = response
        .choices
        .first()
        .and_then(|c| c.message.content.as_deref())
        .unwrap_or("Untitled")
        .trim()
        .to_string();

    Ok(title)
}
