// ---------------------------------------------------------------------------
// Subagents — parallel child agents spawned via the `start_subagent` tool
//
// How it works:
// - Each `agent::run` owns a `SubagentInbox` (a `mpsc::UnboundedReceiver` of
//   `SubagentResult`s) and a `pending` counter.
// - The `StartSubagentTool` in that agent's registry holds the matching
//   `UnboundedSender` + counter. When the LLM calls it, the tool spawns a
//   tokio task that runs a fresh `agent::run` against its own `Conversation`
//   (persisted under `<parent_conv>/subagent/<subagent_id>/`) and returns
//   an immediate "started" tool result.
// - When the child agent finishes, the spawned task sends its final reply
//   back via the channel AND emits a `SubagentCompleted` event on the
//   spawning agent's event sink.
// - The parent's `agent::run` drains the inbox at the top of every loop
//   iteration (injecting results as user messages). When the LLM produces no
//   tool calls, the parent waits on `inbox.recv().await` if `pending > 0`
//   instead of returning — so the agent loop naturally stays alive until
//   every in-flight subagent has reported back.
// - Subagents recurse: each child creates its own inbox + `StartSubagentTool`,
//   so grandchildren's results funnel back to their direct parent, not the
//   top-level caller.
// ---------------------------------------------------------------------------

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::agent::EventSink;
use crate::llm::{ChatBackend, Reasoning};
use crate::tools::{StartSubagentTool, ToolRegistry};

pub mod registry;
pub use registry::{RegistryEntry, RegistryView, SubagentRegistry};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubagentStatus {
    Completed,
    Failed,
}

impl SubagentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Completed => "COMPLETED",
            Self::Failed => "FAILED",
        }
    }
}

/// The final message a subagent task sends back to its parent agent's inbox.
#[derive(Debug)]
pub struct SubagentResult {
    pub id: String,
    pub task: String,
    pub status: SubagentStatus,
    pub output: String,
}

/// Receiver side of an agent's subagent inbox. Held by `agent::run`.
pub struct SubagentInbox {
    rx: UnboundedReceiver<SubagentResult>,
    /// Count of subagents that have been spawned but whose results the parent
    /// agent has not yet drained from the inbox. Used by `agent::run` to
    /// decide whether to wait for more results before returning.
    pub(crate) pending: Arc<AtomicUsize>,
}

impl SubagentInbox {
    /// Number of subagents still in flight (spawned but not yet consumed).
    pub fn pending(&self) -> usize {
        self.pending.load(Ordering::Acquire)
    }

    /// Shared reference to the pending counter. Needed by callers outside
    /// this crate (e.g. the server's continuation loop) that pass it to
    /// `inject_subagent_result`.
    pub fn pending_counter(&self) -> &Arc<AtomicUsize> {
        &self.pending
    }

    /// Drain every result currently queued without blocking.
    pub fn try_drain(&mut self) -> Vec<SubagentResult> {
        let mut out = Vec::new();
        while let Ok(r) = self.rx.try_recv() {
            out.push(r);
        }
        out
    }

    /// Block until the next subagent result arrives (or every sender is dropped).
    pub async fn recv(&mut self) -> Option<SubagentResult> {
        self.rx.recv().await
    }
}

/// Everything an agent needs to spawn subagents. Cloned into each
/// `StartSubagentTool` instance and passed down to each spawned child.
#[derive(Clone)]
pub struct SubagentContext {
    pub chat_client: Arc<dyn ChatBackend>,
    /// Base tools available to every subagent (WITHOUT `start_subagent` — that
    /// gets added per-level so each level has its own inbox).
    pub base_tools: Arc<ToolRegistry>,
    /// Event sink of the agent spawning this subagent. The subagent's own
    /// events get wrapped as `SubagentEvent { subagent_id, inner }` and
    /// forwarded to this sink.
    pub parent_event_sink: Option<EventSink>,
    pub depth: usize,
    /// Direct parent's subagent id, or `None` for the top-level agent. Used
    /// to record the spawn tree in [`SubagentRegistry`].
    pub parent_id: Option<String>,
    pub model: Option<String>,
    pub reasoning: Option<Reasoning>,
    /// Where THIS agent's subagent conversations should be written.
    /// Each spawned subagent's conversation lives at
    /// `<subagent_root>/<subagent_id>.{json,jsonl}` and its own subagents
    /// (grandchildren) live under `<subagent_root>/<subagent_id>/subagent/`.
    pub subagent_root: PathBuf,
    /// Per-conversation scratch directory. Shared with all subagents so
    /// every level writes temp files into the same conversation-scoped
    /// space rather than `/tmp`.
    pub scratch_dir: Option<PathBuf>,
    /// Process-wide registry of running subagents. Tests construct their own
    /// to avoid sharing state across runs; production passes
    /// `SubagentRegistry::global().clone()`.
    pub registry: Arc<SubagentRegistry>,
}

// ---------------------------------------------------------------------------
// Equipping an agent with subagent support
// ---------------------------------------------------------------------------

/// Wire up a `StartSubagentTool` on top of the base tool registry and return
/// the matching inbox. Call this once per `agent::run` that should be allowed
/// to spawn subagents.
pub fn equip(ctx: SubagentContext) -> (SubagentInbox, Arc<ToolRegistry>) {
    let (tx, rx) = mpsc::unbounded_channel::<SubagentResult>();
    let pending = Arc::new(AtomicUsize::new(0));
    let concurrent = Arc::new(AtomicUsize::new(0));

    let tool = StartSubagentTool::new(
        tx,
        Arc::clone(&pending),
        Arc::clone(&concurrent),
        ctx.clone(),
    );

    let registry = ctx.base_tools.with_tool(Arc::new(tool));
    (SubagentInbox { rx, pending }, Arc::new(registry))
}

// ---------------------------------------------------------------------------
// Plumbing used by StartSubagentTool
// ---------------------------------------------------------------------------

pub(crate) type SubagentSender = UnboundedSender<SubagentResult>;
