use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;

/// Maximum number of progress entries retained per job.
const MAX_BUFFER_ENTRIES: usize = 20;

/// Minimum interval (in seconds) between progress queries for the same job
/// to avoid flooding the Realtime API.
const RATE_LIMIT_SECS: u64 = 2;

/// Default character window for the recent_snippet field.
pub(crate) const DEFAULT_WINDOW_SIZE: usize = 1000;

// ── Public types ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Unknown,
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        })
    }
}

/// Read-only snapshot returned to callers that query a job's progress.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProgressSnapshot {
    pub status: JobStatus,
    pub provider: String,
    pub last_message: String,
    pub recent_snippet: String,
}

// ── Internal tracking ──────────────────────────────────────────────────

struct JobProgress {
    status: JobStatus,
    provider: String,
    /// The most recent single progress line (e.g. last agent reply).
    last_message: String,
    /// Bounded ring of recent progress entries (newest last).
    recent_buffer: VecDeque<String>,
    last_updated: Instant,
}

struct ProgressInner {
    jobs: HashMap<String, JobProgress>,
    last_query: HashMap<String, Instant>,
}

// ── Store ──────────────────────────────────────────────────────────────

pub(crate) struct ProgressStore {
    inner: Mutex<ProgressInner>,
}

impl ProgressStore {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(ProgressInner {
                jobs: HashMap::new(),
                last_query: HashMap::new(),
            }),
        }
    }

    /// Register a new job in the progress store.
    pub fn register_job(&self, job_id: &str, provider: &str) {
        let mut inner = self.inner.lock().expect("progress store mutex poisoned");
        inner.jobs.insert(
            job_id.to_string(),
            JobProgress {
                status: JobStatus::Queued,
                provider: provider.to_string(),
                last_message: String::new(),
                recent_buffer: VecDeque::new(),
                last_updated: Instant::now(),
            },
        );
    }

    /// Mark a previously registered job as running.
    pub fn set_running(&self, job_id: &str) {
        let mut inner = self.inner.lock().expect("progress store mutex poisoned");
        if let Some(progress) = inner.jobs.get_mut(job_id) {
            progress.status = JobStatus::Running;
            progress.last_updated = Instant::now();
        }
    }

    /// Append a progress entry for a running job.
    /// The message is trimmed and empty strings are silently ignored.
    pub fn push_progress(&self, job_id: &str, message: &str) {
        let trimmed = message.trim();
        if trimmed.is_empty() {
            return;
        }
        let mut inner = self.inner.lock().expect("progress store mutex poisoned");
        if let Some(progress) = inner.jobs.get_mut(job_id) {
            progress.last_message = trimmed.to_string();
            if progress.recent_buffer.len() >= MAX_BUFFER_ENTRIES {
                progress.recent_buffer.pop_front();
            }
            progress.recent_buffer.push_back(trimmed.to_string());
            progress.last_updated = Instant::now();
        }
    }

    /// Mark a job as completed with the final reply.
    pub fn set_completed(&self, job_id: &str, result: &str) {
        let mut inner = self.inner.lock().expect("progress store mutex poisoned");
        if let Some(progress) = inner.jobs.get_mut(job_id) {
            progress.status = JobStatus::Completed;
            let trimmed = result.trim();
            if !trimmed.is_empty() {
                progress.last_message = trimmed.to_string();
                if progress.recent_buffer.len() >= MAX_BUFFER_ENTRIES {
                    progress.recent_buffer.pop_front();
                }
                progress.recent_buffer.push_back(trimmed.to_string());
            }
            progress.last_updated = Instant::now();
        }
    }

    /// Mark a job as failed with an error message.
    pub fn set_failed(&self, job_id: &str, error: &str) {
        let mut inner = self.inner.lock().expect("progress store mutex poisoned");
        if let Some(progress) = inner.jobs.get_mut(job_id) {
            progress.status = JobStatus::Failed;
            let trimmed = error.trim();
            if !trimmed.is_empty() {
                progress.last_message = trimmed.to_string();
            }
            progress.last_updated = Instant::now();
        }
    }

    /// Query the progress of a job. Returns `None` if the job is unknown.
    /// Applies rate limiting: returns a "rate_limited" snapshot if queried
    /// too frequently, containing only the status and provider fields.
    pub fn get_update(
        &self,
        job_id: &str,
        window_size: Option<usize>,
    ) -> Option<ProgressSnapshot> {
        let mut inner = self.inner.lock().expect("progress store mutex poisoned");

        let progress = inner.jobs.get(job_id)?;
        let status = progress.status;
        let provider = progress.provider.clone();
        let last_message = progress.last_message.clone();
        let recent_buffer = progress.recent_buffer.clone();

        // Rate limit check.
        let now = Instant::now();
        if let Some(last) = inner.last_query.get(job_id) {
            if now.duration_since(*last).as_secs() < RATE_LIMIT_SECS {
                // Return a minimal snapshot indicating rate limit.
                return Some(ProgressSnapshot {
                    status,
                    provider,
                    last_message: String::new(),
                    recent_snippet: format!(
                        "(rate limited — wait {}s between queries)",
                        RATE_LIMIT_SECS
                    ),
                });
            }
        }
        inner.last_query.insert(job_id.to_string(), now);

        let max_chars = window_size.unwrap_or(DEFAULT_WINDOW_SIZE);
        let recent_snippet = truncate_buffer(&recent_buffer, max_chars);

        Some(ProgressSnapshot {
            status,
            provider,
            last_message,
            recent_snippet,
        })
    }

    /// Remove a job from the store (e.g. after it has been completed for a
    /// while and memory should be reclaimed). Returns true if the job
    /// existed.
    pub fn remove(&self, job_id: &str) -> bool {
        let mut inner = self.inner.lock().expect("progress store mutex poisoned");
        inner.jobs.remove(job_id).is_some()
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

/// Concatenate buffer entries and truncate to `max_chars`, keeping the
/// tail (most recent) content.
fn truncate_buffer(buffer: &VecDeque<String>, max_chars: usize) -> String {
    if buffer.is_empty() {
        return String::new();
    }

    // Join all entries with newlines.
    let joined = buffer
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            if i == 0 {
                entry.clone()
            } else {
                format!("\n{entry}")
            }
        })
        .collect::<String>();

    truncate_tail(&joined, max_chars)
}

