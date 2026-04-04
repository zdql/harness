pub mod compaction;
pub mod summarize;

use serde::{Deserialize, Serialize};

use crate::llm::{ChatCompletionMessage, UserContent, UserMessage};

// ---------------------------------------------------------------------------
// Conversation — the core object that core.rs operates on
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub model: Option<String>,
    pub title: Option<String>,
    /// Message count when the title was last generated.
    #[serde(default)]
    pub title_set_at_message_count: usize,
    pub created_at: i64,
    pub updated_at: i64,
    pub messages: Vec<ChatCompletionMessage>,
    /// Absolute indices (into the on-disk .jsonl) where compaction boundary
    /// messages were inserted. The most recent entry is the active compaction.
    #[serde(default)]
    pub compaction_indices: Vec<usize>,
    /// The absolute index of the first element of `messages` relative to the
    /// full on-disk .jsonl. Zero when no compaction has occurred.
    #[serde(default)]
    pub message_offset: usize,
}

impl Conversation {
    pub fn new(id: impl Into<String>) -> Self {
        let now = now_unix();
        Self {
            id: id.into(),
            model: None,
            title: None,
            title_set_at_message_count: 0,
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
            compaction_indices: Vec::new(),
            message_offset: 0,
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

    /// The total number of messages on disk (offset + in-memory length).
    pub fn absolute_len(&self) -> usize {
        self.message_offset + self.messages.len()
    }

    /// Metadata envelope for storage (everything except messages).
    pub fn metadata(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "model": self.model,
            "title": self.title,
            "title_set_at_message_count": self.title_set_at_message_count,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
            "compaction_indices": self.compaction_indices,
            "message_offset": self.message_offset,
        })
    }

    /// Whether this conversation needs a (re-)summarization.
    /// First title after any messages, then re-title every 4 messages.
    pub fn needs_summarization(&self) -> bool {
        let count = self.messages.len();
        if count == 0 {
            return false;
        }
        if self.title.is_none() {
            return true;
        }
        count >= self.title_set_at_message_count + 4
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
///
/// If compactions have occurred, only messages from the most recent compaction
/// boundary onward are loaded into `messages`. The full history remains on
/// disk in the .jsonl file, untouched.
pub fn load<S: ConversationStore>(
    store: &S,
    id: &str,
) -> Result<Conversation, S::Error> {
    let meta = store.load_metadata(id)?;
    let raw_msgs = store.load_messages(id)?;

    let all_messages: Vec<ChatCompletionMessage> = raw_msgs
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();

    let compaction_indices: Vec<usize> = meta["compaction_indices"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_u64().map(|n| n as usize))
                .collect()
        })
        .unwrap_or_default();

    // Slice from the most recent compaction boundary, or load everything.
    let (messages, message_offset) = if let Some(&last_idx) = compaction_indices.last() {
        let offset = last_idx.min(all_messages.len());
        (all_messages[offset..].to_vec(), offset)
    } else {
        (all_messages, 0)
    };

    Ok(Conversation {
        id: meta["id"].as_str().unwrap_or(id).to_string(),
        model: meta["model"].as_str().map(String::from),
        title: meta["title"].as_str().map(String::from),
        title_set_at_message_count: meta["title_set_at_message_count"].as_u64().unwrap_or(0) as usize,
        created_at: meta["created_at"].as_i64().unwrap_or(0),
        updated_at: meta["updated_at"].as_i64().unwrap_or(0),
        messages,
        compaction_indices,
        message_offset,
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
