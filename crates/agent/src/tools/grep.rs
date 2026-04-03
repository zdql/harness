use regex::Regex;
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use std::fs;
use std::path::Path;

use crate::llm::{ChatCompletionTool, FunctionDefinition};

use super::{Tool, ToolError};

pub struct GrepTool;

#[derive(Deserialize)]
struct Args {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    glob: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    250
}

impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn definition(&self) -> ChatCompletionTool {
        ChatCompletionTool::Function {
            function: FunctionDefinition {
                name: "grep".to_string(),
                description: Some(
                    "Search file contents with a regex pattern. Returns matching lines with file paths and line numbers.".to_string(),
                ),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Regex pattern to search for"
                        },
                        "path": {
                            "type": "string",
                            "description": "File or directory to search in (default: current directory)"
                        },
                        "glob": {
                            "type": "string",
                            "description": "Glob filter for file names (e.g. \"*.rs\", \"*.ts\")"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of matches to return (default: 250)"
                        }
                    },
                    "required": ["pattern"],
                    "additionalProperties": false
                })),
                strict: Some(false),
            },
        }
    }

    fn call(&self, arguments: &str) -> Result<JsonValue, ToolError> {
        let args: Args =
            serde_json::from_str(arguments).map_err(|e| ToolError(format!("bad args: {e}")))?;

        let re = Regex::new(&args.pattern)
            .map_err(|e| ToolError(format!("invalid regex: {e}")))?;

        let search_path = args.path.as_deref().unwrap_or(".");
        let path = Path::new(search_path);

        let mut matches: Vec<JsonValue> = Vec::new();

        if path.is_file() {
            search_file(&re, path, args.limit, &mut matches);
        } else if path.is_dir() {
            // If a glob filter is provided, use it to find files; otherwise walk all files.
            let file_pattern = match &args.glob {
                Some(g) => format!("{}/{}",
                    search_path.trim_end_matches('/'),
                    g
                ),
                None => format!("{}/**/*", search_path.trim_end_matches('/')),
            };

            let entries: Vec<_> = glob::glob(&file_pattern)
                .map_err(|e| ToolError(format!("invalid glob: {e}")))?
                .filter_map(|e| e.ok())
                .filter(|p| p.is_file())
                .collect();

            for file_path in entries {
                if matches.len() >= args.limit {
                    break;
                }
                search_file(&re, &file_path, args.limit - matches.len(), &mut matches);
            }
        } else {
            return Err(ToolError(format!("path not found: {search_path}")));
        }

        let total = matches.len();
        Ok(json!({
            "matches": matches,
            "count": total
        }))
    }
}

fn search_file(re: &Regex, path: &Path, limit: usize, matches: &mut Vec<JsonValue>) {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return, // skip binary / unreadable files
    };

    for (i, line) in content.lines().enumerate() {
        if matches.len() >= limit {
            break;
        }
        if re.is_match(line) {
            matches.push(json!({
                "file": path.display().to_string(),
                "line": i + 1,
                "content": line
            }));
        }
    }
}
