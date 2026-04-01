use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use super::common::{
    Annotation, ChoiceLogprobs, CompletionUsage, FinishReason, ServiceTier,
};
use super::tools::{FunctionCall, ToolCall};

// ---------------------------------------------------------------------------
// POST /v1/chat/completions — non-streaming response
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletion {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<CompletionUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<ServiceTier>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: ResponseMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<ChoiceLogprobs>,
    /// OpenRouter: inline error on this choice.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<InlineError>,
}

// ---------------------------------------------------------------------------
// Response message (assistant reply)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<FunctionCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<ResponseAudio>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Vec<Annotation>>,
    /// OpenRouter: reasoning text from thinking models.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    /// OpenRouter: structured reasoning details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_details: Option<Vec<JsonValue>>,
    /// OpenRouter: generated images.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<ResponseImage>>,
}

// ---------------------------------------------------------------------------
// Audio in response
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseAudio {
    pub id: String,
    pub data: String,
    pub expires_at: i64,
    pub transcript: String,
}

// ---------------------------------------------------------------------------
// Image in response (OpenRouter)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseImage {
    pub image_url: ResponseImageUrl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseImageUrl {
    pub url: String,
}

// ---------------------------------------------------------------------------
// Inline error (OpenRouter streaming/response)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineError {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
}
