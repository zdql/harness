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
