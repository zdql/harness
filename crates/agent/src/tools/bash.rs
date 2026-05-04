use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};
use std::process::Command;

use crate::llm::{ChatCompletionTool, FunctionDefinition};

use super::{Tool, ToolError, run_blocking};

pub struct BashTool;

#[derive(Deserialize)]
struct Args {
    command: String,
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn definition(&self) -> ChatCompletionTool {
        ChatCompletionTool::Function {
            function: FunctionDefinition {
                name: "bash".to_string(),
                description: Some(
                    "Execute a bash command and return its stdout and stderr.".to_string(),
                ),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The bash command to execute"
                        },
                        "timeout_ms": {
                            "type": "integer",
                            "description": "Timeout in milliseconds (not yet enforced)"
                        }
                    },
                    "required": ["command"],
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
            let output = Command::new("bash")
                .arg("-c")
                .arg(&args.command)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .map_err(|e| ToolError(format!("failed to run command: {e}")))?;

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code().unwrap_or(-1);

            Ok(json!({
                "stdout": stdout,
                "stderr": stderr,
                "exit_code": exit_code
            }))
        })
        .await
    }
}
