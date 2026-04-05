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

async function main(): Promise<void> {
  // 1. Probe terminal capabilities (Kitty / modifyOtherKeys / bg color).
  //    Must run before rendering so raw mode + escape queries don't collide
  //    with React/Ink's own stdin setup.
  await terminalCapabilityManager.detectCapabilities();

  // 2. Spawn the Rust server + create a conversation to talk to.
  const rpc = new RpcClient();

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

  // 3. Composer submit handler: push user turn, call RPC, push reply.
  //
  // While the request is in flight, the server streams `agent.event`
  // notifications (LlmStart/LlmEnd/ToolCallStart/ToolCallEnd). We translate
  // those into the pending-items list so the user sees a thinking spinner
  // and tool calls appear in real time. When the final response arrives we
  // commit the completed tool items to history and drop the spinner.
  async function onSubmit(text: string): Promise<void> {
    if (await dispatchSlashCommand(text, slashCtx)) return;

    historyStore.addItem({ type: "user", text });

    // Completed tool items, in order. The spinner (thinking / tool-running)
    // is always the last entry in pending until the response lands.
    const completed: HistoryItemInput[] = [];
    let spinner: HistoryItemInput = { type: "thinking", label: "Thinking…" };
    // Live-streamed reasoning text, reset on each new LLM call. Shown as the
    // thinking spinner's label so the user sees the model's chain of thought
    // as it arrives.
    let reasoningBuf = "";
    const pushPending = () => historyStore.setPending([...completed, spinner]);
    pushPending();

    // Tail of the reasoning buffer used as the spinner label. Keeps the UI
    // stable-width by taking the last ~80 chars of the latest line.
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
          // Final reply is streaming in — keep the spinner generic; the
          // assembled reply is committed once the RPC resolves.
          break;
        case "tool_call_start":
          spinner = { type: "tool-running", name: ev.name };
          pushPending();
          break;
        case "tool_call_end":
          completed.push({
            type: "tool",
            name: ev.name,
            result: ev.result,
          });
          // Reset to a thinking spinner — the agent is about to call the
          // LLM again (unless this was the last tool of the turn).
          reasoningBuf = "";
          spinner = { type: "thinking", label: "Thinking…" };
          pushPending();
          break;
        case "llm_end":
          // No UI change on its own — we'll either see more tool calls or
          // the final response arrives next.
          break;
        case "subagent_started":
          completed.push({
            type: "tool",
            name: "start_subagent",
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
          // Nested events from within a subagent are not shown inline today
          // — a dedicated subagent panel is a follow-up task.
          break;
      }
    });

    const res = await rpc.call("conversation.send", {
      id: conversationId,
      message: text,
    });

    rpc.onNotification(null);

    // Drop the spinner, commit completed tool items to scrollback.
    historyStore.setPending(completed);
    historyStore.commitPending();

    if (!res.ok) {
      historyStore.addItem({
        type: "error",
        message: `RPC error (${res.error.code}): ${res.error.message}`,
      });
      return;
    }

    historyStore.addItem({ type: "assistant", text: res.value.reply });
  }

  type AgentEvent =
    | { kind: "llm_start" }
    | { kind: "llm_end" }
    | { kind: "reasoning_delta"; text: string }
    | { kind: "content_delta"; text: string }
    | { kind: "tool_call_start"; name: string; arguments: string }
    | { kind: "tool_call_end"; name: string; result: string }
    | { kind: "subagent_started"; subagent_id: string; task: string }
    | {
        kind: "subagent_completed";
        subagent_id: string;
        status: string;
        output: string;
      }
    | { kind: "subagent_event"; subagent_id: string; inner: AgentEvent };

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
