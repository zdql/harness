use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use super::common::{ChoiceLogprobs, CompletionUsage, FinishReason, ServiceTier};
use super::response::InlineError;
use super::tools::{ToolCall, ToolCallFunction};

// ---------------------------------------------------------------------------
// SSE streaming response chunks
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChunkChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<CompletionUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<ServiceTier>,
    /// OpenRouter: inline error in stream.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<InlineError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkChoice {
    pub index: u32,
    pub delta: ChoiceDelta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<ChoiceLogprobs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<InlineError>,
}

// ---------------------------------------------------------------------------
// Delta — incremental message fields per chunk
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChoiceDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<DeltaToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<DeltaFunctionCall>,
    /// OpenRouter: reasoning text delta from thinking models.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    /// OpenRouter: structured reasoning details delta.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_details: Option<Vec<JsonValue>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaToolCall {
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<DeltaToolCallFunction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaToolCallFunction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaFunctionCall {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

// ---------------------------------------------------------------------------
// StreamAccumulator — merges SSE chunks into a single completed message
//
// Chunks arrive as partial deltas: text comes a few characters at a time,
// tool-call arguments stream in as JSON fragments, reasoning text and details
// arrive progressively. The accumulator stitches them together so the agent
// loop can treat the final result like a non-streaming response.
// ---------------------------------------------------------------------------

/// A fully assembled streaming response, ready to push into the conversation.
#[derive(Debug, Clone, Default)]
pub struct AccumulatedMessage {
    pub content: String,
    pub reasoning: String,
    pub reasoning_details: Vec<JsonValue>,
    pub tool_calls: Vec<ToolCall>,
    pub refusal: Option<String>,
    pub finish_reason: Option<FinishReason>,
}

#[derive(Debug, Default)]
pub struct StreamAccumulator {
    content: String,
    reasoning: String,
    /// Reasoning-details blocks, merged by their `index` field. Each slot
    /// is a JSON object whose `text` / `data` / `summary` fields are
    /// concatenated across chunks while `signature` / `type` / `format` are
    /// overwritten with the latest value. Must round-trip exactly on the
    /// next turn or Anthropic thinking + tool use rejects the request
    /// (invalid signature over fragmented text).
    reasoning_details: Vec<serde_json::Map<String, JsonValue>>,
    refusal: Option<String>,
    finish_reason: Option<FinishReason>,
    /// Tool calls indexed by their stream-delta `index`. Grown as new indices
    /// appear; collapsed into a dense Vec<ToolCall> in `into_message`.
    tool_calls: Vec<PartialToolCall>,
}

#[derive(Debug, Default, Clone)]
struct PartialToolCall {
    id: String,
    r#type: String,
    name: String,
    arguments: String,
}

impl StreamAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fold one chunk's first-choice delta into the accumulator.
    ///
    /// Returns the incremental content/reasoning text that was just appended,
    /// so the caller can emit streaming events without diffing strings itself.
    pub fn push_chunk(&mut self, chunk: &ChatCompletionChunk) -> ChunkAppended {
        let mut appended = ChunkAppended::default();

        // OpenRouter may report a stream-level error as a chunk.
        if let Some(err) = &chunk.error {
            appended.error = Some(err.message.clone());
        }

        let Some(choice) = chunk.choices.first() else {
            return appended;
        };
        if let Some(fr) = choice.finish_reason.clone() {
            self.finish_reason = Some(fr);
        }
        if let Some(err) = &choice.error {
            appended.error = Some(err.message.clone());
        }

        let delta = &choice.delta;

        if let Some(text) = &delta.content {
            self.content.push_str(text);
            appended.content = text.clone();
        }
        if let Some(text) = &delta.reasoning {
            self.reasoning.push_str(text);
            appended.reasoning = text.clone();
        }
        if let Some(details) = &delta.reasoning_details {
            for frag in details {
                let Some(obj) = frag.as_object() else {
                    continue;
                };
                // Group by `index`. If missing, append as its own block.
                let idx = obj
                    .get("index")
                    .and_then(|v| v.as_i64())
                    .unwrap_or_else(|| self.reasoning_details.len() as i64);
                // Find (or create) the slot for this index.
                let slot_pos = self
                    .reasoning_details
                    .iter()
                    .position(|m| m.get("index").and_then(|v| v.as_i64()) == Some(idx));
                let slot = if let Some(pos) = slot_pos {
                    &mut self.reasoning_details[pos]
                } else {
                    let mut m = serde_json::Map::new();
                    m.insert("index".to_string(), JsonValue::from(idx));
                    self.reasoning_details.push(m);
                    self.reasoning_details.last_mut().unwrap()
                };
                // Merge fields. Concatenate streaming text/data/summary,
                // overwrite everything else (type, format, signature, …).
                for (k, v) in obj {
                    match k.as_str() {
                        "index" => {}
                        "text" | "data" | "summary" => {
                            let prev = slot
                                .get(k)
                                .and_then(|e| e.as_str())
                                .unwrap_or("")
                                .to_string();
                            let add = v.as_str().unwrap_or("");
                            slot.insert(k.clone(), JsonValue::String(prev + add));
                        }
                        _ => {
                            slot.insert(k.clone(), v.clone());
                        }
                    }
                }
            }
        }
        if let Some(refusal) = &delta.refusal {
            match &mut self.refusal {
                Some(existing) => existing.push_str(refusal),
                None => self.refusal = Some(refusal.clone()),
            }
        }

        if let Some(calls) = &delta.tool_calls {
            for dc in calls {
                let idx = dc.index as usize;
                if self.tool_calls.len() <= idx {
                    self.tool_calls.resize(idx + 1, PartialToolCall::default());
                }
                let slot = &mut self.tool_calls[idx];
                if let Some(id) = &dc.id {
                    slot.id = id.clone();
                }
                if let Some(t) = &dc.r#type {
                    slot.r#type = t.clone();
                }
                if let Some(func) = &dc.function {
                    if let Some(name) = &func.name {
                        slot.name.push_str(name);
                    }
                    if let Some(args) = &func.arguments {
                        slot.arguments.push_str(args);
                    }
                }
            }
        }

        appended
    }

    /// Consume the accumulator and produce a finished message.
    pub fn into_message(mut self) -> AccumulatedMessage {
        let tool_calls = self
            .tool_calls
            .into_iter()
            .filter(|p| !p.id.is_empty() || !p.name.is_empty() || !p.arguments.is_empty())
            .map(|p| ToolCall {
                id: p.id,
                r#type: if p.r#type.is_empty() {
                    "function".to_string()
                } else {
                    p.r#type
                },
                function: ToolCallFunction {
                    name: p.name,
                    arguments: p.arguments,
                },
            })
            .collect();

        // Sort by index so ordering in the outgoing message matches what
        // the model produced. Drop indices that only ever carried the index
        // field itself (no type/text/data/signature) — those are empty.
        self.reasoning_details
            .sort_by_key(|m| m.get("index").and_then(|v| v.as_i64()).unwrap_or(0));
        let reasoning_details = self
            .reasoning_details
            .into_iter()
            .filter(|m| m.keys().any(|k| k != "index"))
            .map(JsonValue::Object)
            .collect();

        AccumulatedMessage {
            content: self.content,
            reasoning: self.reasoning,
            reasoning_details,
            tool_calls,
            refusal: self.refusal,
            finish_reason: self.finish_reason,
        }
    }
}

