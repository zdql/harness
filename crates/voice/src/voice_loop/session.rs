use serde_json::json;

pub(crate) fn session_update_json() -> serde_json::Value {
    session_update_json_for_model("gpt-realtime-2")
}

pub(super) fn session_update_json_for_model(model: &str) -> serde_json::Value {
    json!({
        "type": "session.update",
        "session": {
            "type": "realtime",
            "model": model,
            "instructions": "You are a realtime voice frontend. Keep the spoken conversation moving. When the user asks for work that benefits from deeper reasoning, tools, files, research, or multi-step execution, call delegate_to_orchestrator. Always include a stable snake_case slug that names what the background agent will do, such as refactor_docs. Reuse the same slug to continue that background conversation; use a new slug for unrelated work. Do not pretend the background work is done until the orchestrator returns an update.",
            "output_modalities": ["audio"],
            "audio": {
                "input": {
                    "format": {
                        "type": "audio/pcm",
                        "rate": 24000
                    },
                    "turn_detection": {
                        "type": "semantic_vad"
                    }
                },
                "output": {
                    "format": {
                        "type": "audio/pcm",
                        "rate": 24000
                    },
                    "voice": "marin"
                }
            },
            "tools": [{
                "type": "function",
                "name": "delegate_to_orchestrator",
                "description": "Delegate deeper work to a background orchestrator agent.",
                "parameters": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "slug": {
                            "type": "string",
                            "description": "Stable snake_case slug for this background task, e.g. refactor_docs. Reuse to continue the same agent conversation; use a new slug to start a separate agent."
                        },
                        "user_intent": {
                            "type": "string",
                            "description": "The user's request, rewritten as a clear task for the background orchestrator."
                        },
                        "recent_context": {
                            "type": "string",
                            "description": "Short recent transcript/context needed to understand the delegation."
                        },
                        "urgency": {
                            "type": "string",
                            "enum": ["now", "background"],
                            "description": "Use now for work the user is actively waiting on; background for longer-running side work."
                        },
                        "suggested_user_update": {
                            "type": "string",
                            "description": "Optional one-sentence phrase the voice model can say while the orchestrator starts."
                        }
                    },
                    "required": ["slug", "user_intent", "recent_context", "urgency"]
                }
            }]
        }
    })
}
