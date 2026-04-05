// ---------------------------------------------------------------------------
// handlers::settings — Settings read/write handlers
// ---------------------------------------------------------------------------

use crate::rpc::methods::settings::{GetParams, GetResult, UpdateParams, UpdateResult};

/// Handle `settings.get`.
pub fn get(_params: GetParams) -> Result<GetResult, String> {
    let s = storage::settings::read();
    Ok(GetResult {
        model: s.model,
        conversation: s.conversation,
        reasoning_effort: s.reasoning_effort,
        reasoning_summary: s.reasoning_summary,
    })
}

/// Handle `settings.update`. Any field set in the params is written; unset
/// params leave the existing value untouched. To clear a field, pass the
/// empty string — it's stored as `None`.
pub fn update(params: UpdateParams) -> Result<UpdateResult, String> {
    let mut s = storage::settings::read();

    if let Some(model) = params.model {
        s.model = if model.is_empty() { None } else { Some(model) };
    }
    if let Some(conversation) = params.conversation {
        s.conversation = if conversation.is_empty() {
            None
        } else {
            Some(conversation)
        };
    }
    if let Some(effort) = params.reasoning_effort {
        s.reasoning_effort = if effort.is_empty() { None } else { Some(effort) };
    }
    if let Some(summary) = params.reasoning_summary {
        s.reasoning_summary = if summary.is_empty() {
            None
        } else {
            Some(summary)
        };
    }

    storage::settings::write(&s).map_err(|e| format!("failed to write settings: {e}"))?;

    Ok(UpdateResult {
        model: s.model,
        conversation: s.conversation,
        reasoning_effort: s.reasoning_effort,
        reasoning_summary: s.reasoning_summary,
    })
}
