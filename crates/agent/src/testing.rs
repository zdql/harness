// ---------------------------------------------------------------------------
// Test harness — a deterministic [`ChatBackend`] for the agent loop.
//
// The agent crate has only one external I/O dependency (the LLM). Tests use
// [`MockBackend`] to script every LLM response, so we can drive the loop
// through complex flows (subagent spawn/drain, recursion, suspend/resume)
// with no network and no flakiness.
//
// Two ways to queue responses:
//   * [`MockBackend::push`] — FIFO; the next request gets the next response.
//   * [`MockBackend::push_matching`] — a predicate over the request; the
//     first queued entry whose predicate matches gets returned. Use this to
//     route parent vs. subagent responses when both use the same backend.
//
// First match wins: entries are scanned in insertion order, and an entry
// without a matcher matches anything.
// ---------------------------------------------------------------------------

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use futures_util::{Stream, stream};
use tokio::sync::Notify;

use crate::llm::{
    ChatBackend, ChatCompletion, ChatCompletionChunk, ChatCompletionMessage, ChatError, Choice,
    ChoiceDelta, ChunkChoice, CreateChatCompletionRequest, DeltaToolCall, DeltaToolCallFunction,
    FinishReason, ResponseMessage, ToolCall, ToolCallFunction, UserContent,
};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// One scripted backend response.
#[derive(Debug, Clone)]
pub enum MockResponse {
    /// Final reply (no tool calls).
    Text(String),
    /// One or more tool calls. `text` may be empty.
    ToolCalls {
        text: String,
        calls: Vec<MockToolCall>,
    },
    /// Force a stream-level error.
    Error(String),
    /// Sleep for `duration` before resolving with `inner`. Useful to slow
    /// down a subagent so the parent has time to suspend.
    Delayed {
        duration: Duration,
        inner: Box<MockResponse>,
    },
    /// Wait until `gate` is released (`gate.notify_one()` from the test) before
    /// resolving with `inner`. Lets tests deterministically observe the
    /// "subagent in flight" intermediate state.
    Gated {
        gate: Arc<Notify>,
        inner: Box<MockResponse>,
    },
}

impl MockResponse {
    pub fn delayed(duration: Duration, inner: MockResponse) -> Self {
        MockResponse::Delayed {
            duration,
            inner: Box::new(inner),
        }
    }

