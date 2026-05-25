use super::protocol::{format_job, preview, SendResult};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

/// Client that drives the OpenAI Codex CLI as a background coding agent.
///
/// Unlike the Harness client (JSON-RPC over stdio), Codex runs as a
/// fire-and-forget subprocess: we send a prompt and collect stdout as the
/// reply. Tool-call details are not exposed by the CLI, so `tool_calls` is
/// always empty and `suspended` is always false.
pub(crate) struct CodexClient {
    path: PathBuf,
    model: Option<String>,
}

impl CodexClient {
    /// Prepare a Codex non-interactive client.
    pub(crate) async fn spawn(
        codex_bin: Option<String>,
        model: Option<String>,
    ) -> Result<Self, String> {
        let path = resolve_codex_bin(codex_bin)?;

        if path.components().count() > 1 && !path.exists() {
            return Err(format!(
                "codex binary not found at {} — install codex-cli or pass --codex-bin",
                path.display()
            ));
        }

        let output = Command::new(&path)
            .arg("exec")
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| format!("failed to probe codex at {}: {e}", path.display()))?;
        if !output.status.success() {
            return Err(format!(
                "codex probe failed at {}: {}",
                path.display(),
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        eprintln!("codex client spawned bin={}", path.display());
        Ok(Self { path, model })
    }

    /// Send a user message to Codex and return the text reply.
    ///
    /// Each call spawns `codex exec` with the prompt on stdin so shell escaping
    /// is avoided. We ask Codex to write its final assistant message to a temp
    /// file because formatted stdout may contain progress output.
    pub(crate) async fn send_message_until_done_for_job(
        &mut self,
        job_id: &str,
        _conversation_id: &str,
        message: &str,
    ) -> Result<SendResult, String> {
        eprintln!(
            "codex send{} message_bytes={}",
            format_job(Some(job_id)),
            message.len()
        );

        let output_path = output_path_for_job(job_id);
        let mut command = Command::new(&self.path);
        command
            .arg("exec")
            .arg("--sandbox")
            .arg("workspace-write")
            .arg("-c")
            .arg("approval_policy=\"never\"")
            .arg("--output-last-message")
            .arg(&output_path)
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(model) = self.model.as_deref() {
            command.arg("--model").arg(model);
        }

        let mut child = command
            .spawn()
            .map_err(|e| format!("failed to spawn codex: {e}"))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(message.as_bytes())
                .await
                .map_err(|e| format!("failed to write to codex stdin: {e}"))?;
            drop(stdin);
        }

        let mut stdout_buf = Vec::new();
        let mut stderr_buf = Vec::new();
        if let Some(mut stdout) = child.stdout.take() {
            stdout
                .read_to_end(&mut stdout_buf)
                .await
                .map_err(|e| format!("failed to read codex stdout: {e}"))?;
        }
        if let Some(mut stderr) = child.stderr.take() {
            stderr
                .read_to_end(&mut stderr_buf)
                .await
                .map_err(|e| format!("failed to read codex stderr: {e}"))?;
        }

        let status = child
            .wait()
            .await
            .map_err(|e| format!("failed to wait for codex: {e}"))?;

        let stderr_text = String::from_utf8_lossy(&stderr_buf);
        if !stderr_text.is_empty() {
            eprintln!(
                "codex stderr{} {}",
                format_job(Some(job_id)),
                preview(&stderr_text)
            );
        }

        if !status.success() {
            return Err(format!(
                "codex exited with code {:?}: {}",
                status.code(),
                preview(&stderr_text)
            ));
        }

        let reply = match tokio::fs::read_to_string(&output_path).await {
            Ok(reply) => reply,
            Err(_) => String::from_utf8(stdout_buf)
                .map_err(|e| format!("codex output was not valid utf-8: {e}"))?,
        };
        let _ = tokio::fs::remove_file(&output_path).await;

        eprintln!(
            "codex recv{} reply_bytes={} reply_preview={}",
            format_job(Some(job_id)),
            reply.len(),
            preview(&reply)
        );

        Ok(SendResult {
            reply,
            tool_calls: Vec::new(),
            suspended: false,
        })
    }
}

fn resolve_codex_bin(codex_bin: Option<String>) -> Result<PathBuf, String> {
    match codex_bin {
        Some(path) => Ok(PathBuf::from(path)),
        None => which_codex(),
    }
}

fn output_path_for_job(job_id: &str) -> PathBuf {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let safe_job_id = job_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    std::env::temp_dir().join(format!(
        "harness-voice-codex-{safe_job_id}-{:x}-{:x}.txt",
        d.as_secs(),
        d.subsec_nanos()
    ))
}

fn which_codex() -> Result<PathBuf, String> {
    // Check a few common install locations before falling back to PATH.
    let candidates = [
        PathBuf::from("/usr/local/bin/codex"),
        PathBuf::from(
            std::env::var("HOME")
                .unwrap_or_default()
                + "/.local/bin/codex",
        ),
        PathBuf::from(
            std::env::var("HOME")
                .unwrap_or_default()
                + "/.npm-global/bin/codex",
        ),
    ];
    for candidate in &candidates {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }

    // Fall back to bare "codex" and let the OS resolve via PATH.
    Ok(PathBuf::from("codex"))
}
