// ---------------------------------------------------------------------------
// ui/SettingsView.tsx
//
// Full-screen settings editor. Rendered by DefaultAppLayout in place of the
// normal chat when `overlayStore.get() === "settings"`. Loads settings via
// RPC on mount, lets the user cycle through values for each field with
// arrow keys, and saves everything in a single `settings.update` call.
//
// UX:
//   ↑ / ↓   move between fields
//   ← / →   cycle value of the current field
//   s       save and close
//   esc/q   cancel and close
//
// All three fields are cyclers. `Model` offers a preset list plus a
// "default (unset)" choice that clears the setting on save. `Reasoning
// effort` / `Reasoning summary` cycle over the values the backend accepts.
// Custom model strings still work — edit ~/.agent-harness/settings.json
// directly or open it with another editor.
// ---------------------------------------------------------------------------

import { Box, Text } from "ink";
import { useEffect, useState } from "react";
import type { RpcClient } from "../rpc/client.ts";
import { useKeypress, type Key } from "../keypress/useKeypress.ts";
import { overlayStore } from "../state/overlayStore.ts";
import { historyStore } from "../state/historyStore.ts";

// Sentinel shown in the UI to mean "clear this setting and use the backend
// default". On save it maps to the empty string, which the Rust handler
// treats as `None`.
const UNSET = "(default)";

// Preset model slugs offered in the cycler. Order roughly mirrors the
// OpenRouter usage leaderboard so the popular models surface first when the
// user starts cycling. Custom slugs typed into the settings file still work.

const MODEL_PRESETS = [
  UNSET,
  "anthropic/claude-opus-4-6",
  "anthropic/claude-sonnet-4-5",
  "anthropic/claude-haiku-4-5",
  "openai/gpt-5.5",
  "openai/gpt-5",
  "openai/gpt-5-mini",
  "openai/gpt-5-nano",
  "openai/gpt-4o",
  "openai/gpt-4o-mini",
  "openai/o3",
  "openai/o3-mini",
  "google/gemini-2.5-pro",
  "google/gemini-2.5-flash",
  "tencent/hy3-preview:free",
  "moonshotai/kimi-k2.6",
  "deepseek/deepseek-v4-pro",
  "deepseek/deepseek-v4-flash",
  "deepseek/deepseek-chat-v3.1",
  "deepseek/deepseek-r1",
  "z-ai/glm-5.1",
  "inclusionai/ling-2.6-1t:free",
  "minimax/minimax-m2.7",
  "stepfun/step-3.5-flash",
  "x-ai/grok-4-fast",
  "qwen/qwen3-coder",
  "meta-llama/llama-3.3-70b-instruct",
];

const REASONING_EFFORT_OPTIONS = [
  UNSET,
  "off",
  "none",
  "minimal",
  "low",
  "medium",
  "high",
  "xhigh",
];

const REASONING_SUMMARY_OPTIONS = [UNSET, "auto", "concise", "detailed"];

interface Field {
  key: "model" | "reasoning_effort" | "reasoning_summary";
  label: string;
  options: string[];
}

const FIELDS: Field[] = [
  { key: "model", label: "Model", options: MODEL_PRESETS },
  {
    key: "reasoning_effort",
    label: "Reasoning effort",
    options: REASONING_EFFORT_OPTIONS,
  },
  {
    key: "reasoning_summary",
    label: "Reasoning summary",
    options: REASONING_SUMMARY_OPTIONS,
  },
];

type Values = Record<Field["key"], string>;

interface Props {
  rpc: RpcClient;
}

