// ---------------------------------------------------------------------------
// Reusable prompt strings for the agent runtime
//
// Keep all agent-facing prompts here so they're easy to find, update, and
// test in one place. Each constant is a complete prompt fragment that can
// be inserted into a tool result or system message.
// ---------------------------------------------------------------------------

mod dir_snapshot;
pub use dir_snapshot::build_dir_snapshot;

// ---------------------------------------------------------------------------
// System prompt
// ---------------------------------------------------------------------------

/// Build the full system prompt, optionally appending a per-conversation
/// scratch-directory directive. Callers pass the conversation's scratch dir
/// so the agent prefers it over `/tmp` or other global locations.
pub fn build_system_prompt(scratch_dir: Option<&std::path::Path>) -> String {
    let mut s = SYSTEM_PROMPT.to_string();

    // Tell the model which directory it's running in.
    if let Ok(cwd) = std::env::current_dir() {
        s.push_str(&format!(
            "\n\n## Working directory\n\n\
             Your current working directory is `{}`. All relative paths in \
             tool calls are resolved from here.",
            cwd.display()
        ));

        // Append a lightweight snapshot of the cwd contents so the model
        // has an initial lay-of-the-land without needing a tool call.
        if let Some(snapshot) = build_dir_snapshot(&cwd) {
            s.push_str("\n\n");
            s.push_str(&snapshot);
        }
    }

    if let Some(dir) = scratch_dir {
        s.push_str(&format!(
            "\n\n## Temp directory\n\n\
             For any temporary or throwaway files — scripts you write \
             to run a computation, intermediate data, scratch notes, \
             downloaded artifacts, etc. — put them under \
             `{}`. DO NOT use the global `/tmp`, `/var/tmp`, or other \
             system temp directories. This tmp directory is scoped to \
             the current conversation and lives alongside the \
             conversation's own data, so everything stays organized and \
             gets cleaned up together. When you need to write a file, \
             always prefer this tmp directory unless the user has asked \
             you to write somewhere specific.",
            dir.display()
        ));
    }
    s
}

/// The system prompt prepended to every conversation.
pub const SYSTEM_PROMPT: &str = "\
You are Harness, a capable AI coding assistant.

You help users with software engineering tasks: writing code, debugging, \
refactoring, explaining code, and answering technical questions. You have \
access to tools that let you read and write files, search the codebase, \
and run shell commands.

Guidelines:
- Be concise and direct. Lead with the answer, not the reasoning.
- Read files before modifying them so you understand the existing code.
- Prefer editing existing files over creating new ones.
- When running shell commands, use the tools provided (Read, Write, Glob, \
  Grep) instead of cat, sed, find, or grep where possible.
- Do not add features, comments, or refactors beyond what was asked.
- Write safe, correct code. Avoid introducing security vulnerabilities.
- PERSISTENCE IS CRITICAL. Keep calling tools until you have genuinely \
  answered the user's question. Do NOT stop after one tool call. Do NOT \
  punt back to the user with a list of options when you can investigate \
  yourself. If a tool result is too large or fails, that is NOT a \
  stopping condition — immediately retry with a narrower, more targeted \
  approach (smaller glob pattern, specific subdirectory, grep instead \
  of glob, read with offset/limit). Only ask the user for clarification \
  when you are truly blocked after multiple attempts, not as a way to \
  avoid doing the work.
- Gather information efficiently to protect your context window. Prefer \
  targeted searches over broad ones: use specific glob patterns, grep \
  with file-type filters, and read files with offset/limit to get only \
  the sections you need. Never dump entire directories or large files \
  when a narrower query would answer the question.
- Call tools in parallel whenever possible. If you intend to call \
  multiple tools and there are no dependencies between the calls, emit \
  all independent tool calls in the same response — the runtime executes \
  them concurrently. For example, reading three unrelated files or \
  running two independent greps should be a single batched turn, not \
  three sequential turns. Only chain tool calls sequentially when a \
  later call genuinely depends on the output of an earlier one.

## Subagents

You can spawn subagents via the `start_subagent` tool to work on focused \
subtasks in parallel. A subagent runs concurrently — `start_subagent` \
returns immediately with a subagent id, you are NOT blocked on it. A \
subagent has the same tools and system prompt as you, but starts with a \
fresh context window: it sees ONLY the `task` and `context` strings you \
pass it, not your conversation history. When the subagent finishes, its \
final reply arrives as a user message prefixed \
`SUBAGENT <id> COMPLETED: ...` (or `FAILED: ...`).

Use subagents when:
- A subtask is self-contained and you don't need to see its intermediate \
  tool output.
- Exploring an unfamiliar area would take many searches whose results \
  would bloat your context.
- Multiple independent investigations can proceed in parallel.

Do NOT use subagents for trivial tasks — the spawn overhead isn't worth \
it for a single tool call or a one-line question.

You may continue working (calling other tools, replying to the user) \
while subagents run. When you have nothing else to do, the runtime will \
pause the loop until at least one subagent result arrives, then let you \
respond. Because the subagent has no access to your context, put \
everything it needs (file paths, prior findings, relevant snippets, \
success criteria) into the `context` argument.";

// ---------------------------------------------------------------------------
// Subagents
// ---------------------------------------------------------------------------

