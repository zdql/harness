// ---------------------------------------------------------------------------
// app.tsx — Root Ink application
//
// Full-screen layout:
//   ┌─ header ──────────────────────────────┐
//   │ message area              (flexGrow=1) │
//   │                                        │
//   │                                        │
//   ├─ prompt input (fixed height) ──────────┤
//   └─ status bar ───────────────────────────┘
//
// - Boots into a new conversation automatically
// - Cmd+S toggles settings overlay
// - Cmd+T creates a new conversation
// - Esc closes any overlay
// ---------------------------------------------------------------------------

import React, { useEffect, useState } from "react";
import { Box, Text, useApp, useInput, useStdout } from "ink";
import { RpcClient } from "./rpc/index.ts";
import { ConversationScreen } from "./screens/conversation.tsx";
import { SettingsScreen } from "./screens/settings.tsx";
import { PromptInput } from "./components/prompt-input.tsx";
import { matchShortcut } from "./shortcuts.ts";

interface Message {
  role: "user" | "assistant";
  content: string;
}

export function App() {
  const { exit } = useApp();
  const { stdout } = useStdout();
  const terminalHeight = stdout?.rows ?? 24;

  const [client] = useState(() => new RpcClient());
  const [conversationId, setConversationId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [showSettings, setShowSettings] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Create a new conversation on boot
  useEffect(() => {
    client
      .call("conversation.create", {})
      .then((r) => {
        setConversationId(r.id);
        setMessages([]);
      })
      .catch((e) => setError(String(e)));

    return () => client.close();
  }, [client]);

  // Global shortcuts (higher priority than PromptInput)
  useInput((input, key) => {
    const action = matchShortcut(input, key);
    if (!action) return;

    switch (action.type) {
      case "open_settings":
        setShowSettings((s) => !s);
        break;
      case "close_overlay":
        if (showSettings) setShowSettings(false);
        break;
      case "new_conversation":
        client
          .call("conversation.create", {})
          .then((r) => {
            setConversationId(r.id);
            setMessages([]);
            setShowSettings(false);
          })
          .catch((e) => setError(String(e)));
        break;
      case "quit":
        client.close();
        exit();
        break;
    }
  });

  function handleSubmit(text: string) {
    setMessages((msgs) => [...msgs, { role: "user", content: text }]);
    // TODO: send to agent, stream response back
    setMessages((msgs) => [
      ...msgs,
      { role: "assistant", content: "(echo) " + text },
    ]);
  }

  if (error) {
    return (
      <Box padding={1} height={terminalHeight}>
        <Text color="red">{error}</Text>
      </Box>
    );
  }

  return (
    <Box flexDirection="column" height={terminalHeight}>
      {/* Header */}
      <Box paddingX={1} justifyContent="space-between">
        <Box gap={1}>
          <Text bold color="green">
            harness
          </Text>
          {conversationId && (
            <Text dimColor>{conversationId}</Text>
          )}
        </Box>
        <Text dimColor>⌘S settings · ⌘T new · q quit</Text>
      </Box>

      {/* Message area — takes all remaining space */}
      {conversationId ? (
        <ConversationScreen
          conversationId={conversationId}
          messages={messages}
        />
      ) : (
        <Box flexGrow={1} justifyContent="center" alignItems="center">
          <Text dimColor>Starting...</Text>
        </Box>
      )}

      {/* Settings overlay — rendered over the message area */}
      {showSettings && (
        <SettingsScreen
          client={client}
          onClose={() => setShowSettings(false)}
          onSwitchConversation={(id) => {
            setConversationId(id);
            setMessages([]);
          }}
        />
      )}

      {/* Prompt input — fixed at the bottom */}
      <PromptInput isActive={!showSettings} onSubmit={handleSubmit} />

      {/* Status bar */}
      <Box paddingX={1}>
        <Text dimColor>
          {messages.length} message{messages.length !== 1 ? "s" : ""}
        </Text>
      </Box>
    </Box>
  );
}
