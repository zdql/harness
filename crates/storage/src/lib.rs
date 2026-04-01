pub mod fs;

use serde_json::Value;

// ---------------------------------------------------------------------------
// Storage trait — the interface everything else accepts
// ---------------------------------------------------------------------------

/// Minimal storage interface for conversations.
///
/// Conversations are stored as:
///   - `<id>.json`  — metadata envelope (id, model, created_at, updated_at)
///   - `<id>.jsonl` — append-only message log, one JSON object per line
pub trait ConversationStore {
    type Error: std::error::Error + Send + Sync + 'static;

    /// List conversation IDs, most-recently-updated first.
    fn list(&self) -> Result<Vec<String>, Self::Error>;

    /// Load the metadata envelope for a conversation.
    fn load_metadata(&self, id: &str) -> Result<Value, Self::Error>;

    /// Load all messages for a conversation (the .jsonl rows).
    fn load_messages(&self, id: &str) -> Result<Vec<Value>, Self::Error>;

    /// Save/overwrite the metadata envelope.
    fn save_metadata(&self, id: &str, metadata: &Value) -> Result<(), Self::Error>;

    /// Append a single message to the message log.
    fn append_message(&self, id: &str, message: &Value) -> Result<(), Self::Error>;

    /// Delete a conversation (both files).
    fn delete(&self, id: &str) -> Result<(), Self::Error>;
}
