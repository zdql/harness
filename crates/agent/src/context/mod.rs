// ---------------------------------------------------------------------------
// Context management — track token usage and prevent overflow
//
// Uses a simple character-based estimate (1 token ≈ 4 chars) which is
// conservative enough to avoid hitting limits while not being so aggressive
// that it rejects reasonable tool results. The estimate covers message
// content and tool definitions serialized as JSON.
// ---------------------------------------------------------------------------

use crate::llm::{ChatCompletionMessage, ChatCompletionTool};

/// Approximate tokens-per-character ratio. ~4 chars per token is a widely
/// used heuristic for English text and JSON payloads.
const CHARS_PER_TOKEN: usize = 4;

/// Default context limit when no model-specific limit is known.
const DEFAULT_CONTEXT_LIMIT: usize = 128_000;

/// Safety margin — reserve tokens for the model's reply.
const REPLY_RESERVE: usize = 4_096;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// A lightweight context budget tracker.
pub struct ContextBudget {
    /// Maximum tokens the model accepts as input.
    pub limit: usize,
    /// Tokens reserved for the model's response.
    pub reply_reserve: usize,
}

impl ContextBudget {
    /// Create a budget for the given model name. Falls back to 128k if the
    /// model isn't recognized.
    pub fn for_model(model: Option<&str>) -> Self {
        let limit = model
            .map(|m| context_limit_for_model(m))
            .unwrap_or(DEFAULT_CONTEXT_LIMIT);

        Self {
            limit,
            reply_reserve: REPLY_RESERVE,
        }
    }

    /// Maximum tokens available for input (limit minus reply reserve).
    pub fn input_budget(&self) -> usize {
        self.limit.saturating_sub(self.reply_reserve)
    }

    /// Estimate whether adding `additional_text` to the current messages +
    /// tool definitions would exceed the input budget.
    ///
    /// Returns `true` if the addition fits, `false` if it would overflow.
    pub fn fits(
        &self,
        messages: &[ChatCompletionMessage],
        tools: &[ChatCompletionTool],
        additional_text: &str,
    ) -> bool {
        let current = estimate_messages(messages) + estimate_tools(tools);
        let addition = estimate_str(additional_text);
        current + addition <= self.input_budget()
    }

    /// If `result` fits within budget, return it as-is. Otherwise return the
    /// overflow prompt so the model retries with a smaller query.
    pub fn guard_tool_result(
        &self,
        messages: &[ChatCompletionMessage],
        tools: &[ChatCompletionTool],
        result: &str,
    ) -> String {
        if self.fits(messages, tools, result) {
            result.to_string()
        } else {
            crate::prompts::TOOL_RESULT_OVERFLOW.to_string()
        }
    }

    /// How many tokens of headroom remain after the current messages + tools.
    pub fn remaining(
        &self,
        messages: &[ChatCompletionMessage],
        tools: &[ChatCompletionTool],
    ) -> usize {
        let current = estimate_messages(messages) + estimate_tools(tools);
        self.input_budget().saturating_sub(current)
    }
}

// ---------------------------------------------------------------------------
// Token estimation helpers
// ---------------------------------------------------------------------------

/// Estimate tokens for a slice of messages by serializing to JSON.
pub fn estimate_messages(messages: &[ChatCompletionMessage]) -> usize {
    // Serialize the whole array — this captures role tags, content,
    // tool_call_id fields, etc., which all consume tokens.
    let json = serde_json::to_string(messages).unwrap_or_default();
    estimate_str(&json)
}

/// Estimate tokens for tool definitions.
pub fn estimate_tools(tools: &[ChatCompletionTool]) -> usize {
    let json = serde_json::to_string(tools).unwrap_or_default();
    estimate_str(&json)
}

/// Estimate tokens from a raw string.
pub fn estimate_str(s: &str) -> usize {
    // Integer ceiling division so we never under-count.
    (s.len() + CHARS_PER_TOKEN - 1) / CHARS_PER_TOKEN
}

// ---------------------------------------------------------------------------
// Model context limits (add more as needed)
// ---------------------------------------------------------------------------

fn context_limit_for_model(model: &str) -> usize {
    let m = model.to_lowercase();

    // Check for known model families. Order matters — more specific first.
    if m.contains("gpt-4o") || m.contains("gpt-4.1") {
        128_000
    } else if m.contains("gpt-4-turbo") {
        128_000
    } else if m.contains("gpt-4") {
        8_192
    } else if m.contains("gpt-3.5") {
        16_385
    } else if m.contains("claude-3") || m.contains("claude-4") {
        200_000
    } else if m.contains("gemini-2") || m.contains("gemini-1.5") {
        1_000_000
    } else if m.contains("deepseek") {
        128_000
    } else if m.contains("llama") {
        128_000
    } else {
        DEFAULT_CONTEXT_LIMIT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_str_basic() {
        // 12 chars → ceil(12/4) = 3 tokens
        assert_eq!(estimate_str("hello world!"), 3);
    }

    #[test]
    fn estimate_str_empty() {
        assert_eq!(estimate_str(""), 0);
    }

    #[test]
    fn known_model_limits() {
        assert_eq!(context_limit_for_model("openai/gpt-4o"), 128_000);
        assert_eq!(context_limit_for_model("openai/gpt-4.1-nano"), 128_000);
        assert_eq!(context_limit_for_model("anthropic/claude-3-opus"), 200_000);
        assert_eq!(context_limit_for_model("unknown/model"), DEFAULT_CONTEXT_LIMIT);
    }

    #[test]
    fn budget_fits() {
        let budget = ContextBudget {
            limit: 100,
            reply_reserve: 10,
        };
        // input budget = 90 tokens = 360 chars
        let empty: Vec<ChatCompletionMessage> = vec![];
        let no_tools: Vec<ChatCompletionTool> = vec![];

        assert!(budget.fits(&empty, &no_tools, "short"));
        // 400 chars = 100 tokens, exceeds 90
        let long = "x".repeat(400);
        assert!(!budget.fits(&empty, &no_tools, &long));
    }
}
