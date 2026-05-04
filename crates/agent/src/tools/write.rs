use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};
use std::fs;
use std::path::Path;

use crate::llm::{ChatCompletionTool, FunctionDefinition};

use super::{Tool, ToolError, run_blocking};

pub struct WriteTool;

#[derive(Deserialize)]
struct Args {
    file_path: String,
    content: String,
}

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }

    fn definition(&self) -> ChatCompletionTool {
        ChatCompletionTool::Function {
            function: FunctionDefinition {
                name: "write".to_string(),
                description: Some(
                    "Write content to a file. Creates parent directories if needed. Overwrites existing files.".to_string(),
                ),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Absolute path to the file to write"
                        },
                        "content": {
                            "type": "string",
                            "description": "The content to write to the file"
                        }
                    },
                    "required": ["file_path", "content"],
                    "additionalProperties": false
                })),
                strict: Some(true),
            },
        }
    }

    async fn call(&self, arguments: &str) -> Result<JsonValue, ToolError> {
        let args: Args =
            serde_json::from_str(arguments).map_err(|e| ToolError(format!("bad args: {e}")))?;

        run_blocking(move || {
            let path = Path::new(&args.file_path);

            // Create parent directories if they don't exist.
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| ToolError(format!("failed to create directories: {e}")))?;
            }

            fs::write(path, &args.content)
                .map_err(|e| ToolError(format!("failed to write {}: {e}", args.file_path)))?;

            let bytes = args.content.len();

            Ok(json!({
                "path": args.file_path,
                "bytes_written": bytes
            }))
        })
        .await
    }
}
