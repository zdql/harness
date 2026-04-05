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
    const pushPending = () => historyStore.setPending([...completed, spinner]);
    pushPending();

    rpc.onNotification((method, params) => {
      if (method !== "agent.event") return;
      const ev = params as AgentEvent;
      switch (ev.kind) {
        case "llm_start":
          spinner = { type: "thinking", label: "Thinking…" };
          pushPending();
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
          spinner = { type: "thinking", label: "Thinking…" };
          pushPending();
          break;
        case "llm_end":
          // No UI change on its own — we'll either see more tool calls or
          // the final response arrives next.
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
    | { kind: "tool_call_start"; name: string; arguments: string }
    | { kind: "tool_call_end"; name: string; result: string };

  // 4. Render. exitOnCtrlC:false — our GlobalKeyHandler owns the quit path.
  const instance = render(
    <App
      onSubmit={(text) => {
        void onSubmit(text);
      }}
      placeholder="message the agent…"
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
