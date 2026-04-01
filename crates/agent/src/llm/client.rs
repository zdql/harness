use std::pin::Pin;

use futures_util::{Stream, StreamExt};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};

use super::request::CreateChatCompletionRequest;
use super::response::ChatCompletion;
use super::streaming::ChatCompletionChunk;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum ChatError {
    Http(reqwest::Error),
    Api { status: u16, body: String },
    Deserialize(serde_json::Error),
    Stream(String),
}

impl std::fmt::Display for ChatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(e) => write!(f, "HTTP error: {e}"),
            Self::Api { status, body } => write!(f, "API error ({status}): {body}"),
            Self::Deserialize(e) => write!(f, "Deserialization error: {e}"),
            Self::Stream(msg) => write!(f, "Stream error: {msg}"),
        }
    }
}

impl std::error::Error for ChatError {}

impl From<reqwest::Error> for ChatError {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(e)
    }
}

impl From<serde_json::Error> for ChatError {
    fn from(e: serde_json::Error) -> Self {
        Self::Deserialize(e)
    }
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";

#[derive(Debug, Clone)]
pub struct ChatClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    http_referer: Option<String>,
    x_title: Option<String>,
    x_categories: Option<String>,
}

impl ChatClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: DEFAULT_BASE_URL.to_string(),
            api_key: api_key.into(),
            http_referer: None,
            x_title: None,
            x_categories: None,
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Set `HTTP-Referer` header — your app's URL for OpenRouter rankings.
    pub fn with_http_referer(mut self, referer: impl Into<String>) -> Self {
        self.http_referer = Some(referer.into());
        self
    }

    /// Set `X-OpenRouter-Title` header — app display name on dashboard.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.x_title = Some(title.into());
        self
    }

    /// Set `X-OpenRouter-Categories` header — comma-separated app categories.
    pub fn with_categories(mut self, categories: impl Into<String>) -> Self {
        self.x_categories = Some(categories.into());
        self
    }

    pub fn with_http_client(mut self, client: reqwest::Client) -> Self {
        self.http = client;
        self
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.api_key))
                .expect("invalid api key characters"),
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if let Some(ref referer) = self.http_referer {
            headers.insert(
                "HTTP-Referer",
                HeaderValue::from_str(referer).expect("invalid referer characters"),
            );
        }
        if let Some(ref title) = self.x_title {
            headers.insert(
                "X-OpenRouter-Title",
                HeaderValue::from_str(title).expect("invalid title characters"),
            );
        }
        if let Some(ref categories) = self.x_categories {
            headers.insert(
                "X-OpenRouter-Categories",
                HeaderValue::from_str(categories).expect("invalid categories characters"),
            );
        }
        headers
    }

    // -----------------------------------------------------------------------
    // Non-streaming completion
    // -----------------------------------------------------------------------

    pub async fn create_chat_completion(
        &self,
        request: &CreateChatCompletionRequest,
    ) -> Result<ChatCompletion, ChatError> {
        let url = format!("{}/chat/completions", self.base_url);

        let resp = self
            .http
            .post(&url)
            .headers(self.headers())
            .json(request)
            .send()
            .await?;

        let status = resp.status().as_u16();
        if status >= 400 {
            let body = resp.text().await.unwrap_or_default();
            return Err(ChatError::Api { status, body });
        }

        let body = resp.text().await?;
        let completion: ChatCompletion = serde_json::from_str(&body)?;
        Ok(completion)
    }

    // -----------------------------------------------------------------------
    // Streaming completion
    // -----------------------------------------------------------------------

    pub async fn create_chat_completion_stream(
        &self,
        request: &CreateChatCompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatCompletionChunk, ChatError>> + Send>>, ChatError>
    {
        let url = format!("{}/chat/completions", self.base_url);

        // Ensure stream is set in the serialized body.
        let mut body = serde_json::to_value(request)?;
        body.as_object_mut()
            .expect("request should serialize to object")
            .insert("stream".to_string(), serde_json::Value::Bool(true));

        let resp = self
            .http
            .post(&url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await?;

        let status = resp.status().as_u16();
        if status >= 400 {
            let body = resp.text().await.unwrap_or_default();
            return Err(ChatError::Api { status, body });
        }

        let stream = resp.bytes_stream();

        // SSE parser: accumulate bytes, split on double-newline, parse `data:` lines.
        let parsed = futures_util::stream::unfold(
            (stream, String::new()),
            |(mut stream, mut buffer)| async move {
                loop {
                    // Try to extract a complete SSE event from the buffer.
                    if let Some(pos) = buffer.find("\n\n") {
                        let event = buffer[..pos].to_string();
                        buffer = buffer[pos + 2..].to_string();

                        for line in event.lines() {
                            let line = line.trim();
                            if let Some(data) = line.strip_prefix("data:") {
                                let data = data.trim();
                                if data == "[DONE]" {
                                    return None;
                                }
                                match serde_json::from_str::<ChatCompletionChunk>(data) {
                                    Ok(chunk) => return Some((Ok(chunk), (stream, buffer))),
                                    Err(e) => {
                                        return Some((Err(ChatError::from(e)), (stream, buffer)))
                                    }
                                }
                            }
                        }
                        continue;
                    }

                    // Need more data from the network.
                    match stream.next().await {
                        Some(Ok(bytes)) => {
                            buffer.push_str(&String::from_utf8_lossy(&bytes));
                        }
                        Some(Err(e)) => {
                            return Some((Err(ChatError::Http(e)), (stream, buffer)));
                        }
                        None => {
                            if buffer.trim().is_empty() {
                                return None;
                            }
                            for line in buffer.lines() {
                                let line = line.trim();
                                if let Some(data) = line.strip_prefix("data:") {
                                    let data = data.trim();
                                    if data == "[DONE]" {
                                        return None;
                                    }
                                    match serde_json::from_str::<ChatCompletionChunk>(data) {
                                        Ok(chunk) => {
                                            return Some((
                                                Ok(chunk),
                                                (stream, String::new()),
                                            ))
                                        }
                                        Err(e) => {
                                            return Some((
                                                Err(ChatError::from(e)),
                                                (stream, String::new()),
                                            ))
                                        }
                                    }
                                }
                            }
                            return None;
                        }
                    }
                }
            },
        );

        Ok(Box::pin(parsed))
    }
}
