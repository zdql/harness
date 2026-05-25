//! JSON-RPC protocol types and log helpers used by the Harness backend.

use serde::Deserialize;
use serde_json::Value;

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

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RpcLogContext<'a> {
    pub(crate) conversation_id: Option<&'a str>,
    pub(crate) job_id: Option<&'a str>,
}
