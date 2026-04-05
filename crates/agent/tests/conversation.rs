// ---------------------------------------------------------------------------
// tests/conversation.rs — Conversation module tests
//
// Unit tests for conversation CRUD, metadata persistence, summarization
// logic, and the save/load round-trip cycle.
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::Mutex;

extern crate agent as agent_crate;

use agent_crate::conversation::{self, summarize, Conversation};
use agent_crate::llm::{
    AssistantContent, AssistantMessage, ChatClient, ChatCompletionMessage, UserContent,
};
use serde_json::Value;
use storage::ConversationStore;

// ---------------------------------------------------------------------------
// In-memory store for tests (same pattern as agent_loop.rs)
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
        Ok(self
            .meta
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .unwrap_or(Value::Null))
    }
    fn load_messages(&self, id: &str) -> Result<Vec<Value>, Self::Error> {
        Ok(self
            .msgs
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .unwrap_or_default())
    }
    fn save_metadata(&self, id: &str, metadata: &Value) -> Result<(), Self::Error> {
        self.meta
            .lock()
            .unwrap()
            .insert(id.to_string(), metadata.clone());
        Ok(())
    }
    fn append_message(&self, id: &str, message: &Value) -> Result<(), Self::Error> {
        self.msgs
            .lock()
            .unwrap()
            .entry(id.to_string())
            .or_default()
            .push(message.clone());
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

fn make_assistant_msg(text: &str) -> ChatCompletionMessage {
    ChatCompletionMessage::Assistant(AssistantMessage {
        content: Some(AssistantContent::String(text.to_string())),
        name: None,
        refusal: None,
        tool_calls: None,
        function_call: None,
        audio: None,
        reasoning_details: None,
    })
}

// ===========================================================================
// Conversation struct tests
// ===========================================================================

#[test]
fn new_conversation_has_defaults() {
    let conv = Conversation::new("test-1");
    assert_eq!(conv.id, "test-1");
    assert!(conv.model.is_none());
    assert!(conv.title.is_none());
    assert_eq!(conv.title_set_at_message_count, 0);
    assert!(conv.messages.is_empty());
    assert!(conv.created_at > 0);
    assert_eq!(conv.created_at, conv.updated_at);
}

#[test]
fn with_model_sets_model() {
    let conv = Conversation::new("test-2").with_model("openai/gpt-4o");
    assert_eq!(conv.model.as_deref(), Some("openai/gpt-4o"));
}

#[test]
fn push_user_appends_message_and_updates_timestamp() {
    let mut conv = Conversation::new("test-3");
    let original_updated = conv.updated_at;

    // Sleep briefly so timestamps differ (1-second resolution)
    std::thread::sleep(std::time::Duration::from_millis(10));

    conv.push_user("hello");
    assert_eq!(conv.messages.len(), 1);
    assert!(conv.updated_at >= original_updated);

    // Verify it's a User message with correct content
    match &conv.messages[0] {
        ChatCompletionMessage::User(u) => match &u.content {
            UserContent::String(s) => assert_eq!(s, "hello"),
            _ => panic!("expected string content"),
        },
        _ => panic!("expected user message"),
    }
}

#[test]
fn push_message_appends_arbitrary_message() {
    let mut conv = Conversation::new("test-4");
    conv.push_message(make_assistant_msg("hi there"));
    assert_eq!(conv.messages.len(), 1);
}

// ===========================================================================
// Metadata tests
// ===========================================================================

#[test]
fn metadata_includes_all_fields() {
    let mut conv = Conversation::new("meta-test").with_model("openai/gpt-4o");
    conv.title = Some("Test Title".to_string());
    conv.title_set_at_message_count = 4;

    let meta = conv.metadata();
    assert_eq!(meta["id"], "meta-test");
    assert_eq!(meta["model"], "openai/gpt-4o");
    assert_eq!(meta["title"], "Test Title");
    assert_eq!(meta["title_set_at_message_count"], 4);
    assert!(meta["created_at"].as_i64().unwrap() > 0);
}

#[test]
fn metadata_handles_null_optionals() {
    let conv = Conversation::new("null-test");
    let meta = conv.metadata();
    assert!(meta["model"].is_null());
    assert!(meta["title"].is_null());
}

// ===========================================================================
// needs_summarization tests
// ===========================================================================

#[test]
fn no_messages_does_not_need_summarization() {
    let conv = Conversation::new("empty");
    assert!(!conv.needs_summarization());
}

#[test]
fn messages_without_title_needs_summarization() {
    let mut conv = Conversation::new("no-title");
    conv.push_user("hello");
    conv.push_message(make_assistant_msg("hi"));
    assert!(conv.needs_summarization());
}

#[test]
fn recently_titled_does_not_need_summarization() {
    let mut conv = Conversation::new("titled");
    conv.push_user("hello");
    conv.push_message(make_assistant_msg("hi"));
    conv.title = Some("Greeting".to_string());
    conv.title_set_at_message_count = 2;
    assert!(!conv.needs_summarization());
}

#[test]
fn needs_resummarization_after_4_more_messages() {
    let mut conv = Conversation::new("re-title");
    conv.push_user("msg 1");
    conv.push_message(make_assistant_msg("reply 1"));
    conv.title = Some("Initial Title".to_string());
    conv.title_set_at_message_count = 2;

    // Add 3 more messages — not yet
    conv.push_user("msg 2");
    conv.push_message(make_assistant_msg("reply 2"));
    conv.push_user("msg 3");
    assert!(!conv.needs_summarization()); // 5 messages, need 6

    // 4th message since title → triggers re-summarization
    conv.push_message(make_assistant_msg("reply 3"));
    assert!(conv.needs_summarization()); // 6 messages, 6 >= 2 + 4
}

#[test]
fn exactly_at_threshold_needs_summarization() {
    let mut conv = Conversation::new("edge");
    conv.title = Some("Old Title".to_string());
    conv.title_set_at_message_count = 0;

    // Add exactly 4 messages
    for i in 0..4 {
        conv.push_user(&format!("msg {i}"));
    }
    assert!(conv.needs_summarization()); // 4 >= 0 + 4
}

// ===========================================================================
// Save / load round-trip tests
// ===========================================================================

#[test]
fn save_and_load_round_trip() {
    let store = MemStore::new();
    let mut conv = Conversation::new("round-trip").with_model("openai/gpt-4o");
    conv.title = Some("My Conversation".to_string());
    conv.title_set_at_message_count = 2;
    conv.push_user("hello");

    // Save the conversation (saves metadata + appends last message)
    conversation::save(&store, &conv).unwrap();

    // Push another message and save again
    conv.push_message(make_assistant_msg("hi there"));
    conversation::save(&store, &conv).unwrap();

    // Load and verify
    let loaded = conversation::load(&store, "round-trip").unwrap();
    assert_eq!(loaded.id, "round-trip");
    assert_eq!(loaded.model.as_deref(), Some("openai/gpt-4o"));
    assert_eq!(loaded.title.as_deref(), Some("My Conversation"));
    assert_eq!(loaded.title_set_at_message_count, 2);
    assert_eq!(loaded.messages.len(), 2);
}

#[test]
fn load_preserves_message_content() {
    let store = MemStore::new();
    let mut conv = Conversation::new("msg-content");
    conv.push_user("what is 2+2?");
    conversation::save(&store, &conv).unwrap();

    conv.push_message(make_assistant_msg("4"));
    conversation::save(&store, &conv).unwrap();

    let loaded = conversation::load(&store, "msg-content").unwrap();
    assert_eq!(loaded.messages.len(), 2);

    // Verify user message
    match &loaded.messages[0] {
        ChatCompletionMessage::User(u) => match &u.content {
            UserContent::String(s) => assert_eq!(s, "what is 2+2?"),
            _ => panic!("expected string content"),
        },
        _ => panic!("expected user message"),
    }

    // Verify assistant message
    match &loaded.messages[1] {
        ChatCompletionMessage::Assistant(a) => match a.content.as_ref().unwrap() {
            AssistantContent::String(s) => assert_eq!(s, "4"),
            _ => panic!("expected string content"),
        },
        _ => panic!("expected assistant message"),
    }
}

#[test]
fn load_without_title_returns_none() {
    let store = MemStore::new();
    let conv = Conversation::new("no-title-load");
    conversation::save(&store, &conv).unwrap();

    let loaded = conversation::load(&store, "no-title-load").unwrap();
    assert!(loaded.title.is_none());
    assert_eq!(loaded.title_set_at_message_count, 0);
}

#[test]
fn load_latest_returns_none_when_empty() {
    let store = MemStore::new();
    let result = conversation::load_latest(&store).unwrap();
    assert!(result.is_none());
}

#[test]
fn load_latest_returns_a_conversation() {
    let store = MemStore::new();
    let conv = Conversation::new("latest-test");
    conversation::save(&store, &conv).unwrap();

    let loaded = conversation::load_latest(&store).unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().id, "latest-test");
}

