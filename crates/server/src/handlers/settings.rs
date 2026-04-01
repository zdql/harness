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
    })
}

/// Handle `settings.update`.
pub fn update(params: UpdateParams) -> Result<UpdateResult, String> {
    let mut s = storage::settings::read();

    if let Some(model) = params.model {
        s.model = Some(model);
    }
    if let Some(conversation) = params.conversation {
        s.conversation = Some(conversation);
    }

    storage::settings::write(&s).map_err(|e| format!("failed to write settings: {e}"))?;

    Ok(UpdateResult {
        model: s.model,
        conversation: s.conversation,
    })
}
