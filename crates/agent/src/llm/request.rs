use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use super::common::{
    AudioParam, CacheControl, DebugOptions, ImageConfig, Modality, Plugin, PredictionContent,
    ProviderRouting, Reasoning, ResponseFormat, ServiceTier, Stop, StreamOptions, Trace,
    WebSearchOptions,
};
use super::message::ChatCompletionMessage;
use super::tools::{ChatCompletionTool, Function, FunctionCallOption, ToolChoice};

// ---------------------------------------------------------------------------
// POST /v1/chat/completions — request body
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChatCompletionRequest {
    pub messages: Vec<ChatCompletionMessage>,

    /// Model ID in `provider/model` format, e.g. `"openai/gpt-4o"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// OpenRouter: multi-model routing. Array of model IDs tried in order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioParam>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,

    /// Deprecated: use `tool_choice`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<FunctionCallOption>,

    /// Deprecated: use `tools`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub functions: Option<Vec<Function>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub logit_bias: Option<HashMap<String, i32>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,

    /// Deprecated: use `max_completion_tokens`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub modalities: Option<Vec<Modality>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub prediction: Option<PredictionContent>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<ServiceTier>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Stop>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<StreamOptions>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ChatCompletionTool>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<u8>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_search_options: Option<WebSearchOptions>,

    // -------------------------------------------------------------------
    // OpenRouter-specific fields
    // -------------------------------------------------------------------
    /// Provider routing preferences.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderRouting>,

    /// OpenRouter plugins (auto-router, web, moderation, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Vec<Plugin>>,

    /// Deprecated: use `provider.sort.partition`. Enum: "fallback" | "sort".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,

    /// Reasoning control for thinking models.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<Reasoning>,

    /// Prompt cache control (Anthropic models).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,

    /// Session ID for grouping related requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    /// Observability/tracing metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<Trace>,

    /// Debug options (e.g. echo upstream body).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug: Option<DebugOptions>,

    /// Provider-specific image generation config.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_config: Option<ImageConfig>,

    /// Any additional fields not explicitly modeled.
    #[serde(flatten)]
    pub extra: Option<HashMap<String, JsonValue>>,
}

impl Default for CreateChatCompletionRequest {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            model: None,
            models: None,
            audio: None,
            frequency_penalty: None,
            function_call: None,
            functions: None,
            logit_bias: None,
            logprobs: None,
            max_completion_tokens: None,
            max_tokens: None,
            metadata: None,
            modalities: None,
            n: None,
            parallel_tool_calls: None,
            prediction: None,
            presence_penalty: None,
            response_format: None,
            seed: None,
            service_tier: None,
            stop: None,
            store: None,
            stream: None,
            stream_options: None,
            temperature: None,
            tool_choice: None,
            tools: None,
            top_logprobs: None,
            top_p: None,
            user: None,
            web_search_options: None,
            provider: None,
            plugins: None,
            route: None,
            reasoning: None,
            cache_control: None,
            session_id: None,
            trace: None,
            debug: None,
            image_config: None,
            extra: None,
        }
    }
}