/// Truncate a string to at most `max_chars`, keeping the tail. If
/// truncated, prepends "…".
fn truncate_tail(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        return text.to_string();
    }

    // Work with char boundaries to avoid panicking on multi-byte UTF-8.
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars {
        return text.to_string();
    }

    let start = chars.len() - max_chars;
    let truncated: String = chars[start..].iter().collect();
    format!("…{truncated}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_update_job() {
        let store = ProgressStore::new();
        store.register_job("job-1", "harness");
        store.set_running("job-1");
        store.push_progress("job-1", "reading files");
        store.push_progress("job-1", "editing code");

        let snap = store.get_update("job-1", None).unwrap();
        assert_eq!(snap.status, JobStatus::Running);
        assert_eq!(snap.provider, "harness");
        assert_eq!(snap.last_message, "editing code");
        assert!(snap.recent_snippet.contains("reading files"));
        assert!(snap.recent_snippet.contains("editing code"));
    }

    #[test]
    fn completed_job_snapshot() {
        let store = ProgressStore::new();
        store.register_job("job-2", "codex");
        store.set_running("job-2");
        store.set_completed("job-2", "all done");

        let snap = store.get_update("job-2", None).unwrap();
        assert_eq!(snap.status, JobStatus::Completed);
        assert_eq!(snap.last_message, "all done");
    }

    #[test]
    fn failed_job_snapshot() {
        let store = ProgressStore::new();
        store.register_job("job-3", "harness");
        store.set_failed("job-3", "something went wrong");

        let snap = store.get_update("job-3", None).unwrap();
        assert_eq!(snap.status, JobStatus::Failed);
        assert_eq!(snap.last_message, "something went wrong");
    }

    #[test]
    fn unknown_job_returns_none() {
        let store = ProgressStore::new();
        assert!(store.get_update("no-such-job", None).is_none());
    }

    #[test]
    fn rate_limiting() {
        let store = ProgressStore::new();
        store.register_job("job-rl", "harness");
        store.set_running("job-rl");
        store.push_progress("job-rl", "working");

        // First query should succeed.
        let snap1 = store.get_update("job-rl", None).unwrap();
        assert!(!snap1.recent_snippet.contains("rate limited"));

        // Immediate second query should be rate limited.
        let snap2 = store.get_update("job-rl", None).unwrap();
        assert!(snap2.recent_snippet.contains("rate limited"));
        // Status should still be available.
        assert_eq!(snap2.status, JobStatus::Running);
    }

    #[test]
    fn truncation_keeps_tail() {
        let long = "x".repeat(500);
        let result = truncate_tail(&long, 100);
        assert_eq!(result.chars().count(), 101); // 100 chars + "…"
        assert!(result.starts_with('…'));
        assert!(result.ends_with('x'));
    }

    #[test]
    fn buffer_truncation() {
        let store = ProgressStore::new();
        store.register_job("job-tr", "harness");
        store.set_running("job-tr");

        // Push entries that together exceed the window size.
        for i in 0..5 {
            store.push_progress("job-tr", &format!("step {} {}", i, "x".repeat(300)));
        }

        let snap = store.get_update("job-tr", Some(500)).unwrap();
        // Should be truncated but still contain the most recent entry.
        assert!(snap.recent_snippet.len() <= 510); // some slack for "…"
        assert!(snap.recent_snippet.contains("step 4"));
    }

    #[test]
    fn remove_job() {
        let store = ProgressStore::new();
        store.register_job("job-rm", "harness");
        assert!(store.remove("job-rm"));
        assert!(!store.remove("job-rm"));
        assert!(store.get_update("job-rm", None).is_none());
    }

    #[test]
    fn push_empty_message_ignored() {
        let store = ProgressStore::new();
        store.register_job("job-emp", "harness");
        store.set_running("job-emp");
        store.push_progress("job-emp", "");
        store.push_progress("job-emp", "   ");

        let snap = store.get_update("job-emp", None).unwrap();
        assert!(snap.last_message.is_empty());
    }

    #[test]
    fn max_buffer_entries_bounded() {
        let store = ProgressStore::new();
        store.register_job("job-buf", "harness");
        store.set_running("job-buf");

        for i in 0..(MAX_BUFFER_ENTRIES + 5) {
            store.push_progress("job-buf", &format!("entry {i}"));
        }

        // Internal buffer should be capped at MAX_BUFFER_ENTRIES.
        let inner = store.inner.lock().unwrap();
        let progress = inner.jobs.get("job-buf").unwrap();
        assert_eq!(progress.recent_buffer.len(), MAX_BUFFER_ENTRIES);
        // The oldest entries should have been dropped.
        assert!(progress.recent_buffer.front().unwrap().starts_with("entry 5"));
        assert!(progress.recent_buffer.back().unwrap().starts_with("entry 24"));
    }
}
