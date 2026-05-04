// ---------------------------------------------------------------------------
// ChatBackend — the LLM call surface the agent code depends on.
//
// The agent loop only ever needs two operations: a streaming completion (for
// the agent turn) and a non-streaming completion (for compaction/summarize).
// Hiding both behind a trait means tests can plug in a deterministic mock
// instead of standing up a fake HTTP server.
// ---------------------------------------------------------------------------

use std::pin::Pin;

use async_trait::async_trait;
use futures_util::Stream;

use super::client::{ChatClient, ChatError};
use super::request::CreateChatCompletionRequest;
use super::response::ChatCompletion;
use super::streaming::ChatCompletionChunk;

pub type ChatStream = Pin<Box<dyn Stream<Item = Result<ChatCompletionChunk, ChatError>> + Send>>;

#[async_trait]
pub trait ChatBackend: Send + Sync {
    async fn create_chat_completion(
        &self,
        request: &CreateChatCompletionRequest,
    ) -> Result<ChatCompletion, ChatError>;

    async fn create_chat_completion_stream(
        &self,
        request: &CreateChatCompletionRequest,
    ) -> Result<ChatStream, ChatError>;
}

#[async_trait]
impl ChatBackend for ChatClient {
    async fn create_chat_completion(
        &self,
        request: &CreateChatCompletionRequest,
    ) -> Result<ChatCompletion, ChatError> {
        ChatClient::create_chat_completion(self, request).await
    }

    async fn create_chat_completion_stream(
        &self,
        request: &CreateChatCompletionRequest,
    ) -> Result<ChatStream, ChatError> {
        ChatClient::create_chat_completion_stream(self, request).await
    }
}
