use std::collections::HashMap;
use std::sync::{Arc, Mutex};

extern crate agent as agent_crate;

use agent_crate::agent;
use agent_crate::conversation::Conversation;
use agent_crate::llm::ChatClient;
use agent_crate::tools::{AdditionTool, ToolRegistry};
use serde_json::Value;
use storage::ConversationStore;

// ---------------------------------------------------------------------------
// In-memory store for tests
// ---------------------------------------------------------------------------

struct MemStore {
    meta: Mutex<HashMap<String, Value>>,
    msgs: Mutex<HashMap<String, Vec<Value>>>,
}

impl MemStore {
    fn new() -> Self {
        Self {
            meta: Mutex::new(HashMap::new()),
            msgs: Mutex::new(HashMap::new()),
        }
    }
}

#[derive(Debug)]
struct MemErr;
impl std::fmt::Display for MemErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MemErr")
    }
}
impl std::error::Error for MemErr {}

impl ConversationStore for MemStore {
    type Error = MemErr;
    fn list(&self) -> Result<Vec<String>, Self::Error> {
        Ok(self.meta.lock().unwrap().keys().cloned().collect())
    }
    fn load_metadata(&self, id: &str) -> Result<Value, Self::Error> {
        Ok(self.meta.lock().unwrap().get(id).cloned().unwrap_or(Value::Null))
    }
    fn load_messages(&self, id: &str) -> Result<Vec<Value>, Self::Error> {
        Ok(self.msgs.lock().unwrap().get(id).cloned().unwrap_or_default())
    }
    fn save_metadata(&self, id: &str, metadata: &Value) -> Result<(), Self::Error> {
        self.meta.lock().unwrap().insert(id.to_string(), metadata.clone());
        Ok(())
    }
    fn append_message(&self, id: &str, message: &Value) -> Result<(), Self::Error> {
        self.msgs.lock().unwrap().entry(id.to_string()).or_default().push(message.clone());
        Ok(())
    }
    fn delete(&self, id: &str) -> Result<(), Self::Error> {
        self.meta.lock().unwrap().remove(id);
        self.msgs.lock().unwrap().remove(id);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn api_key() -> String {
    std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY must be set")
}

fn client() -> ChatClient {
    ChatClient::new(api_key())
        .with_http_referer("https://github.com/harness")
        .with_title("harness-tests")
}

// ---------------------------------------------------------------------------
// 1. No tool calls → exits immediately
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_no_tool_calls_exits() {
    let client = client();
    let store = MemStore::new();
    let tools = Arc::new(ToolRegistry::new()); // no tools registered

    let mut conv = Conversation::new("test-no-tools")
        .with_model("openai/gpt-4.1-nano");

    let result = agent::run(&client, &store, &mut conv, tools, "Say exactly: hello", None, None)
        .await
        .unwrap();

    assert!(!result.reply.is_empty(), "should return non-empty text");
    // Conversation should have exactly 2 messages: user + assistant
    assert_eq!(conv.messages.len(), 2);
    println!("No tools result: {}", result.reply);
}

// ---------------------------------------------------------------------------
// 2. Tool calls cause the loop to iterate
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_tool_calls_loop() {
    let client = client();
    let store = MemStore::new();
    let tools = Arc::new(ToolRegistry::new().register(AdditionTool));

    let mut conv = Conversation::new("test-tool-loop")
        .with_model("openai/gpt-4.1-nano");

    let result = agent::run(
        &client,
        &store,
        &mut conv,
        tools,
        "Use the addition tool to add 2 and 3. Return only the numeric result.",
        None,
        None,
    )
    .await
    .unwrap();

    // Should have looped: user → assistant(tool_call) → tool_result → assistant(final)
    assert!(
        conv.messages.len() >= 4,
        "expected at least 4 messages (user, assistant+tool_call, tool, assistant), got {}",
        conv.messages.len()
    );
    println!("Tool loop result: {}", result.reply);
    println!("Message count: {}", conv.messages.len());
}

// ---------------------------------------------------------------------------
// 3. Live end-to-end: call one tool then exit with correct answer
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_live_agent_addition() {
    let client = client();
    let store = MemStore::new();
    let tools = Arc::new(ToolRegistry::new().register(AdditionTool));

    let mut conv = Conversation::new("test-live-addition")
        .with_model("openai/gpt-4.1-nano");

    let result = agent::run(
        &client,
        &store,
        &mut conv,
        tools,
        "Use the addition tool to compute 17 + 25. Reply with only the number, nothing else.",
        None,
        None,
    )
    .await
    .unwrap();

    println!("Live agent result: {}", result.reply);
    assert!(
        result.reply.contains("42"),
        "expected result to contain 42, got: {}",
        result.reply
    );

    // Verify the conversation structure:
    // 1: user message
    // 2: assistant with tool_call
    // 3: tool result
    // 4: assistant final answer
    assert!(
        conv.messages.len() >= 4,
        "expected at least 4 messages, got {}",
        conv.messages.len()
    );
}