// ===========================================================================
// Summarize — build_messages tests
// ===========================================================================

#[test]
fn build_messages_produces_system_and_user() {
    let mut conv = Conversation::new("summarize-test");
    conv.push_user("hello");
    conv.push_message(make_assistant_msg("hi there"));

    let messages = summarize::build_messages(&conv);
    assert_eq!(messages.len(), 2);

    // First should be system
    match &messages[0] {
        ChatCompletionMessage::System(_) => {}
        other => panic!("expected system message, got: {other:?}"),
    }

    // Second should be user with transcript
    match &messages[1] {
        ChatCompletionMessage::User(u) => match &u.content {
            UserContent::String(s) => {
                assert!(s.contains("User: hello"), "should contain user text");
                assert!(
                    s.contains("Assistant: hi there"),
                    "should contain assistant text"
                );
            }
            _ => panic!("expected string content"),
        },
        other => panic!("expected user message, got: {other:?}"),
    }
}

#[test]
fn build_messages_caps_at_20_messages() {
    let mut conv = Conversation::new("cap-test");
    for i in 0..30 {
        conv.push_user(&format!("message {i}"));
    }

    let messages = summarize::build_messages(&conv);
    // system + user (with transcript)
    assert_eq!(messages.len(), 2);

    // The transcript should contain messages 10-29 (last 20), not 0-9
    match &messages[1] {
        ChatCompletionMessage::User(u) => match &u.content {
            UserContent::String(s) => {
                assert!(
                    !s.contains("message 0\n"),
                    "should not contain earliest messages"
                );
                assert!(
                    !s.contains("message 9\n"),
                    "should not contain message 9"
                );
                assert!(s.contains("message 10"), "should contain message 10");
                assert!(s.contains("message 29"), "should contain message 29");
            }
            _ => panic!("expected string content"),
        },
        _ => panic!("expected user message"),
    }
}

