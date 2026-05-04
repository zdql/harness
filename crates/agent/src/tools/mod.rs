mod addition;
mod bash;
mod edit;
mod glob_tool;
mod grep;
pub mod handler;
mod read;
mod start_subagent;
mod write;

pub use addition::AdditionTool;
pub use bash::BashTool;
pub use edit::EditTool;
pub use glob_tool::GlobTool;
pub use grep::GrepTool;
pub use read::ReadTool;
pub use start_subagent::StartSubagentTool;
pub use write::WriteTool;

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value as JsonValue;

use crate::llm::ChatCompletionTool;

// ---------------------------------------------------------------------------
// Tool trait — implement this for each tool the agent can call
//
// Tools are async so they can spawn tokio tasks (e.g. `start_subagent`),
// perform async I/O, or propagate cancellation cleanly. Tools whose bodies
// are CPU- or blocking-I/O-bound MUST wrap their sync work in
// `tokio::task::spawn_blocking` (via the `run_blocking` helper) so they
// don't starve the async worker pool.
// ---------------------------------------------------------------------------

#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique name matching the function name sent to the LLM.
    fn name(&self) -> &str;

    /// Return the tool definition that gets sent in the request.
    fn definition(&self) -> ChatCompletionTool;

    /// Execute the tool with the raw JSON arguments string from the model.
    async fn call(&self, arguments: &str) -> Result<JsonValue, ToolError>;
}

#[derive(Debug)]
pub struct ToolError(pub String);

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ToolError {}

/// Run a synchronous, blocking closure on tokio's blocking pool and await its
/// result. Use this in tool implementations whose work is CPU-bound or uses
/// blocking-I/O APIs (std::fs, std::process, regex over large files, …).
pub async fn run_blocking<F, T>(f: F) -> Result<T, ToolError>
where
    F: FnOnce() -> Result<T, ToolError> + Send + 'static,
    T: Send + 'static,
{
    match tokio::task::spawn_blocking(f).await {
        Ok(res) => res,
        Err(e) => Err(ToolError(format!("tool task join failed: {e}"))),
    }
}

// ---------------------------------------------------------------------------
// Registry — thin collection of tools for the agent loop
// ---------------------------------------------------------------------------

pub struct ToolRegistry {
    tools: Vec<Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    pub fn register(mut self, tool: impl Tool + 'static) -> Self {
        self.tools.push(Arc::new(tool));
        self
    }

    pub fn register_arc(mut self, tool: Arc<dyn Tool>) -> Self {
        self.tools.push(tool);
        self
    }

    /// Return a new registry containing this registry's tools plus `tool`.
    /// Used to attach per-run state (e.g. `start_subagent`) on top of the
    /// stateless base registry held in `ServerState`.
    pub fn with_tool(&self, tool: Arc<dyn Tool>) -> ToolRegistry {
        let mut tools = self.tools.clone();
        tools.push(tool);
        ToolRegistry { tools }
    }

    /// Tool definitions to include in the chat completion request.
    pub fn definitions(&self) -> Vec<ChatCompletionTool> {
        self.tools.iter().map(|t| t.definition()).collect()
    }

    /// Look up a tool by name and call it.
    pub async fn call(&self, name: &str, arguments: &str) -> Result<JsonValue, ToolError> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.name() == name)
            .ok_or_else(|| ToolError(format!("unknown tool: {name}")))?;
        tool.call(arguments).await
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
