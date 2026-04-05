// ---------------------------------------------------------------------------
// index.tsx — MVP entry point
//
// Runs the capability probe, spawns the RPC client, creates a conversation,
// and renders <App/>. Composer submissions flow: push 'user' item → call
// conversation.send → push 'assistant' item (or 'error').
// ---------------------------------------------------------------------------

import { render } from "ink";
import { terminalCapabilityManager } from "./terminal/terminalCapabilityManager.ts";
import { historyStore } from "./state/historyStore.ts";
import { RpcClient } from "./rpc/client.ts";
import { App } from "./ui/App.tsx";

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
  const conversationId = createRes.value.id;

  historyStore.addItem({
    type: "info",
    text: `Connected. Conversation id=${conversationId}. Type a message and hit Enter.`,
  });

  // 3. Composer submit handler: push user turn, call RPC, push reply.
  async function onSubmit(text: string): Promise<void> {
    historyStore.addItem({ type: "user", text });

    const res = await rpc.call("conversation.send", {
      id: conversationId,
      message: text,
    });

    if (!res.ok) {
      historyStore.addItem({
        type: "error",
        message: `RPC error (${res.error.code}): ${res.error.message}`,
      });
      return;
    }

    historyStore.addItem({ type: "assistant", text: res.value.reply });

    for (const call of res.value.tool_calls ?? []) {
      historyStore.addItem({
        type: "tool",
        name: call.name,
        result: call.result,
      });
    }
  }

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
