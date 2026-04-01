use std::fs;
use std::io::{BufRead, Write};
use std::path::PathBuf;

use serde_json::Value;

use crate::ConversationStore;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum FsError {
    Io(std::io::Error),
    Json(serde_json::Error),
    NotFound(String),
}

impl std::fmt::Display for FsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::NotFound(id) => write!(f, "Conversation not found: {id}"),
        }
    }
}

impl std::error::Error for FsError {}

impl From<std::io::Error> for FsError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for FsError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

// ---------------------------------------------------------------------------
// Filesystem store — saves to ~/.agent-harness/conversations/
// ---------------------------------------------------------------------------

pub struct FsStore {
    dir: PathBuf,
}

impl FsStore {
    /// Create a store rooted at `~/.agent-harness/conversations/`.
    /// Creates the directory if it doesn't exist.
    pub fn new() -> Result<Self, FsError> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let dir = PathBuf::from(home).join(".agent-harness").join("conversations");
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    /// Create a store at a custom path (useful for testing).
    pub fn with_dir(dir: PathBuf) -> Result<Self, FsError> {
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    fn meta_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    fn messages_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.jsonl"))
    }
}

impl ConversationStore for FsStore {
    type Error = FsError;

    fn list(&self) -> Result<Vec<String>, FsError> {
        let mut entries: Vec<(String, std::time::SystemTime)> = Vec::new();

        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let modified = entry.metadata()?.modified()?;
                    entries.push((stem.to_string(), modified));
                }
            }
        }

        // Most recently modified first.
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        Ok(entries.into_iter().map(|(id, _)| id).collect())
    }

    fn load_metadata(&self, id: &str) -> Result<Value, FsError> {
        let path = self.meta_path(id);
        if !path.exists() {
            return Err(FsError::NotFound(id.to_string()));
        }
        let data = fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&data)?)
    }

    fn load_messages(&self, id: &str) -> Result<Vec<Value>, FsError> {
        let path = self.messages_path(id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file = fs::File::open(&path)?;
        let reader = std::io::BufReader::new(file);
        let mut messages = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if !line.trim().is_empty() {
                messages.push(serde_json::from_str(&line)?);
            }
        }
        Ok(messages)
    }

    fn save_metadata(&self, id: &str, metadata: &Value) -> Result<(), FsError> {
        let path = self.meta_path(id);
        let data = serde_json::to_string_pretty(metadata)?;
        fs::write(&path, data)?;
        Ok(())
    }

    fn append_message(&self, id: &str, message: &Value) -> Result<(), FsError> {
        let path = self.messages_path(id);
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        let line = serde_json::to_string(message)?;
        writeln!(file, "{line}")?;
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<(), FsError> {
        let meta = self.meta_path(id);
        let msgs = self.messages_path(id);
        if meta.exists() {
            fs::remove_file(&meta)?;
        }
        if msgs.exists() {
            fs::remove_file(&msgs)?;
        }
        Ok(())
    }
}
