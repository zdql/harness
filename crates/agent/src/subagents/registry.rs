// ---------------------------------------------------------------------------
// SubagentRegistry — process-wide map of every live subagent.
//
// Every `start_subagent` call inserts an entry here; the spawned task removes
// it when it finishes. The handle is held so a future cancellation API can
// `abort()` it — dropping the handle on its own does not cancel the task.
//
// Extended with progress tracking: checkpoints, recent log entries, and
// provider-specific update hooks used by `check_subagent_progress`.
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use tokio::task::JoinHandle;

// ---------------------------------------------------------------------------
// Progress tracking types
// ---------------------------------------------------------------------------

/// A lightweight marker emitted during subagent execution.
#[derive(Debug, Clone)]
pub struct Checkpoint {
    pub label: String,
    pub timestamp: SystemTime,
}

/// A single log entry captured from a `ToolCallEnd` or `Checkpoint` event.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub summary: String,
    pub timestamp: SystemTime,
}

/// Bounded ring buffer that holds the most recent `N` entries, evicting the
/// oldest when full. Used for `recent_logs` in the registry.
#[derive(Debug, Clone)]
pub struct RingBuffer<T> {
    buf: Vec<T>,
    cap: usize,
    head: usize, // index of the oldest element
    len: usize,
}

impl<T> RingBuffer<T> {
    pub fn new(cap: usize) -> Self {
        Self {
            buf: Vec::with_capacity(cap),
            cap,
            head: 0,
            len: 0,
        }
    }

    pub fn push(&mut self, item: T) {
        if self.cap == 0 {
            return;
        }
        if self.len < self.cap {
            self.buf.push(item);
            self.len += 1;
        } else {
            self.buf[self.head] = item;
            self.head = (self.head + 1) % self.cap;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn len(&self) -> usize {
        self.len
    }

    /// Iterate from oldest to newest.
    pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
        (0..self.len).map(move |i| &self.buf[(self.head + i) % self.len.max(1)])
    }
}

// ---------------------------------------------------------------------------
// ProgressProvider — provider-specific hook for `get_update`
// ---------------------------------------------------------------------------

/// Provider-specific progress data returned by `get_update`.
#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    /// Bounded recent output (e.g. last few lines of stdout or the most
    /// recent orchestrator message). Truncated to a safe size by the caller.
    pub recent_output: Option<String>,
    /// A single most-recent message from the orchestrator (for Harness: the
    /// last content delta or tool-call-end summary; for Codex: the last line
    /// of stdout).
    pub last_message: Option<String>,
}

impl ProgressUpdate {
    /// An empty update for providers that don't support mid-flight progress.
    pub fn empty() -> Self {
        Self {
            recent_output: None,
            last_message: None,
        }
    }
}

/// Trait for provider-specific progress retrieval. Implemented by the voice
/// crate's Harness and Codex providers; the agent crate provides a default
/// that reads from the registry.
pub trait ProgressProvider: Send + Sync {
    /// Return the most recent progress data for the given subagent.
    /// Implementations must be safe to call from any thread at any time.
    fn get_update(&self, subagent_id: &str) -> ProgressUpdate;
}

/// A no-op provider that always returns empty updates. Used when no
/// provider-specific hook is available.
pub struct NullProgressProvider;

impl ProgressProvider for NullProgressProvider {
    fn get_update(&self, _subagent_id: &str) -> ProgressUpdate {
        ProgressUpdate::empty()
    }
}

/// A Harness-specific provider that reads the most recent orchestrator
/// message from the subagent registry. This is the default provider used
/// when the voice crate doesn't inject its own.
pub struct HarnessProgressProvider {
    pub registry: Arc<SubagentRegistry>,
}

impl ProgressProvider for HarnessProgressProvider {
    fn get_update(&self, subagent_id: &str) -> ProgressUpdate {
        let entry = self.registry.get_progress(subagent_id);
        match entry {
            Some(p) => {
                let last_log = p.recent_logs.last().map(|e| e.summary.clone());
                let recent: Vec<String> = p.recent_logs.iter().map(|e| e.summary.clone()).collect();
                ProgressUpdate {
                    recent_output: if recent.is_empty() {
                        None
                    } else {
                        Some(recent.join("\n"))
                    },
                    last_message: last_log,
                }
            }
            None => ProgressUpdate::empty(),
        }
    }
}

// ---------------------------------------------------------------------------
// Live record for one running subagent
// ---------------------------------------------------------------------------

/// Live record for one running subagent.
pub struct RegistryEntry {
    pub id: String,
    /// Direct parent's subagent id, or `None` if spawned by the top-level agent.
    pub parent_id: Option<String>,
    pub task: String,
    pub depth: usize,
    pub started_at: SystemTime,
    /// Held to support a future `abort` API. Dropping does NOT cancel.
    pub handle: JoinHandle<()>,
    /// Progress checkpoints emitted during execution (bounded to last 10).
    pub checkpoints: Vec<Checkpoint>,
    /// Recent tool-call log entries (ring buffer, max 5).
    pub recent_logs: RingBuffer<LogEntry>,
    /// Total tool calls executed so far.
    pub tool_calls_made: usize,
}

