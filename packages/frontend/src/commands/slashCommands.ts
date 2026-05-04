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
import { overlayStore } from "../state/overlayStore.ts";

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

// Short aliases → full OpenRouter model slugs. Keeps `/model haiku` as a
// one-keystroke way to swap to something cheap. Anything not in this map is
// passed through as-is, so `/model some/custom-model` still works.
const MODEL_ALIASES: Record<string, string> = {
  // Anthropic
  opus: "anthropic/claude-opus-4-6",
  sonnet: "anthropic/claude-sonnet-4-5",
  haiku: "anthropic/claude-haiku-4-5",
  // OpenAI
  "gpt-5.5": "openai/gpt-5.5",
  "gpt55": "openai/gpt-5.5",
  "gpt-5": "openai/gpt-5",
  "gpt5": "openai/gpt-5",
  mini: "openai/gpt-5-mini",
  nano: "openai/gpt-5-nano",
  "4o": "openai/gpt-4o",
  "4o-mini": "openai/gpt-4o-mini",
  o3: "openai/o3",
  "o3-mini": "openai/o3-mini",
  // Google
  gemini: "google/gemini-2.5-pro",
  "gemini-flash": "google/gemini-2.5-flash",
  flash: "google/gemini-2.5-flash",
  // DeepSeek
  deepseek: "deepseek/deepseek-chat-v3.1",
  "deepseek-v4": "deepseek/deepseek-v4-pro",
  "deepseek-pro": "deepseek/deepseek-v4-pro",
  v4: "deepseek/deepseek-v4-pro",
  "deepseek-flash": "deepseek/deepseek-v4-flash",
  "v4-flash": "deepseek/deepseek-v4-flash",
  "deepseek-r1": "deepseek/deepseek-r1",
  r1: "deepseek/deepseek-r1",
  // Moonshot
  kimi: "moonshotai/kimi-k2.6",
  "kimi-k2": "moonshotai/kimi-k2.6",
  k2: "moonshotai/kimi-k2.6",
  // Tencent
  hy3: "tencent/hy3-preview:free",
  "hy3-preview": "tencent/hy3-preview:free",
  hunyuan: "tencent/hy3-preview:free",
  // Z.ai
  glm: "z-ai/glm-5.1",
  "glm-5.1": "z-ai/glm-5.1",
  // InclusionAI
  ling: "inclusionai/ling-2.6-1t:free",
  "ling-1t": "inclusionai/ling-2.6-1t:free",
  // MiniMax
  minimax: "minimax/minimax-m2.7",
  m2: "minimax/minimax-m2.7",
  "m2.7": "minimax/minimax-m2.7",
  // StepFun
  step: "stepfun/step-3.5-flash",
  "step-flash": "stepfun/step-3.5-flash",
  // xAI
  grok: "x-ai/grok-4-fast",
  // Qwen
  qwen: "qwen/qwen3-coder",
  // Meta
  llama: "meta-llama/llama-3.3-70b-instruct",
};

/**
 * Resolve a user-typed model token to a full OpenRouter slug. If the input
 * already contains a `/` we assume it's a fully qualified slug and pass it
 * through. Otherwise we look it up in MODEL_ALIASES (case-insensitive). If
 * nothing matches, the original input is returned so the agent will get a
 * clear error from OpenRouter rather than silently using the wrong model.
 */
function resolveModel(input: string): string {
  const trimmed = input.trim();
  if (trimmed.includes("/")) return trimmed;
  const hit = MODEL_ALIASES[trimmed.toLowerCase()];
  return hit ?? trimmed;
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
    name: "settings",
    description: "Open the settings editor (model, reasoning effort, …)",
    run: () => {
      overlayStore.set("settings");
    },
  },
  {
    name: "conversations",
    description: "List and switch between past conversations",
    run: () => {
      overlayStore.set("conversations");
    },
  },
  {
    name: "model",
    description:
      "Show or switch the model. Usage: /model [name|alias|list|reset]",
    run: async (args, ctx) => {
      const sub = args[0]?.trim() ?? "";

      // /model — show the model the active conversation will use.
      if (sub === "") {
        const res = await ctx.rpc.call("hud.currentModel.get", {});
        if (!res.ok) {
          historyStore.addItem({
            type: "error",
            message: `Failed to read current model: ${res.error.message}`,
          });
          return;
        }
        historyStore.addItem({
          type: "info",
          text:
            `Current model: ${res.value.model}\n` +
            `Switch with /model <alias|slug>, or /model list to see aliases.`,
        });
        return;
      }

      // /model list — print known aliases.
      if (sub === "list" || sub === "ls") {
        const lines = Object.entries(MODEL_ALIASES)
          .map(([alias, slug]) => `  ${alias.padEnd(14)} → ${slug}`)
          .join("\n");
        historyStore.addItem({
          type: "info",
          text:
            `Known aliases (use any OpenRouter slug too):\n${lines}\n\n` +
            `Examples:\n` +
            `  /model haiku\n` +
            `  /model deepseek\n` +
            `  /model openai/gpt-5-mini\n` +
            `  /model reset   # use the global default again`,
        });
        return;
      }

      // /model reset — clear the per-conversation override + global setting.
      const isReset = sub === "reset" || sub === "default" || sub === "unset";
      const target = isReset ? "" : resolveModel(args.join(" "));

      // Update the global setting so future conversations also use it.
      const settingsRes = await ctx.rpc.call("settings.update", {
        model: target,
      });
      if (!settingsRes.ok) {
        historyStore.addItem({
          type: "error",
          message: `Failed to update settings: ${settingsRes.error.message}`,
        });
        return;
      }

      // And update the *current* conversation so the next send picks it up
      // without forcing the user to /clear.
      const convRes = await ctx.rpc.call("conversation.setModel", {
        id: ctx.getConversationId(),
        model: target,
      });
      if (!convRes.ok) {
        historyStore.addItem({
          type: "error",
          message:
            `Failed to update active conversation: ${convRes.error.message}`,
        });
        return;
      }

      const effective =
        convRes.value.model ?? settingsRes.value.model ?? "(default)";
      historyStore.addItem({
        type: "info",
        text: `Model set to ${effective}.`,
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
