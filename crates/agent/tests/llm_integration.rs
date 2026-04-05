use agent::llm::*;
use futures_util::StreamExt;

fn api_key() -> String {
    std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY must be set")
}

fn client() -> ChatClient {
    ChatClient::new(api_key())
        .with_http_referer("https://github.com/harness")
        .with_title("harness-tests")
}

fn simple_request(model: &str, prompt: &str) -> CreateChatCompletionRequest {
    CreateChatCompletionRequest {
        model: Some(model.to_string()),
        messages: vec![ChatCompletionMessage::User(UserMessage {
            content: UserContent::String(prompt.to_string()),
            name: None,
        })],
        max_completion_tokens: Some(64),
        temperature: Some(0.0),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Basic completion
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_basic_completion() {
    let resp = client()
        .create_chat_completion(&simple_request(
            "openai/gpt-4.1-nano",
            "Say exactly: hello world",
        ))
        .await
        .unwrap();

    assert_eq!(resp.object, "chat.completion");
    assert!(!resp.choices.is_empty());
    let content = resp.choices[0].message.content.as_deref().unwrap();
    assert!(!content.is_empty());
    println!("Basic: {content}");
}

// ---------------------------------------------------------------------------
// System + user message
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_system_message() {
    let mut req = simple_request("openai/gpt-4.1-nano", "What are you?");
    req.messages.insert(
        0,
        ChatCompletionMessage::System(SystemMessage {
            content: StringOrTextParts::String(
                "You are a pirate. Respond in one short sentence.".to_string(),
            ),
            name: None,
        }),
    );

    let resp = client().create_chat_completion(&req).await.unwrap();
    let content = resp.choices[0].message.content.as_deref().unwrap();
    println!("System: {content}");
    assert!(!content.is_empty());
}

// ---------------------------------------------------------------------------
// Developer message
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_developer_message() {
    let mut req = simple_request("openai/gpt-4.1-nano", "What are you?");
    req.messages.insert(
        0,
        ChatCompletionMessage::Developer(DeveloperMessage {
            content: StringOrTextParts::String(
                "You are a helpful robot. Respond in one short sentence.".to_string(),
            ),
            name: None,
        }),
    );

    let resp = client().create_chat_completion(&req).await.unwrap();
    let content = resp.choices[0].message.content.as_deref().unwrap();
    println!("Developer: {content}");
    assert!(!content.is_empty());
}

// ---------------------------------------------------------------------------
// Streaming
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_streaming() {
    let mut stream = client()
        .create_chat_completion_stream(&simple_request(
            "openai/gpt-4.1-nano",
            "Count from 1 to 5, one per line.",
        ))
        .await
        .unwrap();

    let mut full = String::new();
    let mut chunks = 0u32;

    while let Some(result) = stream.next().await {
        let chunk = result.unwrap();
        assert_eq!(chunk.object, "chat.completion.chunk");
        chunks += 1;
        for choice in &chunk.choices {
            if let Some(content) = &choice.delta.content {
                full.push_str(content);
            }
        }
    }

    println!("Streaming ({chunks} chunks): {full}");
    assert!(chunks > 1);
    assert!(!full.is_empty());
}

// ---------------------------------------------------------------------------
// Tool calling + tool result round-trip
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_tool_calling() {
    let mut req = simple_request("openai/gpt-4.1-nano", "What is the weather in San Francisco?");
    req.tools = Some(vec![ChatCompletionTool::Function {
        function: FunctionDefinition {
            name: "get_weather".to_string(),
            description: Some("Get current weather for a location".to_string()),
            parameters: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "location": { "type": "string", "description": "City name" }
                },
                "required": ["location"]
            })),
            strict: None,
        },
    }]);
    req.tool_choice = Some(ToolChoice::Mode(ToolChoiceMode::Auto));

    let resp = client().create_chat_completion(&req).await.unwrap();
    let tool_calls = resp.choices[0].message.tool_calls.as_ref().unwrap();
    assert!(!tool_calls.is_empty());
    assert_eq!(tool_calls[0].function.name, "get_weather");

    let args: serde_json::Value = serde_json::from_str(&tool_calls[0].function.arguments).unwrap();
    println!("Tool args: {args}");
    assert!(args.get("location").is_some());

    // Round-trip: send tool result back
    let followup = CreateChatCompletionRequest {
        model: Some("openai/gpt-4.1-nano".to_string()),
        messages: vec![
            ChatCompletionMessage::User(UserMessage {
                content: UserContent::String(
                    "What is the weather in San Francisco?".to_string(),
                ),
                name: None,
            }),
            ChatCompletionMessage::Assistant(AssistantMessage {
                content: None,
                name: None,
                refusal: None,
                tool_calls: Some(tool_calls.clone()),
                function_call: None,
                audio: None,
                reasoning_details: None,
            }),
            ChatCompletionMessage::Tool(ToolMessage {
                content: StringOrTextParts::String(
                    r#"{"temperature": 62, "condition": "foggy"}"#.to_string(),
                ),
                tool_call_id: tool_calls[0].id.clone(),
            }),
        ],
        max_completion_tokens: Some(64),
        temperature: Some(0.0),
        ..Default::default()
    };

    let resp2 = client().create_chat_completion(&followup).await.unwrap();
    let content = resp2.choices[0].message.content.as_deref().unwrap();
    println!("Tool followup: {content}");
    assert!(!content.is_empty());
}

