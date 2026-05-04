use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};

use crate::llm::{ChatCompletionTool, FunctionDefinition};

use super::{Tool, ToolError, run_blocking};

pub struct GlobTool;

#[derive(Deserialize)]
struct Args {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn definition(&self) -> ChatCompletionTool {
        ChatCompletionTool::Function {
            function: FunctionDefinition {
                name: "glob".to_string(),
                description: Some(
                    "Find files matching a glob pattern. Returns matching file paths.".to_string(),
                ),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Glob pattern to match (e.g. \"**/*.rs\", \"src/**/*.ts\")"
                        },
                        "path": {
                            "type": "string",
                            "description": "Base directory to search in (default: current working directory)"
                        }
                    },
                    "required": ["pattern"],
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
            let full_pattern = match &args.path {
                Some(base) => {
                    let base = base.trim_end_matches('/');
                    format!("{}/{}", base, args.pattern)
                }
                None => args.pattern.clone(),
            };

            let paths: Vec<String> = glob::glob(&full_pattern)
                .map_err(|e| ToolError(format!("invalid glob pattern: {e}")))?
                .filter_map(|entry| entry.ok())
                .filter(|p| p.is_file())
                .map(|p| p.display().to_string())
                .collect();

            Ok(json!({
                "files": paths,
                "count": paths.len()
            }))
        })
        .await
    }
}
