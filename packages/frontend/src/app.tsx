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
// ---------------------------------------------------------------------------

import React, { useEffect, useState, useCallback } from "react";
import { Box, Text, useApp, useStdout } from "ink";
import { RpcClient } from "./rpc/index.ts";
import { ConversationScreen } from "./screens/conversation.tsx";
import { SettingsScreen } from "./screens/settings.tsx";
import { PromptInput } from "./components/prompt-input.tsx";
import {
  KeypressProvider,
  KeypressPriority,
  useKeypress,
  keyMatchers,
  Command,
} from "./input/index.ts";

interface Message {
  role: "user" | "assistant" | "tool";
  content: string;
  toolName?: string;
  toolArgs?: string;
}

function AppInner() {
  const { exit } = useApp();
  const { stdout } = useStdout();
  const terminalHeight = stdout?.rows ?? 24;

  const [client] = useState(() => new RpcClient());
  const [conversationId, setConversationId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [showSettings, setShowSettings] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [scrollOffset, setScrollOffset] = useState(0);

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

    return () => {
      client.close();
    };
  }, [client]);

  // Auto-scroll to bottom on new messages or conversation change
  useEffect(() => {
    setScrollOffset(0);
  }, [messages.length, conversationId]);

  const promptFocused = !showSettings && !loading;

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

  // Global shortcuts (high priority — always active)
  const globalHandler = useCallback(
    (key: import("./input/index.ts").Key) => {
      if (keyMatchers[Command.TOGGLE_SETTINGS](key)) {
        setShowSettings((s) => !s);
        return true;
      }

      if (keyMatchers[Command.NEW_CONVERSATION](key)) {
        createNewConversation();
        return true;
      }

      if (keyMatchers[Command.ESCAPE](key)) {
        if (showSettings) {
          setShowSettings(false);
          return true;
        }
      }

      if (keyMatchers[Command.QUIT](key)) {
        client.close();
        exit();
        return true;
      }

      // Scroll via Shift+Up/Down
      if (keyMatchers[Command.SCROLL_UP](key)) {
        setScrollOffset((s) => s + 3);
        return true;
      }
      if (keyMatchers[Command.SCROLL_DOWN](key)) {
        setScrollOffset((s) => Math.max(0, s - 3));
        return true;
      }
      if (keyMatchers[Command.PAGE_UP](key)) {
        setScrollOffset((s) => s + (terminalHeight - 6));
        return true;
      }
      if (keyMatchers[Command.PAGE_DOWN](key)) {
        setScrollOffset((s) => Math.max(0, s - (terminalHeight - 6)));
        return true;
      }

      // q → quit (only when prompt not focused)
      if (key.name === "q" && !key.ctrl && !key.cmd && !key.alt && !promptFocused) {
        client.close();
        exit();
        return true;
      }

      return false;
    },
    [showSettings, promptFocused, client, exit, terminalHeight],
  );

  useKeypress(globalHandler, {
    isActive: true,
    priority: KeypressPriority.High,
  });

  function handleSubmit(text: string) {
    if (!conversationId || loading) return;

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
          {conversationId && <Text dimColor>{conversationId}</Text>}
        </Box>
        <Text dimColor>^S settings · ^N new · q quit</Text>
      </Box>

      {/* Message area */}
      {conversationId ? (
        <ConversationScreen
          conversationId={conversationId}
          messages={messages}
          availableHeight={terminalHeight - 6}
          scrollOffset={scrollOffset}
        />
      ) : (
        <Box flexGrow={1} justifyContent="center" alignItems="center">
          <Text dimColor>Starting...</Text>
        </Box>
      )}

      {/* Settings overlay */}
      {showSettings && (
        <SettingsScreen
          client={client}
          onClose={() => setShowSettings(false)}
          onSwitchConversation={(id) => {
            setConversationId(id);
            setMessages([]);
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

      {/* Prompt input */}
      <PromptInput isActive={!showSettings && !loading} onSubmit={handleSubmit} />

      {/* Status bar with shortcuts */}
      <Box paddingX={1} justifyContent="space-between">
        {loading ? (
          <Text color="yellow">Thinking...</Text>
        ) : (
          <Text dimColor>
            {messages.length} message{messages.length !== 1 ? "s" : ""}
          </Text>
        )}
        <Text dimColor>
          Shift+Enter newline · Opt+BS del word · ^U clear · /clear new chat
        </Text>
      </Box>
    </Box>
  );
}

export function App() {
  return (
    <KeypressProvider>
      <AppInner />
    </KeypressProvider>
  );
}
