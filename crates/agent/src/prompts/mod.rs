// ---------------------------------------------------------------------------
// Reusable prompt strings for the agent runtime
//
// Keep all agent-facing prompts here so they're easy to find, update, and
// test in one place. Each constant is a complete prompt fragment that can
// be inserted into a tool result or system message.
// ---------------------------------------------------------------------------

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

/// Model used for compaction summaries — cheap and fast.
pub const COMPACTION_MODEL: &str = "openai/gpt-4.1-nano";

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
