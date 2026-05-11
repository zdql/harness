// ---------------------------------------------------------------------------
// index.tsx — MVP entry point
//
// Runs the capability probe, spawns the RPC client, creates a conversation,
// and renders <App/>. Composer submissions flow: push 'user' item → call
// conversation.send → push 'assistant' item (or 'error').
// ---------------------------------------------------------------------------

import { render } from "ink";
import { terminalCapabilityManager } from "./terminal/terminalCapabilityManager.ts";
import { historyStore, type HistoryItemInput } from "./state/historyStore.ts";
import { RpcClient } from "./rpc/client.ts";
import { App } from "./ui/App.tsx";
import { dispatchSlashCommand } from "./commands/slashCommands.ts";
import { startMemoryLogger } from "./memoryLogger.ts";

async function main(): Promise<void> {
  // 1. Probe terminal capabilities (Kitty / modifyOtherKeys / bg color).
  //    Must run before rendering so raw mode + escape queries don't collide
  //    with React/Ink's own stdin setup.
  await terminalCapabilityManager.detectCapabilities();

  // 2. Spawn the Rust server + create a conversation to talk to.
  const rpc = new RpcClient();

  if (process.env["HARNESS_MEMORY_LOG"] === "1") {
    void startMemoryLogger(rpc);
  }

  const createRes = await rpc.call("conversation.create", {});
  if (!createRes.ok) {
    console.error("Failed to create conversation:", createRes.error);
    rpc.close();
    process.exit(1);
  }
  let conversationId = createRes.value.id;

  historyStore.addItem({
    type: "info",
    text: `Connected. Conversation id=${conversationId}. Type a message and hit Enter.`,
  });

  const slashCtx = {
    rpc,
    getConversationId: () => conversationId,
    setConversationId: (id: string) => {
      conversationId = id;
    },
  };

  // 3. Persistent event listener.
  //
  // Agent events can arrive both during a `conversation.send` RPC and after
  // it returns (when subagents are still running in the background). We keep
  // mutable state for the current streaming session and reset it per-send.

  type AgentEvent =
    | { kind: "llm_start" }
    | { kind: "llm_end" }
    | { kind: "reasoning_delta"; text: string }
    | { kind: "content_delta"; text: string }
    | { kind: "tool_call_start"; name: string; arguments: string }
    | { kind: "tool_call_end"; name: string; arguments: string; result: string }
    | { kind: "subagent_started"; subagent_id: string; task: string }
    | {
        kind: "subagent_completed";
        subagent_id: string;
        status: string;
        output: string;
      }
    | { kind: "subagent_event"; subagent_id: string; inner: AgentEvent }
    | { kind: "continuation_done"; reply: string };

  // Streaming session state — lives across the send boundary when suspended.
  let completed: HistoryItemInput[] = [];
  let spinner: HistoryItemInput = { type: "thinking", label: "Thinking…" };
  let reasoningBuf = "";
  let suspended = false;

  const pushPending = () => historyStore.setPending([...completed, spinner]);

  const reasoningLabel = (): string => {
    const lastLine = reasoningBuf.split("\n").filter(Boolean).pop() ?? "";
    const trimmed = lastLine.length > 80 ? `…${lastLine.slice(-80)}` : lastLine;
    return trimmed.length > 0 ? `Thinking… ${trimmed}` : "Thinking…";
  };

  rpc.onNotification((method, params) => {
    if (method !== "agent.event") return;
    const ev = params as AgentEvent;
    switch (ev.kind) {
      case "llm_start":
        reasoningBuf = "";
        spinner = { type: "thinking", label: "Thinking…" };
        pushPending();
        break;
      case "reasoning_delta":
        reasoningBuf += ev.text;
        spinner = { type: "thinking", label: reasoningLabel() };
        pushPending();
        break;
      case "content_delta":
        break;
      case "tool_call_start":
        spinner = { type: "tool-running", name: ev.name, arguments: ev.arguments };
        pushPending();
        break;
      case "tool_call_end":
        completed.push({
          type: "tool",
          name: ev.name,
          arguments: ev.arguments,
          result: ev.result,
        });
        reasoningBuf = "";
        spinner = { type: "thinking", label: "Thinking…" };
        pushPending();
        break;
      case "llm_end":
        break;
      case "subagent_started":
        completed.push({
          type: "tool",
          name: "start_subagent",
          arguments: JSON.stringify({ task: ev.task }),
          result: `started ${ev.subagent_id}: ${ev.task.slice(0, 80)}`,
        });
        pushPending();
        break;
      case "subagent_completed":
        completed.push({
          type: "tool",
          name: `subagent ${ev.subagent_id}`,
          result: `${ev.status}: ${ev.output.slice(0, 200)}`,
        });
        pushPending();
        break;
      case "subagent_event":
        break;
      case "continuation_done":
        // Background continuation finished — commit everything and show
        // the final reply.
        historyStore.setPending(completed);
        historyStore.commitPending();
        historyStore.addItem({ type: "assistant", text: ev.reply });
        completed = [];
        suspended = false;
        break;
    }
  });

  // 4. Composer submit handler.
  async function onSubmit(text: string): Promise<void> {
    if (await dispatchSlashCommand(text, slashCtx)) return;

    if (suspended) {
      historyStore.addItem({
        type: "info",
        text: "Subagents are still running — please wait for them to finish.",
      });
      return;
    }

    historyStore.addItem({ type: "user", text });

    // Reset streaming state for this send.
    completed = [];
    reasoningBuf = "";
    spinner = { type: "thinking", label: "Thinking…" };
    pushPending();

    const res = await rpc.call("conversation.send", {
      id: conversationId,
      message: text,
    });

    if (!res.ok) {
      historyStore.setPending([]);
      historyStore.addItem({
        type: "error",
        message: `RPC error (${res.error.code}): ${res.error.message}`,
      });
      return;
    }

    // Commit whatever tools accumulated during this turn before we either
    // hand off to subagents or finish. Otherwise the suspended branch
    // would discard the parent turn's tool history.
    historyStore.setPending(completed);
    historyStore.commitPending();
    completed = [];

    if (res.value.suspended) {
      suspended = true;
      spinner = { type: "thinking", label: "Waiting for subagents…" };
      pushPending();
    }

    historyStore.addItem({ type: "assistant", text: res.value.reply });
  }

  // 4. Render. exitOnCtrlC:false — our GlobalKeyHandler owns the quit path.
  const instance = render(
    <App
      onSubmit={(text) => {
        void onSubmit(text);
      }}
      placeholder="message the agent…"
      rpc={rpc}
    />,
    { exitOnCtrlC: false },
  );

  await instance.waitUntilExit();
  rpc.close();
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
