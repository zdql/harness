use super::protocol::{
    compact_json, format_job, preview, CreateResult, RpcLogContext, RpcResponse,
    SendResult, ToolCallInfo,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

pub(crate) struct OrchestratorClient {
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
    next_id: i64,
}

impl OrchestratorClient {
    pub(crate) async fn spawn(server_bin: Option<String>) -> Result<Self, String> {
        let path = match server_bin {
            Some(path) => PathBuf::from(path),
            None => default_server_bin()?,
        };

        let mut child = Command::new(&path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("failed to spawn {}: {e}", path.display()))?;

        let stdin = child
            .stdin
            .take()
            .ok_or("harness-server stdin unavailable")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("harness-server stdout unavailable")?;

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout).lines(),
            next_id: 1,
        })
    }

    pub(crate) async fn create_conversation(&mut self) -> Result<String, String> {
        let result: CreateResult = self
            .call("conversation.create", json!({}), RpcLogContext::default())
            .await?;
        eprintln!("orchestrator conversation created id={}", result.id);
        Ok(result.id)
    }

    pub(crate) async fn send_message_until_done_for_job(
        &mut self,
        job_id: &str,
        conversation_id: &str,
        message: &str,
    ) -> Result<SendResult, String> {
        self.send_message_until_done_with_context(conversation_id, message, Some(job_id))
            .await
    }

    async fn send_message_until_done_with_context(
        &mut self,
        conversation_id: &str,
        message: &str,
        job_id: Option<&str>,
    ) -> Result<SendResult, String> {
        let mut result = self
            .send_message_with_context(conversation_id, message, job_id)
            .await?;
        eprintln!(
            "orchestrator rpc recv conversation.send{} conversation={} reply_bytes={} tool_calls={} suspended={}",
            format_job(job_id),
            conversation_id,
            result.reply.len(),
            result.tool_calls.len(),
            result.suspended
        );
        log_tool_call_summaries(job_id, conversation_id, &result.tool_calls);
        if result.suspended {
            eprintln!(
                "orchestrator rpc wait continuation_done{} conversation={}",
                format_job(job_id),
                conversation_id
            );
            result.reply = self
                .wait_for_continuation_done(RpcLogContext {
                    conversation_id: Some(conversation_id),
                    job_id,
                })
                .await?;
            result.suspended = false;
            eprintln!(
                "orchestrator rpc recv continuation_done{} conversation={} reply_bytes={}",
                format_job(job_id),
                conversation_id,
                result.reply.len()
            );
        }
        Ok(result)
    }

    async fn send_message_with_context(
        &mut self,
        conversation_id: &str,
        message: &str,
        job_id: Option<&str>,
    ) -> Result<SendResult, String> {
        eprintln!(
            "orchestrator rpc send conversation.send{} conversation={} bytes={}",
            format_job(job_id),
            conversation_id,
            message.len()
        );
        self.call(
            "conversation.send",
            json!({
                "id": conversation_id,
                "message": message,
            }),
            RpcLogContext {
                conversation_id: Some(conversation_id),
                job_id,
            },
        )
        .await
    }

    async fn call<T>(
        &mut self,
        method: &str,
        params: Value,
        log_context: RpcLogContext<'_>,
    ) -> Result<T, String>
    where
        T: for<'de> Deserialize<'de>,
    {
        let id = self.next_id;
        self.next_id += 1;

        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let line = serde_json::to_string(&request)
            .map_err(|e| format!("failed to serialize request: {e}"))?;
        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| format!("failed to write request: {e}"))?;
        self.stdin
            .write_all(b"\n")
            .await
            .map_err(|e| format!("failed to write request newline: {e}"))?;
        self.stdin
            .flush()
            .await
            .map_err(|e| format!("failed to flush request: {e}"))?;

        loop {
            let Some(line) = self
                .stdout
                .next_line()
                .await
                .map_err(|e| format!("failed to read response: {e}"))?
            else {
                return Err("harness-server closed stdout".to_string());
            };

            let value: Value = serde_json::from_str(&line)
                .map_err(|e| format!("invalid JSON from harness-server: {e}: {line}"))?;

            if value.get("method").and_then(Value::as_str) == Some("agent.event") {
                log_agent_event(log_context, value.get("params").unwrap_or(&Value::Null));
                continue;
            }

            if value.get("id").and_then(Value::as_i64) != Some(id) {
                continue;
            }

            let response: RpcResponse = serde_json::from_value(value)
                .map_err(|e| format!("invalid RPC response shape: {e}"))?;
            if let Some(error) = response.error {
                return Err(error.message);
            }
            let result = response.result.ok_or("RPC response missing result")?;
            return serde_json::from_value(result)
                .map_err(|e| format!("invalid RPC result for {method}: {e}"));
        }
    }

    async fn wait_for_continuation_done(
        &mut self,
        log_context: RpcLogContext<'_>,
    ) -> Result<String, String> {
        loop {
            let Some(line) = self
                .stdout
                .next_line()
                .await
                .map_err(|e| format!("failed to read continuation event: {e}"))?
            else {
                return Err("harness-server closed stdout".to_string());
            };

            let value: Value = serde_json::from_str(&line)
                .map_err(|e| format!("invalid JSON from harness-server: {e}: {line}"))?;
            if value.get("method").and_then(Value::as_str) != Some("agent.event") {
                continue;
            }

            let params = value.get("params").cloned().unwrap_or(Value::Null);
            log_agent_event(log_context, &params);
            if params.get("kind").and_then(Value::as_str) == Some("continuation_done") {
                return Ok(params
                    .get("reply")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string());
            }
        }
    }
}

