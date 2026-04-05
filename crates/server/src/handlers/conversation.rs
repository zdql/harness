// ---------------------------------------------------------------------------
// handlers::conversation — Conversation CRUD + agent send handlers
// ---------------------------------------------------------------------------

use std::sync::Arc;

use crate::rpc::methods::conversation::{
    CreateParams, CreateResult, GetParams, GetResult, ListParams, ListResult,
    ConversationSummary, MessageEntry, SwitchParams, SwitchResult, SendParams,
    SendResult, ToolCallInfo,
};
use crate::ServerState;
use agent::conversation::{self, summarize, Conversation};
use storage::fs::FsStore;
use storage::ConversationStore;

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
    storage::settings::write(&settings)
        .map_err(|e| format!("failed to write settings: {e}"))?;

    Ok(CreateResult { id })
}

/// Handle `conversation.list` — list conversations with >1 message, most recent first.
pub fn list(_params: ListParams) -> Result<ListResult, String> {
    let store = store()?;
    let ids = store.list().map_err(|e| format!("failed to list: {e}"))?;

    let mut conversations = Vec::new();
    for id in ids {
        let meta = store.load_metadata(&id).map_err(|e| format!("failed to load metadata: {e}"))?;
        let msgs = store.load_messages(&id).map_err(|e| format!("failed to load messages: {e}"))?;

        // Skip conversations that never had a real exchange
        if msgs.len() <= 1 {
            continue;
        }

        let title = meta["title"].as_str().map(String::from);
        conversations.push(ConversationSummary { id, title });
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
    storage::settings::write(&settings)
        .map_err(|e| format!("failed to write settings: {e}"))?;

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

/// Handle `conversation.send` — send a user message through the agent loop.
///
/// This is the core integration point: it loads the conversation from disk,
/// runs the agent loop (LLM calls + tool execution), persists the result,
/// and returns the assistant's final text reply.
pub async fn send(
    params: SendParams,
    state: &Arc<ServerState>,
    events: Option<agent::agent::EventSink>,
) -> Result<SendResult, String> {
    let store = store()?;

    // Load the conversation (or error if it doesn't exist)
    let mut conv = conversation::load(&store, &params.id)
        .map_err(|e| format!("failed to load conversation: {e}"))?;

    // Ensure the conversation has a model set
    if conv.model.is_none() {
        let settings = storage::settings::read();
        conv.model = settings.model.or_else(|| {
            Some(agent::prompts::DEFAULT_MODEL.to_string())
        });
    }

    // Run the agent loop — this calls the LLM, executes tools, and loops.
    // Events stream out to the frontend while the loop runs.
    let result = agent::agent::run_with_events(
        &state.chat_client,
        &store,
        &mut conv,
        Arc::clone(&state.tools),
        &params.message,
        events.as_ref(),
    )
    .await
    .map_err(|e| format!("agent error: {e}"))?;

    // Spawn background summarization if needed
    if conv.needs_summarization() {
        let client = state.chat_client.clone();
        let conv_clone = conv.clone();
        tokio::spawn(async move {
            match summarize::summarize(&client, &conv_clone).await {
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

    Ok(SendResult {
        reply: result.reply,
        tool_calls: result.tool_calls.into_iter().map(|tc| ToolCallInfo {
            name: tc.name,
            arguments: tc.arguments,
            result: tc.result,
        }).collect(),
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn uuid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{:x}-{:x}", d.as_secs(), d.subsec_nanos())
}