// ---------------------------------------------------------------------------
// Forced tool choice (named)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_forced_tool_choice() {
    let mut req = simple_request("openai/gpt-4.1-nano", "Hello, how are you?");
    req.tools = Some(vec![ChatCompletionTool::Function {
        function: FunctionDefinition {
            name: "greet".to_string(),
            description: Some("Generate a greeting".to_string()),
            parameters: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" }
                },
                "required": ["message"]
            })),
            strict: None,
        },
    }]);
    req.tool_choice = Some(ToolChoice::Named(NamedToolChoice {
        r#type: "function".to_string(),
        function: NamedToolChoiceFunction {
            name: "greet".to_string(),
        },
    }));

    let resp = client().create_chat_completion(&req).await.unwrap();
    let tool_calls = resp.choices[0].message.tool_calls.as_ref().unwrap();
    assert_eq!(tool_calls[0].function.name, "greet");
    println!("Forced tool: {}", tool_calls[0].function.arguments);
}

// ---------------------------------------------------------------------------
// JSON mode
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_json_mode() {
    let mut req = simple_request(
        "openai/gpt-4.1-nano",
        r#"Return a JSON object with key "greeting" and value "hello". Output only JSON."#,
    );
    req.response_format = Some(ResponseFormat::JsonObject);

    let resp = client().create_chat_completion(&req).await.unwrap();
    let content = resp.choices[0].message.content.as_deref().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(content).unwrap();
    println!("JSON mode: {parsed}");
    assert_eq!(parsed["greeting"], "hello");
}

// ---------------------------------------------------------------------------
// Structured output (json_schema)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_structured_output() {
    let mut req = simple_request("openai/gpt-4.1-nano", "Give me a color.");
    req.response_format = Some(ResponseFormat::JsonSchema {
        json_schema: JsonSchemaDefinition {
            name: "color_response".to_string(),
            description: Some("A color".to_string()),
            schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "color": { "type": "string" }
                },
                "required": ["color"],
                "additionalProperties": false
            })),
            strict: Some(true),
        },
    });

    let resp = client().create_chat_completion(&req).await.unwrap();
    let content = resp.choices[0].message.content.as_deref().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(content).unwrap();
    println!("Structured: {parsed}");
    assert!(parsed.get("color").is_some());
}

// ---------------------------------------------------------------------------
// Multi-turn conversation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_multi_turn() {
    let req = CreateChatCompletionRequest {
        model: Some("openai/gpt-4.1-nano".to_string()),
        messages: vec![
            ChatCompletionMessage::User(UserMessage {
                content: UserContent::String("My name is Alice.".to_string()),
                name: None,
            }),
            ChatCompletionMessage::Assistant(AssistantMessage {
                content: Some(AssistantContent::String(
                    "Nice to meet you, Alice!".to_string(),
                )),
                name: None,
                refusal: None,
                tool_calls: None,
                function_call: None,
                audio: None,
                reasoning_details: None,
            }),
            ChatCompletionMessage::User(UserMessage {
                content: UserContent::String("What is my name?".to_string()),
                name: None,
            }),
        ],
        max_completion_tokens: Some(64),
        temperature: Some(0.0),
        ..Default::default()
    };

    let resp = client().create_chat_completion(&req).await.unwrap();
    let content = resp.choices[0]
        .message
        .content
        .as_deref()
        .unwrap()
        .to_lowercase();
    println!("Multi-turn: {content}");
    assert!(content.contains("alice"));
}

