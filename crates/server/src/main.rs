// ---------------------------------------------------------------------------
// harness-server — JSON-RPC 2.0 stdio server
//
// Protocol:
//   - One JSON object per line on stdin  (frontend → server)
//   - One JSON object per line on stdout (server → frontend)
//   - stderr is reserved for debug logging (never parsed by frontend)
//
// The server is async (tokio) because the agent loop makes HTTP calls to
// OpenRouter and streams AgentEvents back to the frontend as JSON-RPC
// notifications while a `conversation.send` is in-flight. State (ChatClient,
// ToolRegistry) is initialized once at startup and shared across requests.
// ---------------------------------------------------------------------------

mod handlers;
mod rpc;

use std::sync::Arc;

use agent::llm::{ChatBackend, ChatClient};
use agent::tools::{AdditionTool, BashTool, EditTool, GlobTool, GrepTool, ReadTool, ToolRegistry, WriteTool};
use rpc::methods::RpcMethod;
use rpc::{INTERNAL_ERROR, INVALID_PARAMS, METHOD_NOT_FOUND, Notification, Request, Response};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

use crate::rpc::methods::hud;

// ---------------------------------------------------------------------------
// Shared server state — initialized once, passed to async handlers
// ---------------------------------------------------------------------------

pub struct ServerState {
    pub chat_client: Arc<dyn ChatBackend>,
    pub tools: Arc<ToolRegistry>,
}

/// Serialized writer for stdout — notifications and responses share this so
/// we never interleave two JSON messages on a single line.
type SharedStdout = Arc<Mutex<tokio::io::Stdout>>;

#[tokio::main]
async fn main() {
    // Load API key from environment (supports .env via manual read)
    load_dotenv();
    let api_key = std::env::var("OPENROUTER_API_KEY").unwrap_or_else(|_| {
        eprintln!("warning: OPENROUTER_API_KEY not set — conversation.send will fail");
        String::new()
    });

    let default_model = std::env::var("HARNESS_MODEL").ok();
    if let Some(ref m) = default_model {
        eprintln!("info: default model = {m}");
    }

    let state = Arc::new(ServerState {
        chat_client: Arc::new(ChatClient::new(&api_key).with_title("harness")),
        tools: Arc::new(
            ToolRegistry::new()
                .register(AdditionTool)
                .register(BashTool)
                .register(ReadTool)
                .register(WriteTool)
                .register(GlobTool)
                .register(EditTool)
                .register(GrepTool),
        ),
    });

    let stdout: SharedStdout = Arc::new(Mutex::new(tokio::io::stdout()));
    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }

        let state = Arc::clone(&state);
        let stdout = Arc::clone(&stdout);
        tokio::spawn(async move {
            let response = match serde_json::from_str::<Request>(&line) {
                Ok(req) => dispatch(req, &state, &stdout).await,
                Err(e) => Response::error(
                    rpc::RequestId::Number(0),
                    rpc::PARSE_ERROR,
                    format!("parse error: {e}"),
                ),
            };

            write_json(&stdout, &response).await;
        });
    }
}

// ---------------------------------------------------------------------------
// I/O helpers
// ---------------------------------------------------------------------------

async fn write_json<T: serde::Serialize>(stdout: &SharedStdout, value: &T) {
    let json = serde_json::to_string(value).expect("serialization cannot fail");
    let mut out = stdout.lock().await;
    let _ = out.write_all(json.as_bytes()).await;
    let _ = out.write_all(b"\n").await;
    let _ = out.flush().await;
}

fn notification(method: impl Into<String>, params: Value) -> Notification {
    Notification {
        jsonrpc: "2.0".into(),
        method: method.into(),
        params,
    }
}

// ---------------------------------------------------------------------------
// Dispatcher
// ---------------------------------------------------------------------------

