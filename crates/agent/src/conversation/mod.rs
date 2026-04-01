use serde::{Deserialize, Serialize};

use crate::llm::{ChatCompletionMessage, UserContent, UserMessage};

// ---------------------------------------------------------------------------
// Conversation — the core object that core.rs operates on
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub model: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub messages: Vec<ChatCompletionMessage>,
}

impl Conversation {
    pub fn new(id: impl Into<String>) -> Self {
        let now = now_unix();
        Self {
            id: id.into(),
            model: None,
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Push a plain-text user message.
    pub fn push_user(&mut self, text: &str) {
        self.messages.push(ChatCompletionMessage::User(UserMessage {
            content: UserContent::String(text.to_string()),
            name: None,
        }));
        self.updated_at = now_unix();
    }

    /// Push an already-formed message (e.g. assistant reply converted from response).
    pub fn push_message(&mut self, msg: ChatCompletionMessage) {
        self.messages.push(msg);
        self.updated_at = now_unix();
    }

    /// Metadata envelope for storage (everything except messages).
    pub fn metadata(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "model": self.model,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
        })
    }
}

// ---------------------------------------------------------------------------
// Persistence helpers — work with any ConversationStore
// ---------------------------------------------------------------------------

use storage::ConversationStore;

/// Save a conversation: overwrite metadata, append only the *last* message.
pub fn save<S: ConversationStore>(
    store: &S,
    conv: &Conversation,
) -> Result<(), S::Error> {
    store.save_metadata(&conv.id, &conv.metadata())?;
    if let Some(last) = conv.messages.last() {
        let val = serde_json::to_value(last).expect("message serializes");
        store.append_message(&conv.id, &val)?;
    }
    Ok(())
}

/// Load a conversation from storage by id.
pub fn load<S: ConversationStore>(
    store: &S,
    id: &str,
) -> Result<Conversation, S::Error> {
    let meta = store.load_metadata(id)?;
    let raw_msgs = store.load_messages(id)?;

    let messages: Vec<ChatCompletionMessage> = raw_msgs
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();

    Ok(Conversation {
        id: meta["id"].as_str().unwrap_or(id).to_string(),
        model: meta["model"].as_str().map(String::from),
        created_at: meta["created_at"].as_i64().unwrap_or(0),
        updated_at: meta["updated_at"].as_i64().unwrap_or(0),
        messages,
    })
}

/// Load the most recent conversation, or None if nothing is stored.
pub fn load_latest<S: ConversationStore>(
    store: &S,
) -> Result<Option<Conversation>, S::Error> {
    let ids = store.list()?;
    match ids.first() {
        Some(id) => Ok(Some(load(store, id)?)),
        None => Ok(None),
    }
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
