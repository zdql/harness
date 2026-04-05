use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};

use crate::llm::{ChatCompletionTool, FunctionDefinition};

use super::{Tool, ToolError};

pub struct AdditionTool;

#[derive(Deserialize)]
struct Args {
    a: f64,
    b: f64,
}

#[async_trait]
impl Tool for AdditionTool {
    fn name(&self) -> &str {
        "addition"
    }

    fn definition(&self) -> ChatCompletionTool {
        ChatCompletionTool::Function {
            function: FunctionDefinition {
                name: "addition".to_string(),
                description: Some("Add two numbers together and return the result.".to_string()),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "a": { "type": "number", "description": "First number" },
                        "b": { "type": "number", "description": "Second number" }
                    },
                    "required": ["a", "b"],
                    "additionalProperties": false
                })),
                strict: Some(true),
            },
        }
    }

    async fn call(&self, arguments: &str) -> Result<JsonValue, ToolError> {
        let args: Args =
            serde_json::from_str(arguments).map_err(|e| ToolError(format!("bad args: {e}")))?;
        Ok(json!({ "result": args.a + args.b }))
    }
}
