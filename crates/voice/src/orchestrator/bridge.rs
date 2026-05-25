use crate::orchestrator::jobs::{OrchestratorJob, OrchestratorJobEvent, OrchestratorJobManager};
use crate::orchestrator::protocol::preview;
use crate::types::{DelegateToOrchestratorArgs, VoiceUpdate};
use serde_json::json;
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

// The boundary between Realtime tool calls and background orchestrator jobs.
// This module is the only place that kicks jobs off or turns completions into
// Realtime conversation items.
pub(crate) struct OrchestratorBridge {
    handled_function_calls: HashSet<String>,
}

impl OrchestratorBridge {
    pub(crate) fn new() -> Self {
        Self {
            handled_function_calls: HashSet::new(),
        }
    }

    pub(crate) fn handle_realtime_event(
        &mut self,
        value: &serde_json::Value,
        jobs: &OrchestratorJobManager,
    ) -> Result<Vec<serde_json::Value>, String> {
        match value.get("type").and_then(|v| v.as_str()) {
            Some("response.function_call_arguments.done") => {
                self.handle_function_call_done(value, jobs)
            }
            Some("response.output_item.done") => {
                let Some(call) = function_call_from_output_item(value) else {
                    return Ok(Vec::new());
                };
                self.handle_function_call_done(&call, jobs)
            }
            _ => Ok(Vec::new()),
        }
    }

    pub(crate) fn realtime_events_for_job_event(
        &self,
        event: OrchestratorJobEvent,
    ) -> Vec<serde_json::Value> {
        match event {
            OrchestratorJobEvent::Completed {
                job_id,
                slug,
                result,
            } => orchestrator_result_events(
                &job_id,
                &slug,
                result.as_ref().map(String::as_str).map_err(String::as_str),
            ),
        }
    }

    fn handle_function_call_done(
        &mut self,
        value: &serde_json::Value,
        jobs: &OrchestratorJobManager,
    ) -> Result<Vec<serde_json::Value>, String> {
        let name = value.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name != "delegate_to_orchestrator" {
            return Ok(Vec::new());
        }
        let call_id = value
            .get("call_id")
            .and_then(|v| v.as_str())
            .ok_or("function call missing call_id")?;
        let args_raw = value
            .get("arguments")
            .and_then(|v| v.as_str())
            .unwrap_or("{}");
        if args_raw.trim().is_empty() {
            eprintln!("delegate_to_orchestrator ignored empty arguments call_id={call_id}");
            return Ok(Vec::new());
        }
        let args: DelegateToOrchestratorArgs = match serde_json::from_str(args_raw) {
            Ok(args) => args,
            Err(e) => {
                eprintln!(
                    "delegate_to_orchestrator ignored invalid arguments call_id={} error={} raw={}",
                    call_id, e, args_raw
                );
                return Ok(Vec::new());
            }
        };
        let slug = match sanitize_slug(&args.slug) {
            Some(slug) => slug,
            None => {
                eprintln!(
                    "delegate_to_orchestrator ignored missing slug call_id={} intent_preview={}",
                    call_id,
                    preview(&args.user_intent)
                );
                return orchestrator_delegate_error_events(
                    call_id,
                    "delegate_to_orchestrator requires a non-empty snake_case slug. Choose a slug for the background conversation and call the tool again.",
                );
            }
        };

        if !self.handled_function_calls.insert(call_id.to_string()) {
            eprintln!(
                "orchestrator bridge duplicate delegate ignored call_id={} slug={}",
                call_id, slug
            );
            return Ok(Vec::new());
        }

        let job_id = gen_job_id();
        eprintln!(
            "orchestrator bridge delegate call_id={} job={} slug={} urgency={} intent_preview={}",
            call_id,
            job_id,
            slug,
            args.urgency,
            preview(&args.user_intent)
        );
        jobs.enqueue(OrchestratorJob {
            id: job_id,
            slug: slug.clone(),
            args: DelegateToOrchestratorArgs {
                slug: slug.clone(),
                ..args
            },
        })?;

        let update = VoiceUpdate {
            message: format!("Queued {slug}. You will be delivered results once it's done."),
            should_interrupt: false,
            confidence: 1.0,
            done: false,
        };

        let output = serde_json::to_string(&update)
            .map_err(|e| format!("failed to serialize orchestrator output: {e}"))?;
        Ok(vec![
            json!({
                "type": "conversation.item.create",
                "item": {
                    "type": "function_call_output",
                    "call_id": call_id,
                    "output": output,
                }
            }),
            json!({"type": "response.create"}),
        ])
    }
}

