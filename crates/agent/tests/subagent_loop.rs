// ---------------------------------------------------------------------------
// Subagent flow tests — driven by `agent::testing::MockBackend`.
//
// Every test scripts LLM responses with predicates over the request shape
// (does it contain the user's task? a tool result? a SUBAGENT user message?)
// — never positional FIFO for parent/subagent boundaries — so subagent and
// parent timing can race freely without changing the outcome.
//
// All LLM responses are scripted via `MockBackend`. The same backend is shared
// by parent and subagents — predicate routing keeps their queues independent.
// ---------------------------------------------------------------------------

use std::sync::Arc;

use agent::agent::{self as agent_loop, RunOutcome, RunResult};
use agent::conversation::Conversation;
use agent::llm::{ChatBackend, ChatCompletionMessage, CreateChatCompletionRequest, UserContent};
use agent::subagents::{SubagentContext, SubagentInbox, SubagentRegistry};
use agent::testing::{MemStore, MockBackend, MockResponse, MockToolCall};
use agent::tools::ToolRegistry;
use tempfile::TempDir;
use tokio::sync::Notify;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_ctx(
    backend: Arc<dyn ChatBackend>,
    registry: Arc<SubagentRegistry>,
    subagent_root: std::path::PathBuf,
) -> SubagentContext {
    SubagentContext {
        chat_client: backend,
        base_tools: Arc::new(ToolRegistry::new()),
        parent_event_sink: None,
        depth: 0,
        parent_id: None,
        model: Some("mock".to_string()),
        reasoning: None,
        subagent_root,
        scratch_dir: None,
        registry,
    }
}

/// Drive the agent loop to `Done`, manually injecting subagent results when
/// it suspends — exactly what the server's continuation loop does.
async fn run_to_done(
    backend: &dyn ChatBackend,
    store: &MemStore,
    conv: &mut Conversation,
    tools: Arc<ToolRegistry>,
    inbox: &mut SubagentInbox,
    input: &str,
) -> RunResult {
    let mut outcome = agent_loop::run(
        backend,
        store,
        conv,
        Arc::clone(&tools),
        input,
        None,
        None,
        Some(inbox),
        None,
    )
    .await
    .expect("agent::run errored");

    loop {
        match outcome {
            RunOutcome::Done(r) => return r,
            RunOutcome::Suspended { .. } => {
                let result = inbox.recv().await.expect("inbox closed before completion");
                agent_loop::inject_subagent_result(store, conv, inbox.pending_counter(), result)
                    .expect("inject failed");

                outcome = agent_loop::resume(
                    backend,
                    store,
                    conv,
                    Arc::clone(&tools),
                    None,
                    None,
                    Some(inbox),
                    None,
                )
                .await
                .expect("resume errored");
            }
        }
    }
}

// --- request shape predicates ------------------------------------------------

