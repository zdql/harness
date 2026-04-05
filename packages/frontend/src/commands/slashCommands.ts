// ---------------------------------------------------------------------------
// commands/slashCommands.ts
//
// Registry of client-side slash commands (commands that don't get sent to
// the agent). Each command has a name, description, and an async run() that
// receives parsed args and a SlashContext giving it access to the RPC client,
// the history store, and the mutable conversation id.
//
// To add a new command, append a SlashCommand to the `slashCommands` array.
// ---------------------------------------------------------------------------

import type { RpcClient } from "../rpc/client.ts";
import { historyStore } from "../state/historyStore.ts";

export interface SlashContext {
  rpc: RpcClient;
  getConversationId: () => string;
  setConversationId: (id: string) => void;
}

export interface SlashCommand {
  name: string;
  description: string;
  run: (args: string[], ctx: SlashContext) => Promise<void> | void;
}

export const slashCommands: SlashCommand[] = [
  {
    name: "clear",
    description: "Start a new conversation and clear the screen",
    run: async (_args, ctx) => {
      const res = await ctx.rpc.call("conversation.create", {});
      if (!res.ok) {
        historyStore.addItem({
          type: "error",
          message: `Failed to create conversation: ${res.error.message}`,
        });
        return;
      }
      ctx.setConversationId(res.value.id);
      historyStore.clear();
      // Clear the terminal: <Static> items already live in the scrollback
      // buffer, so remounting alone won't remove them. ESC[2J clears the
      // visible screen, ESC[3J clears the scrollback, ESC[H homes the cursor.
      process.stdout.write("\x1b[2J\x1b[3J\x1b[H");
      historyStore.addItem({
        type: "info",
        text: `Cleared. New conversation id=${res.value.id}.`,
      });
    },
  },
  {
    name: "help",
    description: "List available slash commands",
    run: (_args) => {
      const lines = slashCommands
        .map((c) => `  /${c.name} — ${c.description}`)
        .join("\n");
      historyStore.addItem({ type: "info", text: `Commands:\n${lines}` });
    },
  },
];

/**
 * Parse a slash-prefixed input line and dispatch to a command. Returns true
 * if the text was handled as a command (whether it matched or not) and
 * therefore should NOT be forwarded to the agent.
 */
export async function dispatchSlashCommand(
  text: string,
  ctx: SlashContext,
): Promise<boolean> {
  if (!text.startsWith("/")) return false;
  const [name = "", ...args] = text.slice(1).trim().split(/\s+/);
  const cmd = slashCommands.find((c) => c.name === name);
  if (!cmd) {
    historyStore.addItem({
      type: "error",
      message: `Unknown command: /${name}. Try /help.`,
    });
    return true;
  }
  await cmd.run(args, ctx);
  return true;
}