fn orchestrator_delegate_error_events(
    call_id: &str,
    message: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let update = VoiceUpdate {
        message: message.to_string(),
        should_interrupt: false,
        confidence: 1.0,
        done: true,
    };
    let output = serde_json::to_string(&update)
        .map_err(|e| format!("failed to serialize orchestrator error output: {e}"))?;
    Ok(vec![
        json!({
            "type": "conversation.item.create",
            "item": {
                "type": "function_call_output",
                "call_id": call_id,
                "output": output,
            }
        }),
        json!({"type": "response.create"}),
    ])
}

fn function_call_from_output_item(value: &serde_json::Value) -> Option<serde_json::Value> {
    let item = value.get("item")?;
    if item.get("type").and_then(|v| v.as_str()) != Some("function_call") {
        return None;
    }
    let arguments = item.get("arguments").cloned().unwrap_or_default();
    if arguments.as_str().map(str::trim).unwrap_or("").is_empty() {
        eprintln!("realtime function_call output_item.done ignored empty arguments");
        return None;
    }
    Some(json!({
        "type": "response.function_call_arguments.done",
        "name": item.get("name").cloned().unwrap_or_default(),
        "call_id": item.get("call_id").cloned().unwrap_or_default(),
        "arguments": arguments,
    }))
}

fn orchestrator_result_events(
    job_id: &str,
    slug: &str,
    result: Result<&str, &str>,
) -> Vec<serde_json::Value> {
    let call_id = synthetic_result_call_id(job_id);
    let output = match result {
        Ok(reply) => json!({
            "ok": true,
            "job_id": job_id,
            "slug": slug,
            "instruction": "THIS IS THE RESULT OF YOUR TOOL CALL - YOU SHOULD INFORM THE USER ABOUT THIS.",
            "result": reply,
        }),
        Err(error) => json!({
            "ok": false,
            "job_id": job_id,
            "slug": slug,
            "instruction": "THIS IS THE RESULT OF YOUR TOOL CALL - YOU SHOULD INFORM THE USER THAT THE BACKGROUND WORK FAILED.",
            "error": error,
        }),
    };

    vec![
        json!({
            "type": "conversation.item.create",
            "event_id": format!("event_{call_id}_call"),
            "item": {
                "type": "function_call",
                "call_id": call_id,
                "name": "background_orchestrator_result",
                "arguments": serde_json::to_string(&json!({
                    "job_id": job_id,
                    "slug": slug
                }))
                    .unwrap_or_else(|_| "{}".to_string()),
            }
        }),
        json!({
            "type": "conversation.item.create",
            "event_id": format!("event_{call_id}_output"),
            "item": {
                "type": "function_call_output",
                "call_id": call_id,
                "output": serde_json::to_string(&output).unwrap_or_else(|_| {
                    "{\"ok\":false,\"error\":\"failed to serialize orchestrator result\"}"
                        .to_string()
                }),
            }
        }),
        json!({ "type": "response.create" }),
    ]
}

fn synthetic_result_call_id(job_id: &str) -> String {
    let mut id = String::from("call_");
    for ch in job_id.chars() {
        if ch.is_ascii_alphanumeric() {
            id.push(ch);
        } else {
            id.push('_');
        }
    }
    id
}

fn gen_job_id() -> String {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("voice-job-{:x}-{:x}", d.as_secs(), d.subsec_nanos())
}

fn sanitize_slug(value: &str) -> Option<String> {
    let mut slug = String::new();
    let mut last_was_separator = false;
    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator && !slug.is_empty() {
            slug.push('_');
            last_was_separator = true;
        }
    }
    while slug.ends_with('_') {
        slug.pop();
    }
    (!slug.is_empty()).then_some(slug)
}
