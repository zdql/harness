// ---------------------------------------------------------------------------
// Parallel tool call dispatcher
//
// The agent loop receives a batch of tool calls from the model and needs to
// execute them concurrently. Tool implementations (bash, file I/O, grep, …)
// are synchronous blocking code, so each call runs on tokio's blocking pool
// via `spawn_blocking`. Results are collected in the original call order so
// `tool_call_id` pairing matches what the model emitted.
//
// Event emission, context-budget guarding, and conversation persistence stay
// in the agent loop — this module's only job is to run the batch in parallel.
// ---------------------------------------------------------------------------

use std::sync::Arc;

use crate::llm::ToolCall;

use super::ToolRegistry;

/// Execute a batch of tool calls concurrently on tokio's blocking pool.
///
/// Returns `(call, raw_result)` pairs in the same order they were provided.
/// `raw_result` is the tool's JSON-stringified return value, or a JSON error
/// object if the tool failed or the blocking task panicked.
pub async fn execute_batch(
    tools: Arc<ToolRegistry>,
    calls: &[ToolCall],
) -> Vec<(ToolCall, String)> {
    // Dispatch every call onto the blocking pool.
    let mut handles = Vec::with_capacity(calls.len());
    for call in calls {
        let tools_for_task = Arc::clone(&tools);
        let name = call.function.name.clone();
        let args = call.function.arguments.clone();
        let handle = tokio::task::spawn_blocking(move || {
            match tools_for_task.call(&name, &args) {
                Ok(val) => val.to_string(),
                Err(e) => format!("{{\"error\": \"{e}\"}}"),
            }
        });
        handles.push((call.clone(), handle));
    }

    // Join in order — preserves alignment with the model's tool_call_id list.
    let mut results = Vec::with_capacity(handles.len());
    for (call, handle) in handles {
        let raw = handle
            .await
            .unwrap_or_else(|e| format!("{{\"error\": \"tool task join failed: {e}\"}}"));
        results.push((call, raw));
    }
    results
}
