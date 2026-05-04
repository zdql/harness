use crate::llm::ChatError;

#[derive(Debug)]
pub enum RunError<SE: std::error::Error> {
    Chat(ChatError),
    Storage(SE),
}

impl<SE: std::error::Error> std::fmt::Display for RunError<SE> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Chat(e) => write!(f, "{e}"),
            Self::Storage(e) => write!(f, "Storage: {e}"),
        }
    }
}

impl<SE: std::error::Error> std::error::Error for RunError<SE> {}

impl<SE: std::error::Error> From<ChatError> for RunError<SE> {
    fn from(e: ChatError) -> Self {
        Self::Chat(e)
    }
}

#[derive(Debug, Clone)]
pub struct ToolCallInfo {
    pub name: String,
    pub arguments: String,
    pub result: String,
}

#[derive(Debug)]
pub struct RunResult {
    pub reply: String,
    pub tool_calls: Vec<ToolCallInfo>,
}

#[derive(Debug)]
pub enum RunOutcome {
    /// The model finished and there are no pending subagents.
    Done(RunResult),
    /// The model produced a text reply but subagents are still in flight.
    /// The caller should return this reply to the user immediately, then
    /// watch the subagent inbox and call `resume()` when results arrive.
    Suspended {
        result: RunResult,
        pending_subagents: usize,
    },
}