async fn dispatch(req: Request, state: &ServerState, stdout: &SharedStdout) -> Response {
    use rpc::methods::{conversation, settings};

    match req.method.as_str() {
        // -- Settings -------------------------------------------------------
        settings::Get::NAME => handle::<settings::Get, _>(req, handlers::settings::get),
        settings::Update::NAME => handle::<settings::Update, _>(req, handlers::settings::update),

        // -- Conversation ---------------------------------------------------
        conversation::Create::NAME => {
            handle::<conversation::Create, _>(req, handlers::conversation::create)
        }
        conversation::List::NAME => {
            handle::<conversation::List, _>(req, handlers::conversation::list)
        }
        conversation::Switch::NAME => {
            handle::<conversation::Switch, _>(req, handlers::conversation::switch)
        }
        conversation::Get::NAME => handle::<conversation::Get, _>(req, handlers::conversation::get),
        conversation::SetModel::NAME => {
            handle::<conversation::SetModel, _>(req, handlers::conversation::set_model)
        }
        conversation::Send::NAME => handle_send(req, state, stdout).await,

        // -- HUD ------------------------------------------------------------
        hud::CurrentGitBranchGet::NAME => {
            handle::<hud::CurrentGitBranchGet, _>(req, handlers::hud::current_git_branch_get)
        }
        hud::DiffCountsGet::NAME => {
            handle::<hud::DiffCountsGet, _>(req, handlers::hud::diff_counts_get)
        }
        hud::ContextTokensGet::NAME => {
            handle::<hud::ContextTokensGet, _>(req, handlers::hud::context_tokens_get)
        }
        hud::CurrentModelGet::NAME => {
            handle::<hud::CurrentModelGet, _>(req, handlers::hud::current_model_get)
        }

        // -- Unknown --------------------------------------------------------
        _ => Response::error(
            req.id,
            METHOD_NOT_FOUND,
            format!("unknown method: {}", req.method),
        ),
    }
}

/// Handle `conversation.send` with event streaming. Events emitted by the
/// agent loop (and any background continuation) are forwarded to stdout as
/// JSON-RPC notifications. The response is returned as soon as the model
/// produces its first reply — even if subagents are still running.
async fn handle_send(req: Request, state: &ServerState, stdout: &SharedStdout) -> Response {
    use rpc::methods::conversation::SendParams;

    let id = req.id.clone();
    let params: SendParams = match serde_json::from_value(req.params) {
        Ok(p) => p,
        Err(e) => {
            return Response::error(id, INVALID_PARAMS, format!("invalid params: {e}"));
        }
    };

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<agent::agent::AgentEvent>();

    // Forwarder: drain events → stdout as notifications. Runs until every
    // clone of `tx` is dropped — including any clone held by a background
    // continuation task spawned by `send()`.
    let stdout_for_fwd = Arc::clone(stdout);
    tokio::spawn(async move {
        let mut rx = rx;
        while let Some(event) = rx.recv().await {
            let params = serde_json::to_value(&event).unwrap_or_else(|_| serde_json::json!({}));
            let notif = notification("agent.event", params);
            write_json(&stdout_for_fwd, &notif).await;
        }
    });

    let result = handlers::conversation::send(params, state, Some(tx)).await;

    match result {
        Ok(result) => {
            let value = serde_json::to_value(result).expect("result serialization cannot fail");
            Response::success(id, value)
        }
        Err(msg) => Response::error(id, INTERNAL_ERROR, msg),
    }
}

// ---------------------------------------------------------------------------
// Generic handler wrapper (sync handlers)
// ---------------------------------------------------------------------------

fn handle<M: RpcMethod, F>(req: Request, handler: F) -> Response
where
    F: FnOnce(M::Params) -> Result<M::Result, String>,
{
    let params: M::Params = match serde_json::from_value(req.params) {
        Ok(p) => p,
        Err(e) => {
            return Response::error(req.id, INVALID_PARAMS, format!("invalid params: {e}"));
        }
    };

    match handler(params) {
        Ok(result) => {
            let value = serde_json::to_value(result).expect("result serialization cannot fail");
            Response::success(req.id, value)
        }
        Err(msg) => Response::error(req.id, INTERNAL_ERROR, msg),
    }
}

// ---------------------------------------------------------------------------
// Minimal .env loader — reads .env from cwd or parent dirs, no extra dep
// ---------------------------------------------------------------------------

fn load_dotenv() {
    let mut dir = std::env::current_dir().ok();
    while let Some(d) = dir {
        let env_path = d.join(".env");
        if env_path.exists() {
            if let Ok(contents) = std::fs::read_to_string(&env_path) {
                for line in contents.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if let Some((key, val)) = line.split_once('=') {
                        let key = key.trim();
                        let val = val.trim().trim_matches('"').trim_matches('\'');
                        if std::env::var(key).is_err() {
                            // SAFETY: called once at startup before any threads
                            unsafe {
                                std::env::set_var(key, val);
                            }
                        }
                    }
                }
            }
            break;
        }
        dir = d.parent().map(|p| p.to_path_buf());
    }
}
