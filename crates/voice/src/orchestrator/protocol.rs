use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── RPC protocol types ──────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct RpcResponse {
    pub(crate) result: Option<Value>,
    pub(crate) error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RpcError {
    pub(crate) message: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateResult {
    pub(crate) id: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct SendResult {
    pub(crate) reply: String,
    #[serde(default)]
    pub(crate) tool_calls: Vec<ToolCallInfo>,
    #[serde(default)]
    pub(crate) suspended: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct ToolCallInfo {
    pub(crate) name: String,
    pub(crate) arguments: String,
    pub(crate) result: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RpcLogContext<'a> {
    pub(crate) conversation_id: Option<&'a str>,
    pub(crate) job_id: Option<&'a str>,
}

// ── Shared formatting / logging utilities ───────────────────────────

pub(crate) fn format_job(job_id: Option<&str>) -> String {
    job_id.map(|id| format!(" job={id}")).unwrap_or_default()
}

pub(crate) fn preview(value: &str) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut preview = compact.chars().take(160).collect::<String>();
    if compact.chars().count() > 160 {
        preview.push_str("...");
    }
    preview
}

pub(crate) fn compact_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "<unserializable>".to_string())
}