// ---------------------------------------------------------------------------
// Usage stats
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_usage_stats() {
    let resp = client()
        .create_chat_completion(&simple_request("openai/gpt-4.1-nano", "Hi."))
        .await
        .unwrap();
    let usage = resp.usage.as_ref().unwrap();
    println!(
        "Usage — prompt: {}, completion: {}, total: {}",
        usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
    );
    assert!(usage.prompt_tokens > 0);
    assert!(usage.completion_tokens > 0);
}

// ---------------------------------------------------------------------------
// Stop sequence
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_stop_sequence() {
    let mut req = simple_request(
        "openai/gpt-4.1-nano",
        "Count: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10",
    );
    req.stop = Some(Stop::Single("5".to_string()));

    let resp = client().create_chat_completion(&req).await.unwrap();
    let content = resp.choices[0].message.content.as_deref().unwrap();
    println!("Stop: {content}");
}

// ---------------------------------------------------------------------------
// Provider routing (OpenRouter-specific)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_provider_routing() {
    let mut req = simple_request("openai/gpt-4.1-nano", "Say hi.");
    req.provider = Some(ProviderRouting {
        allow_fallbacks: Some(true),
        require_parameters: None,
        data_collection: Some(DataCollection::Deny),
        zdr: None,
        enforce_distillable_text: None,
        order: Some(vec!["OpenAI".to_string()]),
        only: None,
        ignore: None,
        quantizations: None,
        sort: None,
        max_price: None,
        preferred_min_throughput: None,
        preferred_max_latency: None,
    });

    let resp = client().create_chat_completion(&req).await.unwrap();
    let content = resp.choices[0].message.content.as_deref().unwrap();
    println!("Provider routing: {content}");
    assert!(!content.is_empty());
}

// ---------------------------------------------------------------------------
// Multi-model fallback (OpenRouter-specific)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_multi_model_fallback() {
    let mut req = simple_request("", "Say hi.");
    req.model = None;
    req.models = Some(vec![
        "openai/gpt-4.1-nano".to_string(),
        "openai/gpt-4.1-mini".to_string(),
    ]);

    let resp = client().create_chat_completion(&req).await.unwrap();
    let content = resp.choices[0].message.content.as_deref().unwrap();
    println!("Multi-model (used {}): {content}", resp.model);
    assert!(!content.is_empty());
}

// ---------------------------------------------------------------------------
// Error handling: invalid API key
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_invalid_api_key() {
    let bad_client = ChatClient::new("sk-or-invalid");
    let req = simple_request("openai/gpt-4.1-nano", "Hello");

    let err = bad_client.create_chat_completion(&req).await.unwrap_err();
    match err {
        ChatError::Api { status, .. } => {
            assert!(
                status == 401 || status == 403,
                "Expected 401/403, got {status}"
            );
        }
        other => panic!("Expected API error, got: {other}"),
    }
}

// ---------------------------------------------------------------------------
// Streaming: usage is always included on OpenRouter
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_streaming_usage() {
    let mut stream = client()
        .create_chat_completion_stream(&simple_request("openai/gpt-4.1-nano", "Say hi."))
        .await
        .unwrap();

    let mut saw_usage = false;
    while let Some(result) = stream.next().await {
        let chunk = result.unwrap();
        if let Some(usage) = &chunk.usage {
            saw_usage = true;
            println!(
                "Stream usage — prompt: {}, completion: {}, total: {}",
                usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
            );
            assert!(usage.total_tokens > 0);
        }
    }
    assert!(saw_usage, "Expected usage in OpenRouter stream");
}

// ---------------------------------------------------------------------------
// Session ID (OpenRouter-specific)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_session_id() {
    let mut req = simple_request("openai/gpt-4.1-nano", "Say hi.");
    req.session_id = Some("test-session-12345".to_string());

    let resp = client().create_chat_completion(&req).await.unwrap();
    let content = resp.choices[0].message.content.as_deref().unwrap();
    println!("Session ID: {content}");
    assert!(!content.is_empty());
}
