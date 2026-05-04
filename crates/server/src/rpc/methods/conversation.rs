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
    pub title: Option<String>,
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
// conversation.get
// ===========================================================================

pub struct Get;

impl RpcMethod for Get {
    const NAME: &'static str = "conversation.get";
    type Params = GetParams;
    type Result = GetResult;
}

#[derive(serde::Deserialize)]
pub struct GetParams {
    pub id: String,
}

#[derive(serde::Serialize)]
pub struct GetResult {
    pub id: String,
    pub title: Option<String>,
    pub messages: Vec<MessageEntry>,
}

/// A simplified message for the frontend.
#[derive(serde::Serialize)]
pub struct MessageEntry {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_args: Option<String>,
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
    /// Tool calls that were executed during the agent loop.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCallInfo>,
}

#[derive(serde::Serialize)]
pub struct ToolCallInfo {
    pub name: String,
    pub arguments: String,
    pub result: String,
}

// ===========================================================================
// conversation.setModel
// ===========================================================================

pub struct SetModel;

impl RpcMethod for SetModel {
    const NAME: &'static str = "conversation.setModel";
    type Params = SetModelParams;
    type Result = SetModelResult;
}

#[derive(serde::Deserialize)]
pub struct SetModelParams {
    /// Conversation to update.
    pub id: String,
    /// New model id (e.g. `"deepseek/deepseek-chat-v3.1"`). Empty string
    /// clears the per-conversation override and falls back to the global
    /// settings default on the next send.
    pub model: String,
}

#[derive(serde::Serialize)]
pub struct SetModelResult {
    pub id: String,
    pub model: Option<String>,
}
