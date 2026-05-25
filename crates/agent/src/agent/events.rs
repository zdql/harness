// Streaming events emitted by the agent loop. Frontends consume these to
// render live progress (thinking spinner, tool call previews, subagent
// activity) before the final response is ready.

/// An event emitted during the agent loop. Wire shape is a serde-tagged enum
/// so the server can forward these as JSON-RPC notifications directly.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentEvent {
    /// LLM request is in-flight.
    LlmStart,
    /// LLM request returned.
    LlmEnd,
    /// Incremental reasoning / thinking text from a thinking model.
    ReasoningDelta { text: String },
    /// Incremental assistant-visible content text.
    ContentDelta { text: String },
    /// A tool is about to execute.
    ToolCallStart { name: String, arguments: String },
    /// A tool finished executing.
    ToolCallEnd {
        name: String,
        arguments: String,
        result: String,
    },
    /// A subagent has been spawned from this agent's `start_subagent` tool.
    SubagentStarted { subagent_id: String, task: String },
    /// A subagent spawned from this agent has finished.
    SubagentCompleted {
        subagent_id: String,
        status: String,
        output: String,
    },
    /// An event emitted by a subagent (or one of its own subagents). Nested
    /// `SubagentEvent`s form a tree mirroring the spawn hierarchy.
    SubagentEvent {
        subagent_id: String,
        inner: Box<AgentEvent>,
    },
    /// The agent loop resumed after a subagent completed and produced a new
    /// final reply. Emitted by the background continuation watcher.
    ContinuationDone { reply: String },
    /// A progress checkpoint emitted by the agent loop (or subagent). Tools
    /// like `check_subagent_progress` consume these to report intermediate
    /// status to the voice model or parent agent.
    Checkpoint { label: String },
}

/// A channel for streaming [`AgentEvent`]s out of the agent loop.
pub type EventSink = tokio::sync::mpsc::UnboundedSender<AgentEvent>;

pub(crate) fn emit(sink: Option<&EventSink>, event: AgentEvent) {
    if let Some(s) = sink {
        let _ = s.send(event);
    }
}
