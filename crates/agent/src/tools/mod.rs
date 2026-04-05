mod addition;
mod bash;
mod glob_tool;
mod grep;
pub mod handler;
mod read;
mod write;

pub use addition::AdditionTool;
pub use bash::BashTool;
pub use glob_tool::GlobTool;
pub use grep::GrepTool;
pub use read::ReadTool;
pub use write::WriteTool;

use serde_json::Value as JsonValue;

use crate::llm::ChatCompletionTool;

// ---------------------------------------------------------------------------
// Tool trait — implement this for each tool the agent can call
// ---------------------------------------------------------------------------

pub trait Tool: Send + Sync {
    /// Unique name matching the function name sent to the LLM.
    fn name(&self) -> &str;

    /// Return the tool definition that gets sent in the request.
    fn definition(&self) -> ChatCompletionTool;

    /// Execute the tool with the raw JSON arguments string from the model.
    fn call(&self, arguments: &str) -> Result<JsonValue, ToolError>;
}

#[derive(Debug)]
pub struct ToolError(pub String);

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ToolError {}

// ---------------------------------------------------------------------------
// Registry — thin collection of tools for the agent loop
// ---------------------------------------------------------------------------

pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    pub fn register(mut self, tool: impl Tool + 'static) -> Self {
        self.tools.push(Box::new(tool));
        self
    }

    /// Tool definitions to include in the chat completion request.
    pub fn definitions(&self) -> Vec<ChatCompletionTool> {
        self.tools.iter().map(|t| t.definition()).collect()
    }

    /// Look up a tool by name and call it.
    pub fn call(&self, name: &str, arguments: &str) -> Result<JsonValue, ToolError> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.name() == name)
            .ok_or_else(|| ToolError(format!("unknown tool: {name}")))?;
        tool.call(arguments)
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