fn log_agent_event(context: RpcLogContext<'_>, event: &Value) {
    let prefix = format!(
        "orchestrator event{}{}",
        format_job(context.job_id),
        context
            .conversation_id
            .map(|id| format!(" conversation={id}"))
            .unwrap_or_default()
    );
    log_agent_event_inner(&prefix, event, 0);
}

fn log_agent_event_inner(prefix: &str, event: &Value, depth: usize) {
    let kind = event
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    match kind {
        "llm_start" | "llm_end" => {
            eprintln!("{prefix} depth={depth} kind={kind}");
        }
        "reasoning_delta" => {
            let bytes = event
                .get("text")
                .and_then(Value::as_str)
                .map(str::len)
                .unwrap_or(0);
            eprintln!("{prefix} depth={depth} kind=reasoning_delta bytes={bytes}");
        }
        "content_delta" => {
            let text = event.get("text").and_then(Value::as_str).unwrap_or("");
            eprintln!(
                "{prefix} depth={depth} kind=content_delta bytes={} preview={}",
                text.len(),
                preview(text)
            );
        }
        "tool_call_start" => {
            let name = event.get("name").and_then(Value::as_str).unwrap_or("");
            let arguments = event.get("arguments").and_then(Value::as_str).unwrap_or("");
            eprintln!(
                "{prefix} depth={depth} kind=tool_call_start tool={} args_bytes={} args_preview={}",
                name,
                arguments.len(),
                preview(arguments)
            );
        }
        "tool_call_end" => {
            let name = event.get("name").and_then(Value::as_str).unwrap_or("");
            let result = event.get("result").and_then(Value::as_str).unwrap_or("");
            eprintln!(
                "{prefix} depth={depth} kind=tool_call_end tool={} result_bytes={} result_preview={}",
                name,
                result.len(),
                preview(result)
            );
        }
        "subagent_started" => {
            let subagent_id = event
                .get("subagent_id")
                .and_then(Value::as_str)
                .unwrap_or("");
            let task = event.get("task").and_then(Value::as_str).unwrap_or("");
            eprintln!(
                "{prefix} depth={depth} kind=subagent_started subagent={} task_preview={}",
                subagent_id,
                preview(task)
            );
        }
        "subagent_completed" => {
            let subagent_id = event
                .get("subagent_id")
                .and_then(Value::as_str)
                .unwrap_or("");
            let status = event.get("status").and_then(Value::as_str).unwrap_or("");
            let output = event.get("output").and_then(Value::as_str).unwrap_or("");
            eprintln!(
                "{prefix} depth={depth} kind=subagent_completed subagent={} status={} output_bytes={} output_preview={}",
                subagent_id,
                status,
                output.len(),
                preview(output)
            );
        }
        "subagent_event" => {
            let subagent_id = event
                .get("subagent_id")
                .and_then(Value::as_str)
                .unwrap_or("");
            eprintln!("{prefix} depth={depth} kind=subagent_event subagent={subagent_id}");
            if let Some(inner) = event.get("inner") {
                log_agent_event_inner(prefix, inner, depth + 1);
            }
        }
        "continuation_done" => {
            let reply = event.get("reply").and_then(Value::as_str).unwrap_or("");
            eprintln!(
                "{prefix} depth={depth} kind=continuation_done reply_bytes={} reply_preview={}",
                reply.len(),
                preview(reply)
            );
        }
        _ => {
            eprintln!(
                "{prefix} depth={depth} kind={kind} raw={}",
                compact_json(event)
            );
        }
    }
}

fn log_tool_call_summaries(
    job_id: Option<&str>,
    conversation_id: &str,
    tool_calls: &[ToolCallInfo],
) {
    for tool in tool_calls {
        eprintln!(
            "orchestrator tool summary{} conversation={} tool={} args_bytes={} result_bytes={} result_preview={}",
            format_job(job_id),
            conversation_id,
            tool.name,
            tool.arguments.len(),
            tool.result.len(),
            preview(&tool.result)
        );
    }
}



impl Drop for OrchestratorClient {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

fn default_server_bin() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("failed to find current exe: {e}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| format!("failed to find parent dir for {}", exe.display()))?;
    Ok(dir.join("harness-server"))
}
