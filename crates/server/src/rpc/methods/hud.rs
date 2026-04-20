// ---------------------------------------------------------------------------
// rpc::methods::hud — Heads Up Display RPC method definitions
// ---------------------------------------------------------------------------

use super::RpcMethod;

// ===========================================================================
// hud.currentGitBranch.get
// ===========================================================================

#[derive(serde::Deserialize)]
pub struct CurrentGitBranchGetParams {}

#[derive(serde::Serialize)]
pub struct CurrentGitBranchGetResult {
    pub branch: String,
}

pub struct CurrentGitBranchGet;

impl RpcMethod for CurrentGitBranchGet {
    const NAME: &'static str = "hud.currentGitBranch.get";
    type Params = CurrentGitBranchGetParams;
    type Result = CurrentGitBranchGetResult;
}

// ===========================================================================
// hud.diffCounts.get — summed +/- line counts from `git diff HEAD`
// ===========================================================================

#[derive(serde::Deserialize)]
pub struct DiffCountsGetParams {}

#[derive(serde::Serialize)]
pub struct DiffCountsGetResult {
    pub added: u64,
    pub removed: u64,
}

pub struct DiffCountsGet;

impl RpcMethod for DiffCountsGet {
    const NAME: &'static str = "hud.diffCounts.get";
    type Params = DiffCountsGetParams;
    type Result = DiffCountsGetResult;
}

// ===========================================================================
// hud.contextTokens.get — approximate token count for the active conversation
// ===========================================================================

#[derive(serde::Deserialize)]
pub struct ContextTokensGetParams {}

#[derive(serde::Serialize)]
pub struct ContextTokensGetResult {
    pub tokens: u64,
}

pub struct ContextTokensGet;

impl RpcMethod for ContextTokensGet {
    const NAME: &'static str = "hud.contextTokens.get";
    type Params = ContextTokensGetParams;
    type Result = ContextTokensGetResult;
}
