// ---------------------------------------------------------------------------
// screens/settings.tsx — Settings overlay (opened via Ctrl+S)
// ---------------------------------------------------------------------------

import React, { useEffect, useState, useCallback } from "react";
import { Box, Text } from "ink";
import type { RpcClient } from "../rpc/index.ts";
import type { GetResult } from "../rpc/methods/settings.ts";
import type { ConversationSummary } from "../rpc/methods/conversation.ts";
import {
  useKeypress,
  KeypressPriority,
  keyMatchers,
  Command,
  type Key,
} from "../input/index.ts";

interface Props {
  client: RpcClient;
  onClose: () => void;
  onSwitchConversation: (id: string) => void;
}

type Tab = "general" | "conversations";

// Valid OpenRouter model IDs (provider/model format)
const AVAILABLE_MODELS = [
  "openai/gpt-4o",
  "openai/gpt-4o-mini",
  "openai/gpt-4.1",
  "openai/gpt-4.1-mini",
  "openai/gpt-4.1-nano",
  "openai/o4-mini",
  "anthropic/claude-sonnet-4",
  "anthropic/claude-haiku-4",
  "google/gemini-2.5-pro-preview",
  "google/gemini-2.5-flash-preview",
  "google/gemini-2.0-flash-001",
  "deepseek/deepseek-chat-v3-0324",
  "deepseek/deepseek-r1",
  "meta-llama/llama-4-maverick",
  "meta-llama/llama-4-scout",
] as const;

