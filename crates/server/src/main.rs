// ---------------------------------------------------------------------------
// harness-server — JSON-RPC 2.0 stdio server
//
// Protocol:
//   - One JSON object per line on stdin  (frontend → server)
//   - One JSON object per line on stdout (server → frontend)
//   - stderr is reserved for debug logging (never parsed by frontend)
//
// To add a new method:
//   1. Define types in  rpc/methods/<domain>.rs
//   2. Write handler in  handlers/<domain>.rs
//   3. Add a match arm in dispatch() below
// ---------------------------------------------------------------------------

mod handlers;
mod rpc;

use std::io::{self, BufRead, Write};

use rpc::methods::RpcMethod;
use rpc::{Request, Response, INTERNAL_ERROR, INVALID_PARAMS, METHOD_NOT_FOUND};

fn main() {
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
            Ok(req) => dispatch(req),
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
// Dispatcher — routes method names to typed handlers
// ---------------------------------------------------------------------------

fn dispatch(req: Request) -> Response {
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