/// What a single chunk appended to the accumulator — handy for driving
/// streaming events in the agent loop.
#[derive(Debug, Clone, Default)]
pub struct ChunkAppended {
    pub content: String,
    pub reasoning: String,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn chunk_with_details(details: Vec<JsonValue>) -> ChatCompletionChunk {
        ChatCompletionChunk {
            id: "c".into(),
            object: "chat.completion.chunk".into(),
            created: 0,
            model: "m".into(),
            choices: vec![ChunkChoice {
                index: 0,
                delta: ChoiceDelta {
                    role: None,
                    content: None,
                    refusal: None,
                    tool_calls: None,
                    function_call: None,
                    reasoning: None,
                    reasoning_details: Some(details),
                },
                finish_reason: None,
                logprobs: None,
                error: None,
            }],
            usage: None,
            system_fingerprint: None,
            service_tier: None,
            error: None,
        }
    }

    #[test]
    fn reasoning_details_merge_by_index() {
        // Mimic the Anthropic streaming shape: many text fragments at
        // index 0, then a final fragment carrying the signature.
        let mut acc = StreamAccumulator::new();
        acc.push_chunk(&chunk_with_details(vec![json!({
            "type": "reasoning.text",
            "index": 0,
            "format": "anthropic-claude-v1",
            "text": "The ",
        })]));
        acc.push_chunk(&chunk_with_details(vec![json!({
            "type": "reasoning.text",
            "index": 0,
            "format": "anthropic-claude-v1",
            "text": "user is ",
        })]));
        acc.push_chunk(&chunk_with_details(vec![json!({
            "type": "reasoning.text",
            "index": 0,
            "format": "anthropic-claude-v1",
            "text": "saying hi.",
        })]));
        acc.push_chunk(&chunk_with_details(vec![json!({
            "type": "reasoning.text",
            "index": 0,
            "format": "anthropic-claude-v1",
            "signature": "SIG123",
        })]));

        let msg = acc.into_message();
        assert_eq!(msg.reasoning_details.len(), 1, "one merged block");
        let block = msg.reasoning_details[0].as_object().unwrap();
        assert_eq!(block["type"], "reasoning.text");
        assert_eq!(block["text"], "The user is saying hi.");
        assert_eq!(block["signature"], "SIG123");
        assert_eq!(block["format"], "anthropic-claude-v1");
        assert_eq!(block["index"], 0);
    }

    #[test]
    fn reasoning_details_separate_indices() {
        let mut acc = StreamAccumulator::new();
        acc.push_chunk(&chunk_with_details(vec![
            json!({"type": "reasoning.text", "index": 0, "text": "A"}),
            json!({"type": "reasoning.text", "index": 1, "text": "X"}),
        ]));
        acc.push_chunk(&chunk_with_details(vec![
            json!({"type": "reasoning.text", "index": 1, "text": "Y"}),
            json!({"type": "reasoning.text", "index": 0, "text": "B"}),
        ]));

        let msg = acc.into_message();
        assert_eq!(msg.reasoning_details.len(), 2);
        // Sorted by index ascending.
        assert_eq!(msg.reasoning_details[0]["index"], 0);
        assert_eq!(msg.reasoning_details[0]["text"], "AB");
        assert_eq!(msg.reasoning_details[1]["index"], 1);
        assert_eq!(msg.reasoning_details[1]["text"], "XY");
    }
}
