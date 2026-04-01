// ---------------------------------------------------------------------------
// handlers::conversation — Conversation CRUD handlers
// ---------------------------------------------------------------------------

use crate::rpc::methods::conversation::{
    CreateParams, CreateResult, ListParams, ListResult, ConversationSummary,
    SwitchParams, SwitchResult,
};
use storage::fs::FsStore;
use storage::ConversationStore;

fn store() -> Result<FsStore, String> {
    FsStore::new().map_err(|e| format!("failed to open store: {e}"))
}

/// Handle `conversation.create` — create a new empty conversation.
pub fn create(_params: CreateParams) -> Result<CreateResult, String> {
    let store = store()?;
    let id = uuid();

    let metadata = serde_json::json!({
        "id": id,
        "created_at": now_iso(),
        "updated_at": now_iso(),
    });

    store
        .save_metadata(&id, &metadata)
        .map_err(|e| format!("failed to save metadata: {e}"))?;

    // Also persist as the active conversation in settings
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
    // Verify the conversation exists
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

fn now_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs();
    // Simple ISO-ish timestamp without pulling in chrono
    format!("{secs}")
}
