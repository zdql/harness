use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};
use std::fs;

use crate::llm::{ChatCompletionTool, FunctionDefinition};

use super::{Tool, ToolError, run_blocking};

pub struct ReadTool;

#[derive(Deserialize)]
struct Args {
    file_path: String,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }

    fn definition(&self) -> ChatCompletionTool {
        ChatCompletionTool::Function {
            function: FunctionDefinition {
                name: "read".to_string(),
                description: Some(
                    "Read a file's contents. Returns numbered lines. Supports offset and limit for large files.".to_string(),
                ),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Absolute path to the file to read"
                        },
                        "offset": {
                            "type": "integer",
                            "description": "1-based line number to start reading from"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of lines to read (default: 2000)"
                        }
                    },
                    "required": ["file_path"],
                    "additionalProperties": false
                })),
                strict: Some(false),
            },
        }
    }

    async fn call(&self, arguments: &str) -> Result<JsonValue, ToolError> {
        let args: Args =
            serde_json::from_str(arguments).map_err(|e| ToolError(format!("bad args: {e}")))?;

        run_blocking(move || {
            let content = fs::read_to_string(&args.file_path)
                .map_err(|e| ToolError(format!("failed to read {}: {e}", args.file_path)))?;

            let lines: Vec<&str> = content.lines().collect();
            let total_lines = lines.len();

            let offset = args.offset.unwrap_or(1).max(1) - 1; // convert to 0-based
            let limit = args.limit.unwrap_or(2000);

            let selected: Vec<String> = lines
                .iter()
                .enumerate()
                .skip(offset)
                .take(limit)
                .map(|(i, line)| format!("{:>6}\t{}", i + 1, line))
                .collect();

            Ok(json!({
                "content": selected.join("\n"),
                "total_lines": total_lines
            }))
        })
        .await
    }
}
