// ---------------------------------------------------------------------------
// handlers::hud — Heads Up Display read handlers
// ---------------------------------------------------------------------------


use crate::rpc::methods::hud::{
    ContextTokensGetParams, ContextTokensGetResult,
    CurrentGitBranchGetParams, CurrentGitBranchGetResult,
    DiffCountsGetParams, DiffCountsGetResult,
};
use agent::conversation;
use agent::llm::{AssistantContent, ChatCompletionMessage, StringOrTextParts, UserContent};
use agent::prompts::CHARS_PER_TOKEN;
use std::process::Command;
use storage::fs::FsStore;

fn get_current_branch() -> Result<String, String> {
    let output = Command::new("git")
        .arg("branch")
        .arg("--show-current")
        .output()
        .map_err(|e| format!("failed to get current branch: {e}"))?;
    Ok(String::from_utf8(output.stdout).map_err(|e| format!("failed to get current branch: {e}"))?)
}
/// Handle `hud.currentGitBranch.get`.
pub fn current_git_branch_get(_: CurrentGitBranchGetParams) -> Result<CurrentGitBranchGetResult, String> {
    let branch = get_current_branch()?;
    Ok(CurrentGitBranchGetResult { branch })
}

/// Handle `hud.diffCounts.get` — sum added/removed lines from `git diff HEAD --numstat`.
pub fn diff_counts_get(_: DiffCountsGetParams) -> Result<DiffCountsGetResult, String> {
    let output = Command::new("git")
        .args(["diff", "HEAD", "--numstat"])
        .output()
        .map_err(|e| format!("failed to run git diff: {e}"))?;

    if !output.status.success() {
        return Ok(DiffCountsGetResult { added: 0, removed: 0 });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut added: u64 = 0;
    let mut removed: u64 = 0;
    for line in stdout.lines() {
        let mut fields = line.split('\t');
        let a = fields.next().unwrap_or("");
        let r = fields.next().unwrap_or("");
        added += a.parse::<u64>().unwrap_or(0);
        removed += r.parse::<u64>().unwrap_or(0);
    }
    Ok(DiffCountsGetResult { added, removed })
}

/// Handle `hud.contextTokens.get` — approximate token count for the active
/// conversation's stored messages. `0` when no conversation is active.
pub fn context_tokens_get(_: ContextTokensGetParams) -> Result<ContextTokensGetResult, String> {
    let Some(conv_id) = storage::settings::read().conversation else {
        return Ok(ContextTokensGetResult { tokens: 0 });
    };
    let store = FsStore::new().map_err(|e| format!("failed to open store: {e}"))?;
    let conv = match conversation::load(&store, &conv_id) {
        Ok(c) => c,
        Err(_) => return Ok(ContextTokensGetResult { tokens: 0 }),
    };

    let mut chars: usize = 0;
    for msg in &conv.messages {
        chars += message_chars(msg);
    }
    let tokens = (chars / CHARS_PER_TOKEN) as u64;
    Ok(ContextTokensGetResult { tokens })
}

fn message_chars(msg: &ChatCompletionMessage) -> usize {
    match msg {
        ChatCompletionMessage::Developer(m) => parts_chars(&m.content),
        ChatCompletionMessage::System(m) => parts_chars(&m.content),
        ChatCompletionMessage::User(m) => match &m.content {
            UserContent::String(s) => s.len(),
            _ => 0,
        },
        ChatCompletionMessage::Assistant(m) => {
            let text = m.content.as_ref().map_or(0, |c| match c {
                AssistantContent::String(s) => s.len(),
                _ => 0,
            });
            let tool_calls = m.tool_calls.as_ref().map_or(0, |calls| {
                calls
                    .iter()
                    .map(|c| c.function.name.len() + c.function.arguments.len())
                    .sum::<usize>()
            });
            text + tool_calls
        }
        ChatCompletionMessage::Tool(m) => parts_chars(&m.content),
        ChatCompletionMessage::Function(m) => m.content.as_deref().map_or(0, str::len),
    }
}

fn parts_chars(content: &StringOrTextParts) -> usize {
    match content {
        StringOrTextParts::String(s) => s.len(),
        _ => 0,
    }
}