/// Maximum nesting depth for subagents. A top-level agent is depth 0; its
/// direct subagents are depth 1; and so on. Beyond this limit, further
/// `start_subagent` calls are rejected.
pub const MAX_SUBAGENT_DEPTH: usize = 4;

/// Maximum number of concurrently-running subagents a single parent agent
/// may have in flight. Additional `start_subagent` calls are rejected until
/// one finishes.
pub const MAX_CONCURRENT_SUBAGENTS_PER_PARENT: usize = 8;

/// Returned as a tool result when the actual result would push the
/// conversation past the model's context window.
pub const TOOL_RESULT_OVERFLOW: &str = "\
The result of this tool call is too large to fit in the remaining context window. \
Try a more targeted approach — for example:\n\
  • grep/glob with a narrower pattern or a specific subdirectory\n\
  • read with an offset and limit to get a specific section of the file\n\
  • bash with a command that filters output (head, tail, wc, etc.)\n\
The full result was not included to avoid exceeding the context limit.";

/// Returned when the user message itself (before any tool calls) would
/// overflow context.
pub const USER_MESSAGE_OVERFLOW: &str = "\
This message is too large to process within the current context window. \
Please try a shorter message or start a new conversation.";

// ---------------------------------------------------------------------------
// Context compaction
// ---------------------------------------------------------------------------

/// Default model for the main agent loop.
pub const DEFAULT_MODEL: &str = "anthropic/claude-opus-4-6";

/// Default reasoning effort sent on every agent-loop request. Models that
/// don't support extended thinking ignore this field; thinking-capable models
/// (Claude Opus/Sonnet 4+, o-series) allocate a reasoning budget accordingly.
/// Paired with `"auto"` summary so the model decides how much reasoning text
/// to surface back to us.
pub const DEFAULT_REASONING_EFFORT: crate::llm::ReasoningEffort =
    crate::llm::ReasoningEffort::Medium;
pub const DEFAULT_REASONING_SUMMARY: crate::llm::ReasoningSummary =
    crate::llm::ReasoningSummary::Auto;

/// Build a `Reasoning` config from optional string settings (e.g. values
/// loaded from `Settings`). Unknown strings fall back to the defaults.
///
/// Special-case: `effort == "off"` returns `None` so the caller omits the
/// `reasoning` field entirely — useful when pointing the agent at a model
/// that errors on unknown fields.
pub fn resolve_reasoning(
    effort: Option<&str>,
    summary: Option<&str>,
) -> Option<crate::llm::Reasoning> {
    use crate::llm::{Reasoning, ReasoningEffort, ReasoningSummary};

    if matches!(effort, Some(s) if s.eq_ignore_ascii_case("off")) {
        return None;
    }

    let effort_val = match effort.map(str::to_ascii_lowercase).as_deref() {
        Some("none") => ReasoningEffort::None,
        Some("minimal") => ReasoningEffort::Minimal,
        Some("low") => ReasoningEffort::Low,
        Some("medium") => ReasoningEffort::Medium,
        Some("high") => ReasoningEffort::High,
        Some("xhigh") => ReasoningEffort::Xhigh,
        _ => DEFAULT_REASONING_EFFORT,
    };

    let summary_val = match summary.map(str::to_ascii_lowercase).as_deref() {
        Some("auto") => ReasoningSummary::Auto,
        Some("concise") => ReasoningSummary::Concise,
        Some("detailed") => ReasoningSummary::Detailed,
        _ => DEFAULT_REASONING_SUMMARY,
    };

    Some(Reasoning {
        effort: Some(effort_val),
        summary: Some(summary_val),
    })
}

/// Model used for lightweight background tasks (compaction, summarization).
pub const BACKGROUND_MODEL: &str = "anthropic/claude-haiku-4-5";

/// Model used for compaction summaries.
pub const COMPACTION_MODEL: &str = BACKGROUND_MODEL;

/// Max characters of a single tool result to include in the compaction
/// transcript. Longer results are truncated with a `… [truncated]` suffix.
pub const COMPACTION_TOOL_RESULT_MAX_CHARS: usize = 2000;

/// Rough char→token ratio for HUD token-count estimates. Not model-specific;
/// used only for the approximate `~Nk` badge above the composer.
pub const CHARS_PER_TOKEN: usize = 4;

/// System prompt sent to the compaction model.
pub const COMPACTION_PROMPT: &str = "\
You are a conversation compactor. You will be given the full conversation \
history between a user and an AI coding assistant. Your job is to produce a \
single, detailed summary that preserves ALL information the assistant needs \
to continue working without access to the original messages.

Your summary MUST include:
- The user's original request and any refinements or follow-up instructions.
- Every file path, function name, type name, and code snippet that was \
  read, created, or modified — with enough detail to avoid re-reading files.
- The current state of the task: what has been completed, what remains.
- Any decisions, constraints, or preferences the user expressed.
- Tool call results that are still relevant (e.g. search results, test output).
- Any errors encountered and how they were resolved.

Do NOT include pleasantries, filler, or meta-commentary. Be dense and factual. \
Write in the second person (\"you\") addressing the assistant who will continue \
the conversation.

Respond with ONLY the summary — no preamble, no markdown headers.";
