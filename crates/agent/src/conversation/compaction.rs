// ---------------------------------------------------------------------------
// Conversation compaction — summarize old messages to reclaim context budget
//
// When the conversation's token usage exceeds a threshold, we call a cheap
// model to produce a dense summary. That summary becomes a new "compaction
// boundary" user message. The conversation's in-memory messages are then
// replaced with just that summary, and the boundary index is recorded so
// future loads start from there. The full history is never mutated on disk.
// ---------------------------------------------------------------------------

use crate::context::ContextBudget;
use crate::llm::{
    ChatClient, ChatCompletionMessage, ChatCompletionTool, CreateChatCompletionRequest,
    StringOrTextParts, SystemMessage, UserContent, UserMessage,
};
use crate::prompts;

use super::Conversation;

use storage::ConversationStore;

/// Fraction of the input budget at which compaction triggers (80%).
const COMPACTION_THRESHOLD: f64 = 0.80;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Returns `true` if the current conversation + tools exceeds the compaction
/// threshold and a compaction should be performed.
pub fn needs_compaction(
    budget: &ContextBudget,
    messages: &[ChatCompletionMessage],
    tools: &[ChatCompletionTool],
) -> bool {
    let used = crate::context::estimate_messages(messages)
        + crate::context::estimate_tools(tools);
    let limit = budget.input_budget();
    used as f64 > limit as f64 * COMPACTION_THRESHOLD
}

/// Run compaction: call the compaction model to summarize the conversation,
/// then mutate `conv` so that its messages contain only the compaction
/// boundary message. The boundary index is appended to `compaction_indices`
/// and the summary is persisted to disk.
///
/// Returns the summary text on success.
pub async fn compact<S: ConversationStore>(
    client: &ChatClient,
    store: &S,
    conv: &mut Conversation,
) -> Result<String, CompactionError<S::Error>> {
    // 1. Build the compaction request — system prompt + transcript.
    let transcript = render_transcript(&conv.messages);

    let messages = vec![
        ChatCompletionMessage::System(SystemMessage {
            content: StringOrTextParts::String(prompts::COMPACTION_PROMPT.to_string()),
            name: None,
        }),
        ChatCompletionMessage::User(UserMessage {
            content: UserContent::String(transcript),
            name: None,
        }),
    ];

    let request = CreateChatCompletionRequest {
        model: Some(prompts::COMPACTION_MODEL.to_string()),
        messages,
        temperature: Some(0.0),
        ..Default::default()
    };

    // 2. Call the compaction model.
    let response = client
        .create_chat_completion(&request)
        .await
        .map_err(CompactionError::Chat)?;

    let summary = response
        .choices
        .first()
        .and_then(|c| c.message.content.as_deref())
        .unwrap_or("[compaction failed — no summary produced]")
        .trim()
        .to_string();

    // 3. Create the compaction boundary message.
    let boundary_msg = ChatCompletionMessage::User(UserMessage {
        content: UserContent::String(format!(
            "[This message is a compaction summary of the preceding conversation.]\n\n{summary}"
        )),
        name: None,
    });

    // 4. Record the absolute index where this boundary lands on disk.
    let boundary_index = conv.absolute_len();

    // 5. Persist the boundary message to disk (append to .jsonl).
    let val = serde_json::to_value(&boundary_msg).expect("boundary message serializes");
    store
        .append_message(&conv.id, &val)
        .map_err(CompactionError::Storage)?;

    // 6. Update the conversation's compaction tracking.
    conv.compaction_indices.push(boundary_index);
    conv.message_offset = boundary_index;
    conv.messages = vec![boundary_msg];
    conv.updated_at = now_unix();

    // 7. Persist updated metadata (includes new compaction_indices).
    store
        .save_metadata(&conv.id, &conv.metadata())
        .map_err(CompactionError::Storage)?;

    Ok(summary)
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum CompactionError<SE: std::error::Error> {
    Chat(crate::llm::ChatError),
    Storage(SE),
}

impl<SE: std::error::Error> std::fmt::Display for CompactionError<SE> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Chat(e) => write!(f, "Compaction LLM error: {e}"),
            Self::Storage(e) => write!(f, "Compaction storage error: {e}"),
        }
    }
}

