// ---------------------------------------------------------------------------
// screens/conversation.tsx — Active conversation view with scroll support
//
// Renders messages within a fixed-height viewport. Uses line estimates
// to determine which messages are visible given the current scrollOffset
// (managed by the parent via mouse wheel events).
// ---------------------------------------------------------------------------

import React from "react";
import { Box, Text, useStdout } from "ink";
import { ToolCallDisplay } from "../components/tools/index.tsx";

interface Message {
  role: "user" | "assistant" | "tool";
  content: string;
  toolName?: string;
  toolArgs?: string;
}

interface Props {
  conversationId: string;
  messages: Message[];
  availableHeight: number;
  scrollOffset: number;
}

function estimateLines(msg: Message, columns: number): number {
  const usable = Math.max(columns - 2, 20);

  if (msg.role === "tool") {
    const headerLen =
      (msg.toolName ?? "tool").length +
      2 +
      (msg.toolArgs ? msg.toolArgs.length + 3 : 0);
    const headerLines = Math.ceil(headerLen / usable);
    const resultLines = Math.ceil((msg.content.length + 2) / usable) || 1;
    return headerLines + resultLines + 1;
  }

  const headerLines = 1;
  const contentLines = msg.content.split("\n").reduce((acc, line) => {
    return acc + Math.max(1, Math.ceil(line.length / usable));
  }, 0);
  return headerLines + contentLines + 1;
}

export function ConversationScreen({
  conversationId,
  messages,
  availableHeight,
  scrollOffset,
}: Props) {
  const { stdout } = useStdout();
  const columns = stdout?.columns ?? 80;

  if (messages.length === 0) {
    return (
      <Box
        flexDirection="column"
        flexGrow={1}
        paddingX={1}
        justifyContent="center"
        alignItems="center"
      >
        <Text dimColor>
          New conversation. Type a message below to begin.
        </Text>
      </Box>
    );
  }

  const lineEstimates = messages.map((m) => estimateLines(m, columns));
  const totalLines = lineEstimates.reduce((a, b) => a + b, 0);
  const maxOffset = Math.max(0, totalLines - availableHeight);
  const clampedOffset = Math.min(scrollOffset, maxOffset);

  const viewStart = Math.max(0, totalLines - availableHeight - clampedOffset);
  const viewEnd = viewStart + availableHeight;

  let linePos = 0;
  const visible: { msg: Message; idx: number }[] = [];

  for (let i = 0; i < messages.length; i++) {
    const msgHeight = lineEstimates[i]!;
    const msgEnd = linePos + msgHeight;

    if (msgEnd > viewStart && linePos < viewEnd) {
      visible.push({ msg: messages[i]!, idx: i });
    }

    linePos += msgHeight;
  }

  const truncatedAbove = visible.length > 0 && visible[0]!.idx > 0;
  const scrolledUp = clampedOffset > 0;

  return (
    <Box
      flexDirection="column"
      flexGrow={1}
      overflow="hidden"
      paddingX={1}
      height={availableHeight}
    >
      {truncatedAbove && (
        <Text dimColor>
          — {visible[0]!.idx} earlier message
          {visible[0]!.idx !== 1 ? "s" : ""} (scroll up for more) —
        </Text>
      )}
      {visible.map(({ msg, idx }) => (
        <Box key={idx} marginBottom={1} flexDirection="column">
          {msg.role === "tool" ? (
            <ToolCallDisplay
              toolName={msg.toolName ?? "tool"}
              toolArgs={msg.toolArgs}
              content={msg.content}
            />
          ) : (
            <>
              <Text bold color={msg.role === "user" ? "blue" : "green"}>
                {msg.role === "user" ? "You" : "Assistant"}
              </Text>
              <Text>{msg.content}</Text>
            </>
          )}
        </Box>
      ))}
      {scrolledUp && (
        <Text dimColor>— scroll down for latest —</Text>
      )}
    </Box>
  );
}