fn user_strings(req: &CreateChatCompletionRequest) -> Vec<String> {
    req.messages
        .iter()
        .filter_map(|m| match m {
            ChatCompletionMessage::User(u) => match &u.content {
                UserContent::String(s) => Some(s.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn has_subagent_user_msg(req: &CreateChatCompletionRequest) -> bool {
    user_strings(req).iter().any(|s| s.starts_with("SUBAGENT "))
}

fn count_subagent_user_msgs(req: &CreateChatCompletionRequest) -> usize {
    user_strings(req)
        .iter()
        .filter(|s| s.starts_with("SUBAGENT "))
        .count()
}

fn has_tool_msg(req: &CreateChatCompletionRequest) -> bool {
    req.messages
        .iter()
        .any(|m| matches!(m, ChatCompletionMessage::Tool(_)))
}

fn count_subagent_messages_in_conv(conv: &Conversation) -> usize {
    conv.messages
        .iter()
        .filter(|m| match m {
            ChatCompletionMessage::User(u) => match &u.content {
                UserContent::String(s) => s.starts_with("SUBAGENT "),
                _ => false,
            },
            _ => false,
        })
        .count()
}

// ---------------------------------------------------------------------------
// 1. Happy path: spawn → suspend → inject → resume → done
// ---------------------------------------------------------------------------

#[tokio::test]
async fn happy_path_single_subagent() {
    let backend = Arc::new(MockBackend::new());
    let registry = Arc::new(SubagentRegistry::new());
    let tmp = TempDir::new().unwrap();

    // Subagent: text reply when it sees its own task.
    backend.push_when_user_contains("# Task", MockResponse::Text("subagent reply".into()));

    // Parent's three states (predicate-routed, sticky in case retries arise):
    //   no SUBAGENT yet, no tool result yet → spawn the subagent
    backend.push_sticky_matching(
        |req| !has_subagent_user_msg(req) && !has_tool_msg(req),
        MockResponse::ToolCalls {
            text: String::new(),
            calls: vec![MockToolCall::new(
                "c1",
                "start_subagent",
                r#"{"task":"do work"}"#,
            )],
        },
    );
    //   has tool result, no SUBAGENT yet → "kicking off" (forces suspend)
    backend.push_sticky_matching(
        |req| has_tool_msg(req) && !has_subagent_user_msg(req),
        MockResponse::Text("kicking off, will check back".into()),
    );
    //   SUBAGENT message present → final reply
    backend.push_sticky_when_user_contains("SUBAGENT ", MockResponse::Text("all done".into()));

    let ctx = make_ctx(
        backend.clone(),
        Arc::clone(&registry),
        tmp.path().join("subagent"),
    );
    let (mut inbox, tools) = agent::subagents::equip(ctx);
    let store = MemStore::new();
    let mut conv = Conversation::new("test").with_model("mock");

    let result = run_to_done(
        backend.as_ref(),
        &store,
        &mut conv,
        tools,
        &mut inbox,
        "Process the user request",
    )
    .await;

    assert_eq!(result.reply, "all done");
    assert_eq!(count_subagent_messages_in_conv(&conv), 1);
    assert_eq!(inbox.pending(), 0);
    assert_eq!(registry.len(), 0);

    let injected = conv
        .messages
        .iter()
        .find_map(|m| match m {
            ChatCompletionMessage::User(u) => match &u.content {
                UserContent::String(s) if s.starts_with("SUBAGENT ") => Some(s.clone()),
                _ => None,
            },
            _ => None,
        })
        .expect("SUBAGENT message exists");
    assert!(injected.contains("COMPLETED"));
    assert!(injected.contains("subagent reply"));
}

// ---------------------------------------------------------------------------
// 2. Fan-out: 3 subagents in one tool batch — all results land in conv
// ---------------------------------------------------------------------------

#[tokio::test]
async fn fan_out_three_subagents() {
    let backend = Arc::new(MockBackend::new());
    let registry = Arc::new(SubagentRegistry::new());
    let tmp = TempDir::new().unwrap();

    for tag in ["alpha", "bravo", "charlie"] {
        backend.push_when_user_contains(
            format!("task-{tag}"),
            MockResponse::Text(format!("done-{tag}")),
        );
    }

    // Parent: spawn 3, then wait, then "all three done" once they're all in.
    backend.push_sticky_matching(
        |req| !has_subagent_user_msg(req) && !has_tool_msg(req),
        MockResponse::ToolCalls {
            text: String::new(),
            calls: vec![
                MockToolCall::new("c1", "start_subagent", r#"{"task":"task-alpha"}"#),
                MockToolCall::new("c2", "start_subagent", r#"{"task":"task-bravo"}"#),
                MockToolCall::new("c3", "start_subagent", r#"{"task":"task-charlie"}"#),
            ],
        },
    );
    backend.push_sticky_matching(
        |req| count_subagent_user_msgs(req) >= 3,
        MockResponse::Text("all three done".into()),
    );
    // Otherwise (some subagents drained, others pending): no tool calls →
    // suspends so we can drain more.
    backend.push_sticky_matching(
        |req| has_tool_msg(req) && count_subagent_user_msgs(req) < 3,
        MockResponse::Text("waiting".into()),
    );

    let ctx = make_ctx(
        backend.clone(),
        Arc::clone(&registry),
        tmp.path().join("subagent"),
    );
    let (mut inbox, tools) = agent::subagents::equip(ctx);
    let store = MemStore::new();
    let mut conv = Conversation::new("fan-out").with_model("mock");

    let result = run_to_done(
        backend.as_ref(),
        &store,
        &mut conv,
        tools,
        &mut inbox,
        "Process the user request",
    )
    .await;

    assert_eq!(result.reply, "all three done");
    assert_eq!(count_subagent_messages_in_conv(&conv), 3);
    assert_eq!(inbox.pending(), 0);
    assert_eq!(registry.len(), 0);
}

// ---------------------------------------------------------------------------
// 3. Failed subagent → SUBAGENT … FAILED message
// ---------------------------------------------------------------------------

#[tokio::test]
async fn failed_subagent_propagates_as_failed_message() {
    let backend = Arc::new(MockBackend::new());
    let registry = Arc::new(SubagentRegistry::new());
    let tmp = TempDir::new().unwrap();

    backend.push_when_user_contains(
        "# Task",
        MockResponse::Error("upstream LLM exploded".into()),
    );
    backend.push_sticky_matching(
        |req| !has_subagent_user_msg(req) && !has_tool_msg(req),
        MockResponse::ToolCalls {
            text: String::new(),
            calls: vec![MockToolCall::new(
                "c1",
                "start_subagent",
                r#"{"task":"will-fail"}"#,
            )],
        },
    );
    backend.push_sticky_matching(
        |req| has_tool_msg(req) && !has_subagent_user_msg(req),
        MockResponse::Text("starting".into()),
    );
    backend.push_sticky_when_user_contains(
        "SUBAGENT ",
        MockResponse::Text("noted the failure".into()),
    );

    let ctx = make_ctx(
        backend.clone(),
        Arc::clone(&registry),
        tmp.path().join("subagent"),
    );
    let (mut inbox, tools) = agent::subagents::equip(ctx);
    let store = MemStore::new();
    let mut conv = Conversation::new("fail").with_model("mock");

    let result = run_to_done(
        backend.as_ref(),
        &store,
        &mut conv,
        tools,
        &mut inbox,
        "Process the user request",
    )
    .await;

    assert_eq!(result.reply, "noted the failure");
    let sub = conv
        .messages
        .iter()
        .find_map(|m| match m {
            ChatCompletionMessage::User(u) => match &u.content {
                UserContent::String(s) if s.starts_with("SUBAGENT ") => Some(s.clone()),
                _ => None,
            },
            _ => None,
        })
        .expect("SUBAGENT message exists");
    assert!(sub.contains("FAILED"));
    assert!(sub.contains("upstream LLM exploded"));
    assert_eq!(registry.len(), 0);
}

// ---------------------------------------------------------------------------
// 4. Depth cap: at MAX, the tool returns rejected without spawning.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn depth_cap_rejects_spawn() {
    use agent::prompts::MAX_SUBAGENT_DEPTH;

    let backend = Arc::new(MockBackend::new());
    let registry = Arc::new(SubagentRegistry::new());
    let tmp = TempDir::new().unwrap();

    let mut ctx = make_ctx(
        backend.clone(),
        Arc::clone(&registry),
        tmp.path().join("subagent"),
    );
    ctx.depth = MAX_SUBAGENT_DEPTH;

    let (_inbox, tools) = agent::subagents::equip(ctx);

    let has_start_subagent = tools.definitions().iter().any(|t| match t {
        agent::llm::ChatCompletionTool::Function { function } => function.name == "start_subagent",
        _ => false,
    });
    assert!(has_start_subagent, "start_subagent should be registered");

    let result = tools
        .call("start_subagent", r#"{"task":"deep"}"#)
        .await
        .expect("tool call should not error — caps return Ok(JSON)");

    assert_eq!(result["status"].as_str(), Some("rejected"));
    assert_eq!(registry.len(), 0, "rejected spawns must not touch registry");
}

// ---------------------------------------------------------------------------
// 5. Recursion: grandchild's parent_id chain in the registry
// ---------------------------------------------------------------------------

#[tokio::test]
async fn registry_records_parent_id_for_grandchildren() {
    let backend = Arc::new(MockBackend::new());
    let registry = Arc::new(SubagentRegistry::new());
    let tmp = TempDir::new().unwrap();

    // Grandchild: gated text reply — held until the test releases it after
    // observing both subagent and grandchild live in the registry.
    let grandchild_gate = Arc::new(Notify::new());
    backend.push_when_user_contains(
        "# Task\n\ngrandchild-task",
        MockResponse::gated(
            Arc::clone(&grandchild_gate),
            MockResponse::Text("grandchild-done".into()),
        ),
    );

    // Subagent: first call (no SUBAGENT in its conv yet) spawns a grandchild.
    backend.push_sticky_matching(
        |req| {
            user_strings(req)
                .iter()
                .any(|s| s.contains("# Task\n\nparent-task"))
                && !has_subagent_user_msg(req)
                && !has_tool_msg(req)
        },
        MockResponse::ToolCalls {
            text: String::new(),
            calls: vec![MockToolCall::new(
                "g1",
                "start_subagent",
                r#"{"task":"grandchild-task"}"#,
            )],
        },
    );
    // Subagent's "waiting" turn (has tool result, no SUBAGENT yet).
    backend.push_sticky_matching(
        |req| {
            user_strings(req)
                .iter()
                .any(|s| s.contains("# Task\n\nparent-task"))
                && has_tool_msg(req)
                && !has_subagent_user_msg(req)
        },
        MockResponse::Text("subagent waiting on grandchild".into()),
    );
    // Subagent's final reply once SUBAGENT msg is in its conv.
    backend.push_sticky_matching(
        |req| {
            user_strings(req)
                .iter()
                .any(|s| s.contains("# Task\n\nparent-task"))
                && has_subagent_user_msg(req)
        },
        MockResponse::Text("subagent-final".into()),
    );

    // Top-level parent: spawn → wait → final.
    backend.push_sticky_matching(
        |req| {
            !user_strings(req).iter().any(|s| s.contains("# Task"))
                && !has_subagent_user_msg(req)
                && !has_tool_msg(req)
        },
        MockResponse::ToolCalls {
            text: String::new(),
            calls: vec![MockToolCall::new(
                "p1",
                "start_subagent",
                r#"{"task":"parent-task"}"#,
            )],
        },
    );
    backend.push_sticky_matching(
        |req| {
            !user_strings(req).iter().any(|s| s.contains("# Task"))
                && has_tool_msg(req)
                && !has_subagent_user_msg(req)
        },
        MockResponse::Text("top dispatched".into()),
    );
    backend.push_sticky_matching(
        |req| !user_strings(req).iter().any(|s| s.contains("# Task")) && has_subagent_user_msg(req),
        MockResponse::Text("parent-final".into()),
    );

    let ctx = make_ctx(
        backend.clone(),
        Arc::clone(&registry),
        tmp.path().join("subagent"),
    );
    let (mut inbox, tools) = agent::subagents::equip(ctx);
    let store = MemStore::new();
    let mut conv = Conversation::new("recurse").with_model("mock");

    let registry_for_check: Arc<SubagentRegistry> = Arc::clone(&registry);
    let backend_for_run = backend.clone();

    let runner = tokio::spawn(async move {
        run_to_done(
            backend_for_run.as_ref(),
            &store,
            &mut conv,
            tools,
            &mut inbox,
            "Process the user request",
        )
        .await
    });

    // Poll the registry until both subagent and grandchild are live. The
    // grandchild's response is gated, so it'll stay in the registry until we
    // release it.
    let mut seen_chain = false;
    for _ in 0..200 {
        let snap = registry_for_check.snapshot();
        if snap.len() == 2 {
            let mut by_depth = snap;
            by_depth.sort_by_key(|v| v.depth);
            let parent = &by_depth[0];
            let grand = &by_depth[1];
            assert_eq!(parent.depth, 0);
            assert_eq!(parent.parent_id, None);
            assert_eq!(grand.depth, 1);
            assert_eq!(grand.parent_id, Some(parent.id.clone()));
            seen_chain = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert!(seen_chain, "expected 2-entry registry snapshot");

    // Release the grandchild and let the run complete.
    grandchild_gate.notify_waiters();

    let result = runner.await.unwrap();
    assert_eq!(result.reply, "parent-final");
    assert_eq!(registry_for_check.len(), 0);
}

// ---------------------------------------------------------------------------
// 6. No tool calls + no pending → Done immediately
// ---------------------------------------------------------------------------

#[tokio::test]
async fn no_tool_calls_no_pending_returns_done_immediately() {
    let backend = Arc::new(MockBackend::new());
    let registry = Arc::new(SubagentRegistry::new());
    let tmp = TempDir::new().unwrap();
    backend.push_text("hi");

    let ctx = make_ctx(
        backend.clone(),
        Arc::clone(&registry),
        tmp.path().join("subagent"),
    );
    let (mut inbox, tools) = agent::subagents::equip(ctx);
    let store = MemStore::new();
    let mut conv = Conversation::new("plain").with_model("mock");

    let outcome = agent_loop::run(
        backend.as_ref(),
        &store,
        &mut conv,
        tools,
        "say hi",
        None,
        None,
        Some(&mut inbox),
        None,
    )
    .await
    .unwrap();

    match outcome {
        RunOutcome::Done(r) => assert_eq!(r.reply, "hi"),
        _ => panic!("expected Done"),
    }
}

// ---------------------------------------------------------------------------
// 7. SubagentRegistry — drains to empty after a full spawn cycle
// ---------------------------------------------------------------------------

#[tokio::test]
async fn registry_drains_after_completion() {
    let backend = Arc::new(MockBackend::new());
    let registry = Arc::new(SubagentRegistry::new());
    let tmp = TempDir::new().unwrap();

    backend.push_when_user_contains("# Task", MockResponse::Text("done".into()));
    backend.push_sticky_matching(
        |req| !has_subagent_user_msg(req) && !has_tool_msg(req),
        MockResponse::ToolCalls {
            text: String::new(),
            calls: vec![MockToolCall::new("c", "start_subagent", r#"{"task":"x"}"#)],
        },
    );
    backend.push_sticky_matching(
        |req| has_tool_msg(req) && !has_subagent_user_msg(req),
        MockResponse::Text("started".into()),
    );
    backend.push_sticky_when_user_contains("SUBAGENT ", MockResponse::Text("ok".into()));

    let ctx = make_ctx(
        backend.clone(),
        Arc::clone(&registry),
        tmp.path().join("subagent"),
    );
    let (mut inbox, tools) = agent::subagents::equip(ctx);
    let store = MemStore::new();
    let mut conv = Conversation::new("reg").with_model("mock");

    let _ = run_to_done(
        backend.as_ref(),
        &store,
        &mut conv,
        tools,
        &mut inbox,
        "Process the user request",
    )
    .await;

    assert_eq!(registry.len(), 0);
}

// ---------------------------------------------------------------------------
// 8. Interleaved user/subagent communication.
//
// Scenario: user sends MESSAGE-A → agent spawns subagent and suspends. User
// sends MESSAGE-B in the meantime (a fresh agent::run on the same conv, which
// is what the server does today). Subagent finally finishes; the watcher
// injects + resumes.
//
// This test pins down what the watcher's resumed LLM call sees. Today the
// watcher operates on a stale in-memory `Conversation` that pre-dates
// MESSAGE-B and never reloads from disk → MESSAGE-B is invisible to it.
// We assert that explicitly so a future fix that reloads the conversation
// flips the assertion.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn interleaved_user_messages_during_suspension() {
    let backend = Arc::new(MockBackend::new());
    let registry = Arc::new(SubagentRegistry::new());
    let tmp = TempDir::new().unwrap();

    // Subagent: gated text reply — held until the test explicitly releases
    // it (after MESSAGE-B has been processed). This guarantees the parent's
    // first run reaches `Suspended` instead of racing to `Done`.
    let subagent_gate = Arc::new(Notify::new());
    backend.push_when_user_contains(
        "# Task",
        MockResponse::gated(
            Arc::clone(&subagent_gate),
            MockResponse::Text("subagent finished".into()),
        ),
    );

    // Phase 1 — first user turn: spawn. Routed by MESSAGE-A presence and
    // no tool result yet, no SUBAGENT yet.
    backend.push_sticky_matching(
        |req| {
            user_strings(req).iter().any(|s| s.contains("MESSAGE-A"))
                && !has_subagent_user_msg(req)
                && !has_tool_msg(req)
        },
        MockResponse::ToolCalls {
            text: String::new(),
            calls: vec![MockToolCall::new(
                "c1",
                "start_subagent",
                r#"{"task":"long work"}"#,
            )],
        },
    );
    // Phase 1 cont. — has tool result, no SUBAGENT, no MESSAGE-B yet → suspend.
    backend.push_sticky_matching(
        |req| {
            user_strings(req).iter().any(|s| s.contains("MESSAGE-A"))
                && has_tool_msg(req)
                && !has_subagent_user_msg(req)
                && !user_strings(req).iter().any(|s| s.contains("MESSAGE-B"))
        },
        MockResponse::Text("started, suspending".into()),
    );

    // Phase 2 — second user turn (MESSAGE-B) on a fresh agent::run.
    backend.push_sticky_matching(
        |req| {
            user_strings(req).iter().any(|s| s.contains("MESSAGE-B")) && !has_subagent_user_msg(req)
        },
        MockResponse::Text("acknowledging your second message".into()),
    );

    // Phase 3 — watcher resumes after subagent completes; conv has SUBAGENT.
    backend.push_sticky_when_user_contains(
        "SUBAGENT ",
        MockResponse::Text("watcher's final reply".into()),
    );

    let ctx = make_ctx(
        backend.clone(),
        Arc::clone(&registry),
        tmp.path().join("subagent"),
    );
    let (mut inbox, tools) = agent::subagents::equip(ctx);
    let store = MemStore::new();
    let mut conv = Conversation::new("interleave").with_model("mock");

    // ----- Phase 1: spawn + suspend.
    let outcome = agent_loop::run(
        backend.as_ref(),
        &store,
        &mut conv,
        Arc::clone(&tools),
        "MESSAGE-A from user",
        None,
        None,
        Some(&mut inbox),
        None,
    )
    .await
    .unwrap();

    let suspended = matches!(outcome, RunOutcome::Suspended { .. });
    assert!(
        suspended,
        "expected Suspended after phase 1, got {outcome:?}"
    );

    // The server hands the parent's `conv` + inbox to a background watcher.
    // The watcher holds the inbox across iterations (its sender lives in the
    // tools registry built for phase 1) but reloads `conv` from storage on
    // every iteration so it sees any user messages that arrived in the
    // meantime — see `subagent_continuation_loop` in the server crate.
    let conv_id = conv.id.clone();
    drop(conv);

    // ----- Phase 2: another user turn while subagent is still in flight.
    // Fresh agent::run on the SAME on-disk conversation, exactly as the
    // server does (each `conversation.send` mints a new inbox).
    let (mut second_inbox, second_tools) = agent::subagents::equip(make_ctx(
        backend.clone(),
        Arc::clone(&registry),
        tmp.path().join("subagent"),
    ));
    let mut second_conv = agent::conversation::load(&store, &conv_id).expect("conv load failed");
    let outcome2 = agent_loop::run(
        backend.as_ref(),
        &store,
        &mut second_conv,
        second_tools,
        "MESSAGE-B from user",
        None,
        None,
        Some(&mut second_inbox),
        None,
    )
    .await
    .unwrap();
    match outcome2 {
        RunOutcome::Done(r) => {
            assert_eq!(r.reply, "acknowledging your second message");
        }
        _ => panic!("expected Done after second user turn, got {outcome2:?}"),
    }

    // ----- Phase 3: release the subagent and let it finish; watcher reloads,
    // injects, and resumes — exactly the shape of the server's continuation.
    subagent_gate.notify_waiters();
    let sub_result = inbox.recv().await.expect("subagent should report back");
    let mut watcher_conv =
        agent::conversation::load(&store, &conv_id).expect("watcher conv reload failed");
    agent_loop::inject_subagent_result(
        &store,
        &mut watcher_conv,
        inbox.pending_counter(),
        sub_result,
    )
    .unwrap();

    let outcome3 = agent_loop::resume(
        backend.as_ref(),
        &store,
        &mut watcher_conv,
        tools,
        None,
        None,
        Some(&mut inbox),
        None,
    )
    .await
    .unwrap();
    match outcome3 {
        RunOutcome::Done(r) => assert_eq!(r.reply, "watcher's final reply"),
        _ => panic!("expected Done after watcher resume"),
    }

    // ----- The watcher's resume LLM call MUST see MESSAGE-B now. The reload-
    // before-inject contract is the whole point of this scenario.
    let last_request = backend
        .requests()
        .into_iter()
        .last()
        .expect("at least one request");
    let saw_message_b = user_strings(&last_request)
        .iter()
        .any(|s| s.contains("MESSAGE-B"));
    assert!(
        saw_message_b,
        "watcher's resumed LLM call must see MESSAGE-B (reload-before-inject); \
         if this regresses, subagent_continuation_loop is no longer reloading \
         the conversation from storage between iterations."
    );
}
