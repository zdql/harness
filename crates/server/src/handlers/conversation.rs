// ---------------------------------------------------------------------------
// handlers::conversation — Conversation CRUD + agent send handlers
// ---------------------------------------------------------------------------

use crate::rpc::methods::conversation::{
    CreateParams, CreateResult, ListParams, ListResult, ConversationSummary,
    SwitchParams, SwitchResult, SendParams, SendResult,
};
use crate::ServerState;
use agent::conversation::{self, Conversation};
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

/// Handle `conversation.list` — list all conversations, most recent first.
pub fn list(_params: ListParams) -> Result<ListResult, String> {
    let store = store()?;
    let ids = store.list().map_err(|e| format!("failed to list: {e}"))?;
    let conversations = ids
        .into_iter()
        .map(|id| ConversationSummary { id })
        .collect();
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

/// Handle `conversation.send` — send a user message through the agent loop.
///
/// This is the core integration point: it loads the conversation from disk,
/// runs the agent loop (LLM calls + tool execution), persists the result,
/// and returns the assistant's final text reply.
pub async fn send(params: SendParams, state: &ServerState) -> Result<SendResult, String> {
    let store = store()?;

    // Load the conversation (or error if it doesn't exist)
    let mut conv = conversation::load(&store, &params.id)
        .map_err(|e| format!("failed to load conversation: {e}"))?;

    // Ensure the conversation has a model set
    if conv.model.is_none() {
        let settings = storage::settings::read();
        conv.model = settings.model.or_else(|| {
            Some("openai/gpt-4o-mini".to_string()) // sensible default
        });
    }

    // Run the agent loop — this calls the LLM, executes tools, and loops
    let reply = agent::agent::run(
        &state.chat_client,
        &store,
        &mut conv,
        &state.tools,
        &params.message,
    )
    .await
    .map_err(|e| format!("agent error: {e}"))?;

    Ok(SendResult { reply })
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