    pub fn gated(gate: Arc<Notify>, inner: MockResponse) -> Self {
        MockResponse::Gated {
            gate,
            inner: Box::new(inner),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MockToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

impl MockToolCall {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            arguments: arguments.into(),
        }
    }
}

type Matcher = Box<dyn Fn(&CreateChatCompletionRequest) -> bool + Send + Sync>;

struct Entry {
    matcher: Option<Matcher>,
    response: MockResponse,
    sticky: bool,
}

// ---------------------------------------------------------------------------
// MockBackend
// ---------------------------------------------------------------------------

pub struct MockBackend {
    queue: Mutex<Vec<Entry>>,
    requests: Mutex<Vec<CreateChatCompletionRequest>>,
}

impl Default for MockBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl MockBackend {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(Vec::new()),
            requests: Mutex::new(Vec::new()),
        }
    }

    /// Queue a response in FIFO order — matches any request, consumed on use.
    pub fn push(&self, response: MockResponse) -> &Self {
        self.queue.lock().unwrap().push(Entry {
            matcher: None,
            response,
            sticky: false,
        });
        self
    }

    pub fn push_text(&self, text: impl Into<String>) -> &Self {
        self.push(MockResponse::Text(text.into()))
    }

    pub fn push_tool_call(
        &self,
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: impl Into<String>,
    ) -> &Self {
        self.push(MockResponse::ToolCalls {
            text: String::new(),
            calls: vec![MockToolCall::new(id, name, arguments)],
        })
    }

    pub fn push_tool_calls(&self, text: impl Into<String>, calls: Vec<MockToolCall>) -> &Self {
        self.push(MockResponse::ToolCalls {
            text: text.into(),
            calls,
        })
    }

    pub fn push_error(&self, msg: impl Into<String>) -> &Self {
        self.push(MockResponse::Error(msg.into()))
    }

    /// Queue a response that only matches requests whose predicate returns true.
    /// Consumed on use.
    pub fn push_matching<F>(&self, matcher: F, response: MockResponse) -> &Self
    where
        F: Fn(&CreateChatCompletionRequest) -> bool + Send + Sync + 'static,
    {
        self.queue.lock().unwrap().push(Entry {
            matcher: Some(Box::new(matcher)),
            response,
            sticky: false,
        });
        self
    }

    /// Like [`push_matching`] but the entry is NOT consumed — every matching
    /// request gets the same response. Use for "always reply X when condition
    /// Y holds" scenarios (e.g. an idle-state response across many drains).
    pub fn push_sticky_matching<F>(&self, matcher: F, response: MockResponse) -> &Self
    where
        F: Fn(&CreateChatCompletionRequest) -> bool + Send + Sync + 'static,
    {
        self.queue.lock().unwrap().push(Entry {
            matcher: Some(Box::new(matcher)),
            response,
            sticky: true,
        });
        self
    }

    /// Queue a response routed by substring match in any user message.
    /// Consumed on use.
    pub fn push_when_user_contains(
        &self,
        needle: impl Into<String>,
        response: MockResponse,
    ) -> &Self {
        let needle = needle.into();
        self.push_matching(move |req| user_messages_contain(req, &needle), response)
    }

    /// Queue a sticky response routed by substring match — every matching
    /// request gets the same reply.
    pub fn push_sticky_when_user_contains(
        &self,
        needle: impl Into<String>,
        response: MockResponse,
    ) -> &Self {
        let needle = needle.into();
        self.push_sticky_matching(move |req| user_messages_contain(req, &needle), response)
    }

    /// Snapshot of every request observed so far.
    pub fn requests(&self) -> Vec<CreateChatCompletionRequest> {
        self.requests.lock().unwrap().clone()
    }

    pub fn request_count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }

    pub fn responses_remaining(&self) -> usize {
        self.queue.lock().unwrap().len()
    }

    fn record(&self, request: &CreateChatCompletionRequest) {
        self.requests.lock().unwrap().push(request.clone());
    }

    fn next_response(&self, request: &CreateChatCompletionRequest) -> MockResponse {
        let mut queue = self.queue.lock().unwrap();
        let idx = queue.iter().position(|e| match &e.matcher {
            Some(m) => m(request),
            None => true,
        });
        match idx {
            Some(i) => {
                if queue[i].sticky {
                    queue[i].response.clone()
                } else {
                    queue.remove(i).response
                }
            }
            None => panic!(
                "MockBackend: no scripted response matched request #{}. \
                 {} entries remain. Last user msg: {:?}",
                self.request_count() + 1,
                queue.len(),
                last_user_text(request),
            ),
        }
    }
}

#[async_trait]
impl ChatBackend for MockBackend {
    async fn create_chat_completion(
        &self,
        request: &CreateChatCompletionRequest,
    ) -> Result<ChatCompletion, ChatError> {
        self.record(request);
        let response = resolve(self.next_response(request)).await;
        match response {
            MockResponse::Text(text) => Ok(synthesize_completion(text, vec![])),
            MockResponse::ToolCalls { text, calls } => Ok(synthesize_completion(text, calls)),
            MockResponse::Error(msg) => Err(ChatError::Stream(msg)),
            MockResponse::Delayed { .. } | MockResponse::Gated { .. } => {
                unreachable!("resolve() unwraps wrappers")
            }
        }
    }

