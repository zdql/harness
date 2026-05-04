use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};
use std::fs;
use std::path::Path;

use crate::llm::{ChatCompletionTool, FunctionDefinition};

use super::{Tool, ToolError, run_blocking};

pub struct EditTool;

#[derive(Deserialize)]
struct Args {
    file_path: String,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn definition(&self) -> ChatCompletionTool {
        ChatCompletionTool::Function {
            function: FunctionDefinition {
                name: "edit".to_string(),
                description: Some(
                    "Edit a file by replacing an exact string. \
                     This is the preferred tool for modifying files — use it instead of write \
                     whenever you are changing an existing file (as opposed to creating a new one \
                     from scratch). The old_string must match literally (not a regex). \
                     If old_string is not unique in the file and replace_all is false, the tool \
                     errors — you must provide a larger string with more surrounding context to \
                     make it unique. Do NOT use this tool to create new files; use write for that."
                        .to_string(),
                ),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Absolute path to the file to edit"
                        },
                        "old_string": {
                            "type": "string",
                            "description": "Exact literal text to replace — not a regex. Must be unique in the file unless replace_all is true."
                        },
                        "new_string": {
                            "type": "string",
                            "description": "Replacement text. Must differ from old_string."
                        },
                        "replace_all": {
                            "type": "boolean",
                            "default": false,
                            "description": "If true, replace every occurrence of old_string. If false (default), old_string must appear exactly once."
                        }
                    },
                    "required": ["file_path", "old_string", "new_string"],
                    "additionalProperties": false
                })),
                strict: Some(true),
            },
        }
    }

    async fn call(&self, arguments: &str) -> Result<JsonValue, ToolError> {
        let args: Args =
            serde_json::from_str(arguments).map_err(|e| ToolError(format!("bad args: {e}")))?;

        if args.old_string.is_empty() {
            return Err(ToolError("old_string must not be empty".to_string()));
        }

        if args.old_string == args.new_string {
            return Err(ToolError(
                "old_string and new_string must be different".to_string(),
            ));
        }

        run_blocking(move || {
            let path = Path::new(&args.file_path);
            let original =
                fs::read_to_string(path)
                    .map_err(|e| ToolError(format!("failed to read {}: {e}", args.file_path)))?;

            let count = original.matches(&args.old_string).count();

            if count == 0 {
                return Err(ToolError(format!(
                    "old_string not found in {}",
                    args.file_path
                )));
            }

            if !args.replace_all && count > 1 {
                return Err(ToolError(format!(
                    "old_string is not unique in {} — found {} matches. \
                     Provide a larger string with more surrounding context to make it unique, \
                     or set replace_all to true.",
                    args.file_path, count
                )));
            }

            let replaced = if args.replace_all {
                original.replace(&args.old_string, &args.new_string)
            } else {
                original.replacen(&args.old_string, &args.new_string, 1)
            };

            fs::write(path, &replaced)
                .map_err(|e| ToolError(format!("failed to write {}: {e}", args.file_path)))?;

            let replacements = if args.replace_all { count } else { 1 };

            Ok(json!({
                "path": args.file_path,
                "replacements": replacements
            }))
        })
        .await
    }
}
