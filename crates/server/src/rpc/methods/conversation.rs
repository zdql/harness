// ---------------------------------------------------------------------------
// rpc::methods::conversation — Conversation RPC method definitions
// ---------------------------------------------------------------------------

use super::RpcMethod;

// ===========================================================================
// conversation.create
// ===========================================================================

pub struct Create;

impl RpcMethod for Create {
    const NAME: &'static str = "conversation.create";
    type Params = CreateParams;
    type Result = CreateResult;
}

#[derive(serde::Deserialize)]
pub struct CreateParams {}

#[derive(serde::Serialize)]
pub struct CreateResult {
    pub id: String,
}

// ===========================================================================
// conversation.list
// ===========================================================================

pub struct List;

impl RpcMethod for List {
    const NAME: &'static str = "conversation.list";
    type Params = ListParams;
    type Result = ListResult;
}

#[derive(serde::Deserialize)]
pub struct ListParams {}

#[derive(serde::Serialize)]
pub struct ListResult {
    pub conversations: Vec<ConversationSummary>,
}

#[derive(serde::Serialize)]
pub struct ConversationSummary {
    pub id: String,
}

// ===========================================================================
// conversation.switch
// ===========================================================================

pub struct Switch;

impl RpcMethod for Switch {
    const NAME: &'static str = "conversation.switch";
    type Params = SwitchParams;
    type Result = SwitchResult;
}

#[derive(serde::Deserialize)]
pub struct SwitchParams {
    pub id: String,
}

#[derive(serde::Serialize)]
pub struct SwitchResult {
    pub id: String,
}

// ===========================================================================
// conversation.send
// ===========================================================================

pub struct Send;

impl RpcMethod for Send {
    const NAME: &'static str = "conversation.send";
    type Params = SendParams;
    type Result = SendResult;
}

#[derive(serde::Deserialize)]
pub struct SendParams {
    /// The conversation to send to (must already exist).
    pub id: String,
    /// The user's message text.
    pub message: String,
}

#[derive(serde::Serialize)]
pub struct SendResult {
    /// The assistant's final text reply.
    pub reply: String,
}