    async fn create_chat_completion_stream(
        &self,
        request: &CreateChatCompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatCompletionChunk, ChatError>> + Send>>, ChatError>
    {
        self.record(request);
        let response = resolve(self.next_response(request)).await;
        let chunk = match response {
            MockResponse::Text(text) => synthesize_chunk(text, vec![]),
            MockResponse::ToolCalls { text, calls } => synthesize_chunk(text, calls),
            MockResponse::Error(msg) => return Err(ChatError::Stream(msg)),
            MockResponse::Delayed { .. } | MockResponse::Gated { .. } => {
                unreachable!("resolve() unwraps wrappers")
            }
        };
        Ok(Box::pin(stream::iter(vec![Ok(chunk)])))
    }
}

/// Recursively unwrap `Delayed` / `Gated` wrappers, awaiting their preconditions.
async fn resolve(mut response: MockResponse) -> MockResponse {
    loop {
        match response {
            MockResponse::Delayed { duration, inner } => {
                tokio::time::sleep(duration).await;
                response = *inner;
            }
            MockResponse::Gated { gate, inner } => {
                gate.notified().await;
                response = *inner;
            }
            other => return other,
        }
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn user_messages_contain(req: &CreateChatCompletionRequest, needle: &str) -> bool {
    req.messages.iter().any(|m| match m {
        ChatCompletionMessage::User(u) => match &u.content {
            UserContent::String(s) => s.contains(needle),
            _ => false,
        },
        _ => false,
    })
}

fn last_user_text(req: &CreateChatCompletionRequest) -> Option<String> {
    req.messages.iter().rev().find_map(|m| match m {
        ChatCompletionMessage::User(u) => match &u.content {
            UserContent::String(s) => Some(s.clone()),
            _ => None,
        },
        _ => None,
    })
}

fn synthesize_completion(text: String, calls: Vec<MockToolCall>) -> ChatCompletion {
    let tool_calls = (!calls.is_empty()).then(|| {
        calls
            .into_iter()
            .map(|c| ToolCall {
                id: c.id,
                r#type: "function".to_string(),
                function: ToolCallFunction {
                    name: c.name,
                    arguments: c.arguments,
                },
            })
            .collect()
    });

    ChatCompletion {
        id: "mock".to_string(),
        object: "chat.completion".to_string(),
        created: 0,
        model: "mock".to_string(),
        choices: vec![Choice {
            index: 0,
            message: ResponseMessage {
                role: "assistant".to_string(),
                content: if text.is_empty() { None } else { Some(text) },
                refusal: None,
                tool_calls,
                function_call: None,
                audio: None,
                annotations: None,
                reasoning: None,
                reasoning_details: None,
                images: None,
            },
            finish_reason: Some(FinishReason::Stop),
            logprobs: None,
            error: None,
        }],
        usage: None,
        system_fingerprint: None,
        service_tier: None,
    }
}

fn synthesize_chunk(text: String, calls: Vec<MockToolCall>) -> ChatCompletionChunk {
    let tool_calls = (!calls.is_empty()).then(|| {
        calls
            .into_iter()
            .enumerate()
            .map(|(idx, c)| DeltaToolCall {
                index: idx as u32,
                id: Some(c.id),
                r#type: Some("function".to_string()),
                function: Some(DeltaToolCallFunction {
                    name: Some(c.name),
                    arguments: Some(c.arguments),
                }),
            })
            .collect()
    });

    ChatCompletionChunk {
        id: "mock".to_string(),
        object: "chat.completion.chunk".to_string(),
        created: 0,
        model: "mock".to_string(),
        choices: vec![ChunkChoice {
            index: 0,
            delta: ChoiceDelta {
                role: Some("assistant".to_string()),
                content: if text.is_empty() { None } else { Some(text) },
                refusal: None,
                tool_calls,
                function_call: None,
                reasoning: None,
                reasoning_details: None,
            },
            finish_reason: Some(FinishReason::Stop),
            logprobs: None,
            error: None,
        }],
        usage: None,
        system_fingerprint: None,
        service_tier: None,
        error: None,
    }
}

// ---------------------------------------------------------------------------
// In-memory ConversationStore for tests
// ---------------------------------------------------------------------------

use std::collections::HashMap;

use serde_json::Value;
use storage::ConversationStore;

#[derive(Debug)]
pub struct MemStoreError;
impl std::fmt::Display for MemStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MemStoreError")
    }
}
impl std::error::Error for MemStoreError {}

#[derive(Default)]
pub struct MemStore {
    inner: Arc<Mutex<MemStoreInner>>,
}

#[derive(Default)]
struct MemStoreInner {
    meta: HashMap<String, Value>,
    msgs: HashMap<String, Vec<Value>>,
}

impl MemStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Clone for MemStore {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl ConversationStore for MemStore {
    type Error = MemStoreError;

    fn list(&self) -> Result<Vec<String>, Self::Error> {
        Ok(self.inner.lock().unwrap().meta.keys().cloned().collect())
    }

    fn load_metadata(&self, id: &str) -> Result<Value, Self::Error> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .meta
            .get(id)
            .cloned()
            .unwrap_or(Value::Null))
    }

    fn load_messages(&self, id: &str) -> Result<Vec<Value>, Self::Error> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .msgs
            .get(id)
            .cloned()
            .unwrap_or_default())
    }

    fn save_metadata(&self, id: &str, metadata: &Value) -> Result<(), Self::Error> {
        self.inner
            .lock()
            .unwrap()
            .meta
            .insert(id.to_string(), metadata.clone());
        Ok(())
    }

    fn append_message(&self, id: &str, message: &Value) -> Result<(), Self::Error> {
        self.inner
            .lock()
            .unwrap()
            .msgs
            .entry(id.to_string())
            .or_default()
            .push(message.clone());
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<(), Self::Error> {
        let mut inner = self.inner.lock().unwrap();
        inner.meta.remove(id);
        inner.msgs.remove(id);
        Ok(())
    }
}
