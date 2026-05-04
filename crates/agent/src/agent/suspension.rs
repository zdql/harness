// Helpers for the subagent-suspension half of the agent loop:
//   - drain_inbox_into_conv: top-of-loop, fold finished subagents into the
//     conversation as user messages.
//   - finalize_or_suspend: end-of-loop, decide between Done and Suspended
//     based on whether subagents are still in flight.
//   - inject_subagent_result: append one finished subagent result to the
//     conversation. Public so the server's continuation loop can call it
//     between `inbox.recv().await` and `agent::resume()`.

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::conversation::{self, Conversation};
use crate::subagents::{SubagentInbox, SubagentResult};
use storage::ConversationStore;

use super::types::{RunOutcome, RunResult, ToolCallInfo};

pub(crate) fn drain_inbox_into_conv<S: ConversationStore>(
    store: &S,
    conv: &mut Conversation,
    inbox: &mut Option<&mut SubagentInbox>,
) -> Result<(), S::Error> {
    if let Some(ib) = inbox.as_mut() {
        for result in ib.try_drain() {
            inject_subagent_result(store, conv, &ib.pending, result)?;
        }
    }
    Ok(())
}

pub(crate) fn finalize_or_suspend(
    reply: String,
    tool_calls: Vec<ToolCallInfo>,
    pending_subagents: usize,
) -> RunOutcome {
    let result = RunResult { reply, tool_calls };
    if pending_subagents > 0 {
        RunOutcome::Suspended {
            result,
            pending_subagents,
        }
    } else {
        RunOutcome::Done(result)
    }
}

pub fn inject_subagent_result<S: ConversationStore>(
    store: &S,
    conv: &mut Conversation,
    pending: &AtomicUsize,
    result: SubagentResult,
) -> Result<(), S::Error> {
    let body = format!(
        "SUBAGENT {} {}: {}",
        result.id,
        result.status.as_str(),
        result.output,
    );
    conv.push_user(&body);
    conversation::save(store, conv)?;
    pending.fetch_sub(1, Ordering::AcqRel);
    Ok(())
}
