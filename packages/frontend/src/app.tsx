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
// - Ctrl+S toggles settings overlay
// - Ctrl+N creates a new conversation
// - Esc closes any overlay
// ---------------------------------------------------------------------------

import React, { useEffect, useState } from "react";
import { Box, Text, useApp, useInput, useStdout } from "ink";
import { RpcClient } from "./rpc/index.ts";
import { ConversationScreen } from "./screens/conversation.tsx";
import { SettingsScreen } from "./screens/settings.tsx";
import { PromptInput } from "./components/prompt-input.tsx";
import { matchShortcut } from "./shortcuts.ts";
import { isMouseSequence } from "./hooks/mouse-filter.ts";

interface Message {
  role: "user" | "assistant" | "tool";
  content: string;
  toolName?: string;
  toolArgs?: string;
}

export function App() {
  const { exit } = useApp();
  const { stdout } = useStdout();
  const terminalHeight = stdout?.rows ?? 24;

  const [client] = useState(() => new RpcClient());
  const [conversationId, setConversationId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [showSettings, setShowSettings] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Create a new conversation on boot
  useEffect(() => {
    client.call("conversation.create", {}).then((r) => {
      if (r.ok) {
        setConversationId(r.value.id);
        setMessages([]);
      } else {
        setError(r.error.message);
      }
    });

    return () => client.close();
  }, [client]);

  const promptFocused = !showSettings && !loading;

  // Global shortcuts
  useInput((input, key) => {
    if (isMouseSequence(input)) return;

    const action = matchShortcut(input, key, { promptFocused });
    if (!action) return;

    switch (action.type) {
      case "open_settings":
        setShowSettings((s) => !s);
        break;
      case "close_overlay":
        if (showSettings) setShowSettings(false);
        break;
      case "new_conversation":
        createNewConversation();
        break;
      case "quit":
        client.close();
        exit();
        break;
    }
  });

  function createNewConversation() {
    client.call("conversation.create", {}).then((r) => {
      if (r.ok) {
        setConversationId(r.value.id);
        setMessages([]);
        setShowSettings(false);
      } else {
        setError(r.error.message);
      }
    });
  }

  function handleSubmit(text: string) {
    if (!conversationId || loading) return;

    // Handle slash commands
    if (text === "/clear") {
      createNewConversation();
      return;
    }

    setMessages((msgs) => [...msgs, { role: "user", content: text }]);
    setLoading(true);

    client
      .call("conversation.send", { id: conversationId, message: text })
      .then((r) => {
        if (r.ok) {
          const newMessages: Message[] = [
            { role: "assistant", content: r.value.reply },
          ];
          // Prepend tool call messages if any
          if (r.value.tool_calls && r.value.tool_calls.length > 0) {
            for (const tc of r.value.tool_calls) {
              newMessages.unshift({
                role: "tool",
                content: tc.result,
                toolName: tc.name,
                toolArgs: tc.arguments,
              });
            }
          }
          setMessages((msgs) => [...msgs, ...newMessages]);
        } else {
          setMessages((msgs) => [
            ...msgs,
            { role: "assistant", content: `Error: ${r.error.message}` },
          ]);
        }
        setLoading(false);
      });
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
        <Text dimColor>^S settings · ^N new · q quit</Text>
      </Box>

      {/* Message area — takes all remaining space */}
      {conversationId ? (
        <ConversationScreen
          conversationId={conversationId}
          messages={messages}
          availableHeight={terminalHeight - 5}
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
            // Load existing messages from the conversation
            client.call("conversation.get", { id }).then((r) => {
              if (r.ok) {
                setMessages(
                  r.value.messages.map((m) => ({
                    role: m.role as Message["role"],
                    content: m.content,
                    toolName: m.tool_name,
                    toolArgs: m.tool_args,
                  }))
                );
              }
            });
          }}
        />
      )}

      {/* Prompt input — fixed at the bottom */}
      <PromptInput isActive={!showSettings && !loading} onSubmit={handleSubmit} />

      {/* Status bar */}
      <Box paddingX={1}>
        {loading ? (
          <Text color="yellow">Thinking...</Text>
        ) : (
          <Text dimColor>
            {messages.length} message{messages.length !== 1 ? "s" : ""}
          </Text>
        )}
      </Box>
    </Box>
  );
}
