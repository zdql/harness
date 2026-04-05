// ---------------------------------------------------------------------------
// Reusable prompt strings for the agent runtime
//
// Keep all agent-facing prompts here so they're easy to find, update, and
// test in one place. Each constant is a complete prompt fragment that can
// be inserted into a tool result or system message.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// System prompt
// ---------------------------------------------------------------------------

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
  later call genuinely depends on the output of an earlier one.";

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