impl<SE: std::error::Error> std::error::Error for CompactionError<SE> {}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Render the conversation messages into a plain-text transcript for the
/// compaction model. Includes role labels and tool call info.
fn render_transcript(messages: &[ChatCompletionMessage]) -> String {
    let mut out = String::new();
    for msg in messages {
        match msg {
            ChatCompletionMessage::System(s) => {
                let text = match &s.content {
                    StringOrTextParts::String(s) => s.as_str(),
                    StringOrTextParts::Parts(parts) => {
                        if let Some(p) = parts.first() {
                            p.text.as_str()
                        } else {
                            continue;
                        }
                    }
                };
                out.push_str(&format!("[System]: {text}\n\n"));
            }
            ChatCompletionMessage::User(u) => {
                let text = match &u.content {
                    UserContent::String(s) => s.clone(),
                    _ => continue,
                };
                out.push_str(&format!("[User]: {text}\n\n"));
            }
            ChatCompletionMessage::Assistant(a) => {
                if let Some(ref content) = a.content {
                    let text = match content {
                        crate::llm::AssistantContent::String(s) => s.as_str(),
                        _ => "",
                    };
                    if !text.is_empty() {
                        out.push_str(&format!("[Assistant]: {text}\n\n"));
                    }
                }
                if let Some(ref calls) = a.tool_calls {
                    for call in calls {
                        out.push_str(&format!(
                            "[Assistant tool_call]: {}({})\n\n",
                            call.function.name, call.function.arguments
                        ));
                    }
                }
            }
            ChatCompletionMessage::Tool(t) => {
                let text = match &t.content {
                    StringOrTextParts::String(s) => s.as_str(),
                    StringOrTextParts::Parts(parts) => {
                        if let Some(p) = parts.first() {
                            p.text.as_str()
                        } else {
                            continue;
                        }
                    }
                };
                // Truncate very long tool results in the transcript to keep
                // the compaction request itself manageable.
                let max = prompts::COMPACTION_TOOL_RESULT_MAX_CHARS;
                let truncated = if text.len() > max {
                    format!("{}… [truncated]", &text[..max])
                } else {
                    text.to_string()
                };
                out.push_str(&format!(
                    "[Tool result for {}]: {truncated}\n\n",
                    t.tool_call_id
                ));
            }
            ChatCompletionMessage::Developer(d) => {
                let text = match &d.content {
                    StringOrTextParts::String(s) => s.as_str(),
                    StringOrTextParts::Parts(parts) => {
                        if let Some(p) = parts.first() {
                            p.text.as_str()
                        } else {
                            continue;
                        }
                    }
                };
                out.push_str(&format!("[Developer]: {text}\n\n"));
            }
            ChatCompletionMessage::Function(f) => {
                if let Some(ref c) = f.content {
                    out.push_str(&format!("[Function {}]: {c}\n\n", f.name));
                }
            }
        }
    }
    out
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_compaction_below_threshold() {
        let budget = ContextBudget {
            limit: 1000,
            reply_reserve: 100,
        };
        // input budget = 900 tokens. 80% = 720 tokens = 2880 chars.
        // Empty messages + tools → 0 tokens, well below threshold.
        let msgs: Vec<ChatCompletionMessage> = vec![];
        let tools: Vec<ChatCompletionTool> = vec![];
        assert!(!needs_compaction(&budget, &msgs, &tools));
    }

    #[test]
    fn needs_compaction_above_threshold() {
        let budget = ContextBudget {
            limit: 100,
            reply_reserve: 10,
        };
        // input budget = 90 tokens = 360 chars. 80% = 72 tokens = 288 chars.
        // Create a user message with enough text to exceed 288 chars.
        let big_msg = ChatCompletionMessage::User(UserMessage {
            content: UserContent::String("x".repeat(400)),
            name: None,
        });
        let msgs = vec![big_msg];
        let tools: Vec<ChatCompletionTool> = vec![];
        assert!(needs_compaction(&budget, &msgs, &tools));
    }

    #[test]
    fn render_transcript_basic() {
        let msgs = vec![
            ChatCompletionMessage::User(UserMessage {
                content: UserContent::String("hello".to_string()),
                name: None,
            }),
            ChatCompletionMessage::Assistant(crate::llm::AssistantMessage {
                content: Some(crate::llm::AssistantContent::String("hi there".to_string())),
                name: None,
                refusal: None,
                tool_calls: None,
                function_call: None,
                audio: None,
                reasoning_details: None,
            }),
        ];
        let transcript = render_transcript(&msgs);
        assert!(transcript.contains("[User]: hello"));
        assert!(transcript.contains("[Assistant]: hi there"));
    }
}
