// ---------------------------------------------------------------------------
// screens/conversation.tsx — Active conversation view
//
// Takes up all available vertical space (flexGrow={1}). Messages fill from
// the top, empty space below. The prompt input is rendered by the parent.
// ---------------------------------------------------------------------------

import React from "react";
import { Box, Text } from "ink";

interface Message {
  role: "user" | "assistant";
  content: string;
}

interface Props {
  conversationId: string;
  messages: Message[];
}

export function ConversationScreen({ conversationId, messages }: Props) {
  return (
    <Box flexDirection="column" flexGrow={1} overflow="hidden" paddingX={1}>
      {messages.length === 0 ? (
        <Box flexGrow={1} justifyContent="center" alignItems="center">
          <Text dimColor>New conversation. Type a message below to begin.</Text>
        </Box>
      ) : (
        messages.map((msg, idx) => (
          <Box key={idx} marginBottom={1} flexDirection="column">
            <Text bold color={msg.role === "user" ? "blue" : "green"}>
              {msg.role === "user" ? "You" : "Assistant"}
            </Text>
            <Text>{msg.content}</Text>
          </Box>
        ))
      )}
    </Box>
  );
}