export function SettingsScreen({ client, onClose, onSwitchConversation }: Props) {
  const [tab, setTab] = useState<Tab>("general");
  const [settings, setSettings] = useState<GetResult | null>(null);
  const [conversations, setConversations] = useState<ConversationSummary[]>([]);
  const [selectedIdx, setSelectedIdx] = useState(0);
  const [pickingModel, setPickingModel] = useState(false);
  const [modelIdx, setModelIdx] = useState(0);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    client.call("settings.get", {}).then(r => {
      if (r.ok) setSettings(r.value);
      else setError(r.error.message);
    });
    client.call("conversation.list", {}).then(r => {
      if (r.ok) setConversations(r.value.conversations);
      else setError(r.error.message);
    });
  }, []);

  function openModelPicker() {
    const current = settings?.model;
    const idx = current ? AVAILABLE_MODELS.indexOf(current as typeof AVAILABLE_MODELS[number]) : -1;
    setModelIdx(idx >= 0 ? idx : 0);
    setPickingModel(true);
  }

  const handler = useCallback(
    (key: Key): boolean | void => {
      // Model picker mode
      if (pickingModel) {
        if (keyMatchers[Command.DIALOG_UP](key)) {
          setModelIdx(i => Math.max(0, i - 1));
          return true;
        }
        if (keyMatchers[Command.DIALOG_DOWN](key)) {
          setModelIdx(i => Math.min(AVAILABLE_MODELS.length - 1, i + 1));
          return true;
        }
        if (keyMatchers[Command.DIALOG_SELECT](key)) {
          const model = AVAILABLE_MODELS[modelIdx]!;
          setPickingModel(false);
          client.call("settings.update", { model }).then(r => {
            if (r.ok) setSettings(r.value);
            else setError(r.error.message);
          });
          return true;
        }
        if (keyMatchers[Command.DIALOG_DISMISS](key)) {
          setPickingModel(false);
          return true;
        }
        return false;
      }

      // Tab switching
      if (key.name === "1" && !key.ctrl && !key.cmd) {
        setTab("general");
        setSelectedIdx(0);
        return true;
      }
      if (key.name === "2" && !key.ctrl && !key.cmd) {
        setTab("conversations");
        setSelectedIdx(0);
        return true;
      }
      if (keyMatchers[Command.MOVE_LEFT](key)) {
        setTab("general");
        setSelectedIdx(0);
        return true;
      }
      if (keyMatchers[Command.MOVE_RIGHT](key)) {
        setTab("conversations");
        setSelectedIdx(0);
        return true;
      }

      const items = tab === "general" ? ["model"] : conversations.map(c => c.id);
      const maxIdx = items.length - 1;

      if (keyMatchers[Command.DIALOG_UP](key)) {
        setSelectedIdx(i => Math.max(0, i - 1));
        return true;
      }
      if (keyMatchers[Command.DIALOG_DOWN](key)) {
        setSelectedIdx(i => Math.min(maxIdx, i + 1));
        return true;
      }
      if (keyMatchers[Command.DIALOG_SELECT](key) || key.name === "e") {
        if (tab === "general") {
          openModelPicker();
        } else if (tab === "conversations") {
          const conv = conversations[selectedIdx];
          if (conv) {
            onSwitchConversation(conv.id);
            onClose();
          }
        }
        return true;
      }

      return false;
    },
    [pickingModel, modelIdx, tab, selectedIdx, conversations, settings, client, onClose, onSwitchConversation],
  );

  useKeypress(handler, {
    isActive: true,
    priority: KeypressPriority.Critical,
  });

  if (error) {
    return (
      <Box flexDirection="column" padding={1} borderStyle="round" borderColor="yellow">
        <Text color="red">Error: {error}</Text>
      </Box>
    );
  }

  return (
    <Box flexDirection="column" padding={1} borderStyle="round" borderColor="yellow">
      <Box marginBottom={1} gap={2}>
        <Text bold underline={tab === "general"} color={tab === "general" ? "cyan" : undefined}>
          General (1)
        </Text>
        <Text bold underline={tab === "conversations"} color={tab === "conversations" ? "cyan" : undefined}>
          Conversations (2)
        </Text>
      </Box>

      {tab === "general" && settings && (
        <>
          {pickingModel ? (
            <Box flexDirection="column">
              <Text bold>Select model:</Text>
              {AVAILABLE_MODELS.map((m, idx) => {
                const isSelected = idx === modelIdx;
                const isCurrent = m === settings.model;
                return (
                  <Box key={m} gap={1}>
                    <Text color={isSelected ? "cyan" : undefined}>
                      {isSelected ? "\u25B8" : " "}
                    </Text>
                    <Text color={isSelected ? "cyan" : undefined}>
                      {m}
                    </Text>
                    {isCurrent && <Text dimColor>(current)</Text>}
                  </Box>
                );
              })}
            </Box>
          ) : (
            <Box gap={1}>
              <Text color={selectedIdx === 0 ? "cyan" : undefined}>
                {selectedIdx === 0 ? "\u25B8" : " "}
              </Text>
              <Text bold color={selectedIdx === 0 ? "cyan" : undefined}>model:</Text>
              <Text dimColor={settings.model === null}>
                {settings.model ?? "(not set)"}
              </Text>
            </Box>
          )}
        </>
      )}

      {tab === "conversations" && (
        <>
          {conversations.length === 0 ? (
            <Text dimColor>No conversations yet.</Text>
          ) : (
            conversations.map((conv, idx) => {
              const isSelected = idx === selectedIdx;
              return (
                <Box key={conv.id} gap={1}>
                  <Text color={isSelected ? "cyan" : undefined}>{isSelected ? "\u25B8" : " "}</Text>
                  <Text color={isSelected ? "cyan" : undefined}>
                    {conv.title ?? conv.id}
                  </Text>
                  {conv.title && <Text dimColor>({conv.id})</Text>}
                </Box>
              );
            })
          )}
        </>
      )}

      <Box marginTop={1}>
        <Text dimColor>
          {pickingModel
            ? "\u2191\u2193 = navigate \u00B7 Enter = select \u00B7 Esc = cancel"
            : "\u2190\u2192 = tabs \u00B7 \u2191\u2193 = navigate \u00B7 Enter = select \u00B7 Esc = close"}
        </Text>
      </Box>
    </Box>
  );
}
