use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Settings — flat JSON file at ~/.agent-harness/settings.json
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub conversation: Option<String>,
    /// Reasoning effort sent to thinking models. One of:
    /// `"off"` (disable), `"none"`, `"minimal"`, `"low"`, `"medium"`, `"high"`,
    /// `"xhigh"`. `None` = use default from prompts.
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    /// Reasoning summary verbosity. One of: `"auto"`, `"concise"`, `"detailed"`.
    /// `None` = use default from prompts.
    #[serde(default)]
    pub reasoning_summary: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            model: None,
            conversation: None,
            reasoning_effort: None,
            reasoning_summary: None,
        }
    }
}

fn settings_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".agent-harness").join("settings.json")
}

/// Read current settings from disk. Returns defaults if the file doesn't exist.
pub fn read() -> Settings {
    let path = settings_path();
    match fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

/// Write settings to disk.
pub fn write(settings: &Settings) -> Result<(), std::io::Error> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(settings)?;
    fs::write(&path, data)
}
