// ---------------------------------------------------------------------------
// SubagentRegistry — process-wide map of every live subagent.
//
// Every `start_subagent` call inserts an entry here; the spawned task removes
// it when it finishes. The handle is held so a future cancellation API can
// `abort()` it — dropping the handle on its own does not cancel the task.
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use tokio::task::JoinHandle;

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

pub struct SubagentRegistry {
    inner: Mutex<HashMap<String, RegistryEntry>>,
}

impl SubagentRegistry {
    /// Construct an empty registry. Production code should use [`global`];
    /// tests construct their own so they don't share state.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
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

    pub fn insert(&self, entry: RegistryEntry) {
        self.lock().insert(entry.id.clone(), entry);
    }

    pub fn remove(&self, id: &str) -> Option<RegistryEntry> {
        self.lock().remove(id)
    }

    pub fn get(&self, id: &str) -> Option<RegistryView> {
        self.lock().get(id).map(RegistryView::from)
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
            });
        }
        let snap = reg.snapshot();
        assert_eq!(snap.len(), 3);
    }
}
