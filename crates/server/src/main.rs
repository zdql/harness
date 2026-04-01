// ---------------------------------------------------------------------------
// harness-server — JSON-RPC 2.0 stdio server
//
// Protocol:
//   - One JSON object per line on stdin  (frontend → server)
//   - One JSON object per line on stdout (server → frontend)
//   - stderr is reserved for debug logging (never parsed by frontend)
//
// The server is async (tokio) because the agent loop makes HTTP calls to
// OpenRouter. State (ChatClient, ToolRegistry) is initialized once at
// startup and shared across all requests.
// ---------------------------------------------------------------------------

mod handlers;
mod rpc;

use std::io::{self, BufRead, Write};
use std::sync::Arc;

use agent::llm::ChatClient;
use agent::tools::{AdditionTool, ToolRegistry};
use rpc::methods::RpcMethod;
use rpc::{Request, Response, INTERNAL_ERROR, INVALID_PARAMS, METHOD_NOT_FOUND};

// ---------------------------------------------------------------------------
// Shared server state — initialized once, passed to async handlers
// ---------------------------------------------------------------------------

pub struct ServerState {
    pub chat_client: ChatClient,
    pub tools: ToolRegistry,
}

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
        chat_client: ChatClient::new(&api_key).with_title("harness"),
        tools: ToolRegistry::new().register(AdditionTool),
    });

    let stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();

    for line in stdin.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<Request>(&line) {
            Ok(req) => dispatch(req, &state).await,
            Err(e) => Response::error(
                rpc::RequestId::Number(0),
                rpc::PARSE_ERROR,
                format!("parse error: {e}"),
            ),
        };

        let json = serde_json::to_string(&response).expect("response serialization cannot fail");
        let _ = writeln!(stdout, "{json}");
        let _ = stdout.flush();
    }
}

// ---------------------------------------------------------------------------
// Dispatcher
// ---------------------------------------------------------------------------

async fn dispatch(req: Request, state: &Arc<ServerState>) -> Response {
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
        conversation::Send::NAME => {
            let state = Arc::clone(state);
            let id = req.id.clone();
            let params: conversation::SendParams = match serde_json::from_value(req.params) {
                Ok(p) => p,
                Err(e) => {
                    return Response::error(id, INVALID_PARAMS, format!("invalid params: {e}"));
                }
            };
            match handlers::conversation::send(params, &state).await {
                Ok(result) => {
                    let value = serde_json::to_value(result)
                        .expect("result serialization cannot fail");
                    Response::success(id, value)
                }
                Err(msg) => Response::error(id, INTERNAL_ERROR, msg),
            }
        }

        // -- Unknown --------------------------------------------------------
        _ => Response::error(
            req.id,
            METHOD_NOT_FOUND,
            format!("unknown method: {}", req.method),
        ),
    }
}

// ---------------------------------------------------------------------------
// Generic handler wrapper
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
                            unsafe { std::env::set_var(key, val); }
                        }
                    }
                }
            }
            break;
        }
        dir = d.parent().map(|p| p.to_path_buf());
    }
}