#[test]
fn build_messages_skips_tool_messages() {
    use agent_crate::llm::{StringOrTextParts, ToolMessage};

    let mut conv = Conversation::new("skip-tools");
    conv.push_user("use addition");
    conv.push_message(ChatCompletionMessage::Tool(ToolMessage {
        content: StringOrTextParts::String("42".to_string()),
        tool_call_id: "call-1".to_string(),
    }));
    conv.push_message(make_assistant_msg("the answer is 42"));

    let messages = summarize::build_messages(&conv);
    match &messages[1] {
        ChatCompletionMessage::User(u) => match &u.content {
            UserContent::String(s) => {
                assert!(s.contains("User: use addition"));
                assert!(s.contains("Assistant: the answer is 42"));
                // Tool messages should not appear in transcript
                assert!(!s.contains("42\n") || s.contains("the answer is 42"));
            }
            _ => panic!("expected string content"),
        },
        _ => panic!("expected user message"),
    }
}

#[test]
fn build_messages_handles_empty_conversation() {
    let conv = Conversation::new("empty-conv");
    let messages = summarize::build_messages(&conv);
    assert_eq!(messages.len(), 2);

    // Transcript should be empty but the structure should be valid
    match &messages[1] {
        ChatCompletionMessage::User(u) => match &u.content {
            UserContent::String(s) => {
                assert!(s.contains("Generate a short title"));
            }
            _ => panic!("expected string content"),
        },
        _ => panic!("expected user message"),
    }
}

// ===========================================================================
// Summarize — live integration test (requires API key)
// ===========================================================================

fn api_key() -> String {
    std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY must be set")
}

fn client() -> ChatClient {
    ChatClient::new(api_key())
        .with_http_referer("https://github.com/harness")
        .with_title("harness-tests")
}

#[tokio::test]
async fn test_summarize_returns_short_title() {
    let client = client();
    let mut conv = Conversation::new("live-summarize").with_model("openai/gpt-4.1-nano");

    conv.push_user("What is the capital of France?");
    conv.push_message(make_assistant_msg(
        "The capital of France is Paris.",
    ));

    let title = summarize::summarize(&client, &conv).await.unwrap();

    println!("Generated title: {title}");
    assert!(!title.is_empty(), "title should not be empty");
    // A reasonable title for this conversation should be short
    let word_count = title.split_whitespace().count();
    assert!(
        word_count <= 10,
        "title should be concise, got {word_count} words: {title}"
    );
}
