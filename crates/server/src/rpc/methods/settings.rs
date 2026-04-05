// ---------------------------------------------------------------------------
// rpc::methods::settings — Settings RPC method definitions
// ---------------------------------------------------------------------------

use super::RpcMethod;

// ===========================================================================
// settings.get
// ===========================================================================

pub struct Get;

impl RpcMethod for Get {
    const NAME: &'static str = "settings.get";
    type Params = GetParams;
    type Result = GetResult;
}

#[derive(serde::Deserialize)]
pub struct GetParams {}

#[derive(serde::Serialize)]
pub struct GetResult {
    pub model: Option<String>,
    pub conversation: Option<String>,
    pub reasoning_effort: Option<String>,
    pub reasoning_summary: Option<String>,
}

// ===========================================================================
// settings.update
// ===========================================================================

pub struct Update;

impl RpcMethod for Update {
    const NAME: &'static str = "settings.update";
    type Params = UpdateParams;
    type Result = UpdateResult;
}

#[derive(serde::Deserialize)]
pub struct UpdateParams {
    pub model: Option<String>,
    pub conversation: Option<String>,
    pub reasoning_effort: Option<String>,
    pub reasoning_summary: Option<String>,
}

#[derive(serde::Serialize)]
pub struct UpdateResult {
    pub model: Option<String>,
    pub conversation: Option<String>,
    pub reasoning_effort: Option<String>,
    pub reasoning_summary: Option<String>,
}
