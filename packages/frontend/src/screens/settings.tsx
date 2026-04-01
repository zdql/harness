// ---------------------------------------------------------------------------
// screens/settings.tsx — Settings overlay (opened via Cmd+S)
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

const SETTING_FIELDS = ["model"] as const;

export function SettingsScreen({ client, onClose, onSwitchConversation }: Props) {
  const [tab, setTab] = useState<Tab>("general");
  const [settings, setSettings] = useState<GetResult | null>(null);
  const [conversations, setConversations] = useState<ConversationSummary[]>([]);
  const [selectedIdx, setSelectedIdx] = useState(0);
  const [editing, setEditing] = useState(false);
  const [editBuffer, setEditBuffer] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    client.call("settings.get", {}).then(setSettings).catch(e => setError(String(e)));
    client.call("conversation.list", {}).then(r => setConversations(r.conversations)).catch(e => setError(String(e)));
  }, []);

  const items = tab === "general" ? SETTING_FIELDS : conversations.map(c => c.id);
  const maxIdx = items.length - 1;

  useInput((input, key) => {
    // Editing mode
    if (editing) {
      if (key.return) {
        const field = SETTING_FIELDS[selectedIdx]!;
        const value = editBuffer.trim() || undefined;
        setEditing(false);
        client
          .call("settings.update", { [field]: value })
          .then(setSettings)
          .catch(e => setError(String(e)));
      } else if (key.escape) {
        setEditing(false);
      } else if (key.backspace || key.delete) {
        setEditBuffer(b => b.slice(0, -1));
      } else if (input && !key.ctrl && !key.meta) {
        setEditBuffer(b => b + input);
      }
      return;
    }

    // Tab switching
    if (key.meta && input === "1") { setTab("general"); setSelectedIdx(0); return; }
    if (key.meta && input === "2") { setTab("conversations"); setSelectedIdx(0); return; }

    // Navigation
    if (key.upArrow || input === "k") {
      setSelectedIdx(i => Math.max(0, i - 1));
    } else if (key.downArrow || input === "j") {
      setSelectedIdx(i => Math.min(maxIdx, i + 1));
    } else if (key.return || input === "e") {
      if (tab === "general") {
        const field = SETTING_FIELDS[selectedIdx]!;
        setEditBuffer(settings?.[field] ?? "");
        setEditing(true);
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
          General (⌘1)
        </Text>
        <Text bold underline={tab === "conversations"} color={tab === "conversations" ? "cyan" : undefined}>
          Conversations (⌘2)
        </Text>
      </Box>

      {tab === "general" && settings && (
        <>
          {SETTING_FIELDS.map((field, idx) => {
            const isSelected = idx === selectedIdx;
            const value = settings[field];
            return (
              <Box key={field} gap={1}>
                <Text color={isSelected ? "cyan" : undefined}>{isSelected ? "▸" : " "}</Text>
                <Text bold color={isSelected ? "cyan" : undefined}>{field}:</Text>
                {editing && isSelected ? (
                  <Text color="yellow">{editBuffer}█</Text>
                ) : (
                  <Text dimColor={value === null}>{value ?? "(not set)"}</Text>
                )}
              </Box>
            );
          })}
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
          {editing
            ? "Enter = save · Esc = cancel"
            : "↑↓/jk = navigate · Enter = select · Esc = close"}
        </Text>
      </Box>
    </Box>
  );
}