export function SettingsView({ rpc }: Props): React.JSX.Element {
  const [values, setValues] = useState<Values | null>(null);
  const [cursor, setCursor] = useState(0);
  const [status, setStatus] = useState<string>("loading…");

  // Load current settings on mount.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const res = await rpc.call("settings.get", {});
      if (cancelled) return;
      if (!res.ok) {
        setStatus(`failed to load settings: ${res.error.message}`);
        return;
      }
      setValues({
        model: res.value.model ?? UNSET,
        reasoning_effort: res.value.reasoning_effort ?? UNSET,
        reasoning_summary: res.value.reasoning_summary ?? UNSET,
      });
      setStatus("");
    })();
    return () => {
      cancelled = true;
    };
  }, [rpc]);

  const close = (): void => {
    overlayStore.set("none");
  };

  const save = async (): Promise<void> => {
    if (!values) return;
    setStatus("saving…");
    const toEmpty = (v: string): string => (v === UNSET ? "" : v);
    const res = await rpc.call("settings.update", {
      model: toEmpty(values.model),
      reasoning_effort: toEmpty(values.reasoning_effort),
      reasoning_summary: toEmpty(values.reasoning_summary),
    });
    if (!res.ok) {
      setStatus(`save failed: ${res.error.message}`);
      return;
    }

    // Also push the new model onto the active conversation so it takes
    // effect on the next send instead of waiting for /clear. Empty string
    // clears the per-conversation override so the global default kicks in.
    const activeId = res.value.conversation;
    if (activeId) {
      const convRes = await rpc.call("conversation.setModel", {
        id: activeId,
        model: toEmpty(values.model),
      });
      if (!convRes.ok) {
        setStatus(`save failed: ${convRes.error.message}`);
        return;
      }
    }

    historyStore.addItem({
      type: "info",
      text:
        `Settings saved. Model=${res.value.model ?? "(default)"}, ` +
        `reasoning_effort=${res.value.reasoning_effort ?? "(default)"}, ` +
        `reasoning_summary=${res.value.reasoning_summary ?? "(default)"}.`,
    });
    close();
  };

  const cycle = (delta: 1 | -1): void => {
    if (!values) return;
    const field = FIELDS[cursor];
    if (!field) return;
    // Current option: might be a user-set value that isn't in the preset list.
    // Treat an off-list value as if it sat just before index 0 so cycling
    // forward lands on options[0] and backward lands on options[last].
    const current = values[field.key];
    const idx = field.options.indexOf(current);
    const len = field.options.length;
    const next =
      idx === -1
        ? delta === 1
          ? field.options[0]!
          : field.options[len - 1]!
        : field.options[(idx + delta + len) % len]!;
    setValues({ ...values, [field.key]: next });
  };

  useKeypress(
    (key: Key): boolean | void => {
      // Handle keys whether or not values have loaded — esc/q always works.
      if (key.name === "escape" || (key.name === "q" && !key.ctrl)) {
        close();
        return true;
      }
      if (!values) return;
      if (key.name === "up" || key.name === "k") {
        setCursor((c) => (c - 1 + FIELDS.length) % FIELDS.length);
        return true;
      }
      if (key.name === "down" || key.name === "j") {
        setCursor((c) => (c + 1) % FIELDS.length);
        return true;
      }
      if (key.name === "left" || key.name === "h") {
        cycle(-1);
        return true;
      }
      if (key.name === "right" || key.name === "l") {
        cycle(1);
        return true;
      }
      if (key.name === "s" && !key.ctrl) {
        void save();
        return true;
      }
      return;
    },
    { isActive: true, priority: true },
  );

  return (
    <Box flexDirection="column" paddingX={1} paddingY={0}>
      <Box marginBottom={1}>
        <Text bold color="cyan">
          Settings
        </Text>
        <Text dimColor>  ·  editing ~/.agent-harness/settings.json</Text>
      </Box>

      {values ? (
        <Box flexDirection="column">
          {FIELDS.map((field, i) => {
            const active = i === cursor;
            const val = values[field.key];
            return (
              <Box key={field.key}>
                <Text color={active ? "cyan" : undefined}>
                  {active ? "▶ " : "  "}
                </Text>
                <Box width={22}>
                  <Text color={active ? "cyan" : undefined}>
                    {field.label}
                  </Text>
                </Box>
                <Text dimColor>◀ </Text>
                <Text
                  color={active ? "cyan" : "white"}
                  bold={active}
                >
                  {val}
                </Text>
                <Text dimColor> ▶</Text>
              </Box>
            );
          })}
        </Box>
      ) : (
        <Text dimColor>{status || "loading…"}</Text>
      )}

      {status && values ? (
        <Box marginTop={1}>
          <Text color="yellow">{status}</Text>
        </Box>
      ) : null}

      <Box marginTop={1}>
        <Text dimColor>
          ↑/↓ move   ←/→ change   s save   esc cancel
        </Text>
      </Box>
    </Box>
  );
}