/// Read-only snapshot of a registry entry, safe to clone and hand to callers
/// that just want to inspect what's running.
#[derive(Debug, Clone)]
pub struct RegistryView {
    pub id: String,
    pub parent_id: Option<String>,
    pub task: String,
    pub depth: usize,
    pub started_at: SystemTime,
}

/// Read-only progress snapshot extracted from a registry entry.
#[derive(Debug, Clone)]
pub struct ProgressView {
    pub checkpoints: Vec<Checkpoint>,
    pub recent_logs: Vec<LogEntry>,
    pub tool_calls_made: usize,
}

impl From<&RegistryEntry> for RegistryView {
    fn from(e: &RegistryEntry) -> Self {
        Self {
            id: e.id.clone(),
            parent_id: e.parent_id.clone(),
            task: e.task.clone(),
            depth: e.depth,
            started_at: e.started_at,
        }
    }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

pub struct SubagentRegistry {
    inner: Mutex<HashMap<String, RegistryEntry>>,
    /// Optional provider-specific progress hook. If `None`, the harness
    /// default (reading from the registry itself) is used.
    progress_provider: Option<Arc<dyn ProgressProvider>>,
}

impl SubagentRegistry {
    /// Construct an empty registry. Production code should use [`global`];
    /// tests construct their own so they don't share state.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            progress_provider: None,
        }
    }

    /// Test convenience: a fresh registry that doesn't share state with the
    /// process-wide one. Equivalent to `SubagentRegistry::new()` but the name
    /// makes intent obvious at call sites.
    pub fn global_for_test() -> Self {
        Self::new()
    }

    /// The single shared registry for this process.
    pub fn global() -> &'static Arc<SubagentRegistry> {
        static REGISTRY: OnceLock<Arc<SubagentRegistry>> = OnceLock::new();
        REGISTRY.get_or_init(|| Arc::new(SubagentRegistry::new()))
    }

    /// Attach a provider-specific progress hook. This is called by the voice
    /// crate when it creates the registry for a realtime session.
    pub fn set_progress_provider(&mut self, provider: Arc<dyn ProgressProvider>) {
        self.progress_provider = Some(provider);
    }

    pub fn insert(&self, entry: RegistryEntry) {
        self.lock().insert(entry.id.clone(), entry);
    }

    pub fn remove(&self, id: &str) -> Option<RegistryEntry> {
        self.lock().remove(id)
    }

    pub fn get(&self, id: &str) -> Option<RegistryView> {
        self.lock().get(id).map(RegistryView::from)
    }

    /// Get progress data for a specific subagent.
    pub fn get_progress(&self, id: &str) -> Option<ProgressView> {
        self.lock().get(id).map(|e| ProgressView {
            checkpoints: e.checkpoints.clone(),
            recent_logs: e.recent_logs.iter().cloned().collect(),
            tool_calls_made: e.tool_calls_made,
        })
    }

    /// Record a checkpoint in a subagent's registry entry.
    pub fn add_checkpoint(&self, id: &str, label: String) {
        let mut lock = self.lock();
        if let Some(entry) = lock.get_mut(id) {
            const MAX_CHECKPOINTS: usize = 10;
            entry.checkpoints.push(Checkpoint {
                label,
                timestamp: SystemTime::now(),
            });
            if entry.checkpoints.len() > MAX_CHECKPOINTS {
                let drain = entry.checkpoints.len() - MAX_CHECKPOINTS;
                entry.checkpoints.drain(0..drain);
            }
        }
    }

    /// Record a tool-call log entry in a subagent's registry entry.
    pub fn add_log(&self, id: &str, summary: String) {
        let mut lock = self.lock();
        if let Some(entry) = lock.get_mut(id) {
            entry.tool_calls_made += 1;
            entry.recent_logs.push(LogEntry {
                summary,
                timestamp: SystemTime::now(),
            });
        }
    }

    /// Invoke the provider-specific `get_update` hook, or fall back to the
    /// harness default (read from registry).
    pub fn get_update(&self, subagent_id: &str) -> ProgressUpdate {
        if let Some(ref provider) = self.progress_provider {
            provider.get_update(subagent_id)
        } else {
            // Default harness behavior: read from registry
            let entry = self.get_progress(subagent_id);
            match entry {
                Some(p) => {
                    let last_log = p.recent_logs.last().map(|e| e.summary.clone());
                    let recent: Vec<String> =
                        p.recent_logs.iter().map(|e| e.summary.clone()).collect();
                    ProgressUpdate {
                        recent_output: if recent.is_empty() {
                            None
                        } else {
                            Some(recent.join("\n"))
                        },
                        last_message: last_log,
                    }
                }
                None => ProgressUpdate::empty(),
            }
        }
    }

    pub fn snapshot(&self) -> Vec<RegistryView> {
        self.lock().values().map(RegistryView::from).collect()
    }

    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, RegistryEntry>> {
        self.inner.lock().expect("subagent registry mutex poisoned")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::task;

    #[tokio::test]
    async fn insert_remove_roundtrip() {
        let reg = SubagentRegistry::new();
        let handle = task::spawn(async {});
        reg.insert(RegistryEntry {
            id: "sub-a".to_string(),
            parent_id: None,
            task: "demo".to_string(),
            depth: 0,
            started_at: SystemTime::now(),
            handle,
            checkpoints: Vec::new(),
            recent_logs: RingBuffer::new(5),
            tool_calls_made: 0,
        });
        assert_eq!(reg.len(), 1);
        assert!(reg.get("sub-a").is_some());
        let removed = reg.remove("sub-a").expect("entry should exist");
        assert_eq!(removed.id, "sub-a");
        assert!(reg.is_empty());
    }

    #[tokio::test]
    async fn snapshot_lists_entries() {
        let reg = SubagentRegistry::new();
        for i in 0..3 {
            reg.insert(RegistryEntry {
                id: format!("sub-{i}"),
                parent_id: if i == 0 {
                    None
                } else {
                    Some(format!("sub-{}", i - 1))
                },
                task: format!("task-{i}"),
                depth: i,
                started_at: SystemTime::now(),
                handle: task::spawn(async {}),
                checkpoints: Vec::new(),
                recent_logs: RingBuffer::new(5),
                tool_calls_made: 0,
            });
        }
        let snap = reg.snapshot();
        assert_eq!(snap.len(), 3);
    }

    #[test]
    fn ring_buffer_evicts_oldest() {
        let mut buf: RingBuffer<i32> = RingBuffer::new(3);
        buf.push(1);
        buf.push(2);
        buf.push(3);
        assert_eq!(buf.len(), 3);
        buf.push(4); // evicts 1
        assert_eq!(buf.len(), 3);
        let vals: Vec<&i32> = buf.iter().collect();
        assert_eq!(vals, vec![&2, &3, &4]);
    }

    #[test]
    fn ring_buffer_empty() {
        let buf: RingBuffer<i32> = RingBuffer::new(5);
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
    }

    #[tokio::test]
    async fn add_checkpoint_and_log() {
        let reg = SubagentRegistry::new();
        let handle = task::spawn(async {});
        reg.insert(RegistryEntry {
            id: "sub-x".to_string(),
            parent_id: None,
            task: "test".to_string(),
            depth: 0,
            started_at: SystemTime::now(),
            handle,
            checkpoints: Vec::new(),
            recent_logs: RingBuffer::new(5),
            tool_calls_made: 0,
        });
        reg.add_checkpoint("sub-x", "files searched".to_string());
        reg.add_log("sub-x", "read src/main.rs (ok)".to_string());
        reg.add_log("sub-x", "edit src/lib.rs (ok)".to_string());

        let progress = reg.get_progress("sub-x").expect("entry should exist");
        assert_eq!(progress.checkpoints.len(), 1);
        assert_eq!(progress.recent_logs.len(), 2);
        assert_eq!(progress.tool_calls_made, 2);
    }

    #[tokio::test]
    async fn checkpoint_bounds_enforced() {
        let reg = SubagentRegistry::new();
        let handle = task::spawn(async {});
        reg.insert(RegistryEntry {
            id: "sub-y".to_string(),
            parent_id: None,
            task: "test".to_string(),
            depth: 0,
            started_at: SystemTime::now(),
            handle,
            checkpoints: Vec::new(),
            recent_logs: RingBuffer::new(5),
            tool_calls_made: 0,
        });
        for i in 0..15 {
            reg.add_checkpoint("sub-y", format!("checkpoint-{i}"));
        }
        let progress = reg.get_progress("sub-y").expect("entry should exist");
        assert_eq!(progress.checkpoints.len(), 10); // MAX_CHECKPOINTS
    }

    #[tokio::test]
    async fn get_update_returns_progress() {
        let reg = SubagentRegistry::new();
        let handle = task::spawn(async {});
        reg.insert(RegistryEntry {
            id: "sub-z".to_string(),
            parent_id: None,
            task: "test".to_string(),
            depth: 0,
            started_at: SystemTime::now(),
            handle,
            checkpoints: Vec::new(),
            recent_logs: RingBuffer::new(5),
            tool_calls_made: 0,
        });
        reg.add_log("sub-z", "read src/main.rs (ok)".to_string());

        let update = reg.get_update("sub-z");
        assert!(update.recent_output.is_some());
        assert!(update.last_message.is_some());
        assert_eq!(update.last_message.unwrap(), "read src/main.rs (ok)");
    }

    #[test]
    fn null_progress_provider_returns_empty() {
        let provider = NullProgressProvider;
        let update = provider.get_update("anything");
        assert!(update.recent_output.is_none());
        assert!(update.last_message.is_none());
    }
}
