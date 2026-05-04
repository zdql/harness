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

// ===========================================================================
// hud.currentModel.get — model id the active conversation will use on its
// next send. Falls back to the global settings model, then the built-in
// default if neither is set.
// ===========================================================================

#[derive(serde::Deserialize)]
pub struct CurrentModelGetParams {}

#[derive(serde::Serialize)]
pub struct CurrentModelGetResult {
    pub model: String,
}

pub struct CurrentModelGet;

impl RpcMethod for CurrentModelGet {
    const NAME: &'static str = "hud.currentModel.get";
    type Params = CurrentModelGetParams;
    type Result = CurrentModelGetResult;
}
