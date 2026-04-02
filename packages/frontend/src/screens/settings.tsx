// ---------------------------------------------------------------------------
// screens/settings.tsx — Settings overlay (opened via Ctrl+S)
// ---------------------------------------------------------------------------

import React, { useEffect, useState } from "react";
import { Box, Text, useInput } from "ink";
import type { RpcClient } from "../rpc/index.ts";
import type { GetResult } from "../rpc/methods/settings.ts";
import type { ConversationSummary } from "../rpc/methods/conversation.ts";

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

  // Sync modelIdx to the currently-selected model when opening the picker
  function openModelPicker() {
    const current = settings?.model;
    const idx = current ? AVAILABLE_MODELS.indexOf(current as typeof AVAILABLE_MODELS[number]) : -1;
    setModelIdx(idx >= 0 ? idx : 0);
    setPickingModel(true);
  }

  useInput((input, key) => {
    // Model picker mode
    if (pickingModel) {
      if (key.upArrow || input === "k") {
        setModelIdx(i => Math.max(0, i - 1));
      } else if (key.downArrow || input === "j") {
        setModelIdx(i => Math.min(AVAILABLE_MODELS.length - 1, i + 1));
      } else if (key.return) {
        const model = AVAILABLE_MODELS[modelIdx]!;
        setPickingModel(false);
        client.call("settings.update", { model }).then(r => {
          if (r.ok) setSettings(r.value);
          else setError(r.error.message);
        });
      } else if (key.escape) {
        setPickingModel(false);
      }
      return;
    }

    // Tab switching
    if (key.ctrl && input === "1") { setTab("general"); setSelectedIdx(0); return; }
    if (key.ctrl && input === "2") { setTab("conversations"); setSelectedIdx(0); return; }
    // Also support 1/2 keys directly when not in a text field
    if (input === "1" && !key.ctrl && !key.meta) { setTab("general"); setSelectedIdx(0); return; }
    if (input === "2" && !key.ctrl && !key.meta) { setTab("conversations"); setSelectedIdx(0); return; }

    const items = tab === "general" ? ["model"] : conversations.map(c => c.id);
    const maxIdx = items.length - 1;

    // Navigation
    if (key.upArrow || input === "k") {
      setSelectedIdx(i => Math.max(0, i - 1));
    } else if (key.downArrow || input === "j") {
      setSelectedIdx(i => Math.min(maxIdx, i + 1));
    } else if (key.return || input === "e") {
      if (tab === "general") {
        openModelPicker();
      } else if (tab === "conversations") {
        const conv = conversations[selectedIdx];
        if (conv) {
          onSwitchConversation(conv.id);
          onClose();
        }
      }
    }
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
                      {isSelected ? "▸" : " "}
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
                {selectedIdx === 0 ? "▸" : " "}
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
                  <Text color={isSelected ? "cyan" : undefined}>{isSelected ? "▸" : " "}</Text>
                  <Text color={isSelected ? "cyan" : undefined}>{conv.id}</Text>
                </Box>
              );
            })
          )}
        </>
      )}

      <Box marginTop={1}>
        <Text dimColor>
          {pickingModel
            ? "↑↓/jk = navigate · Enter = select · Esc = cancel"
            : "↑↓/jk = navigate · Enter = select · Esc = close"}
        </Text>
      </Box>
    </Box>
  );
}
