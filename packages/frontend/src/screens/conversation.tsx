// ---------------------------------------------------------------------------
// screens/conversation.tsx — Active conversation view with mouse scrolling
//
// Renders messages within a fixed-height viewport. Computes line estimates
// for each message, then uses a scrollOffset (lines from bottom) to decide
// which slice of messages to display. Mouse wheel events adjust the offset.
// New messages auto-scroll to the bottom (offset resets to 0).
// ---------------------------------------------------------------------------

import React, { useState, useEffect, useCallback } from "react";
import { Box, Text, useStdout } from "ink";
import { useMouseScroll } from "../hooks/use-mouse-scroll.ts";
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
}

const SCROLL_SPEED = 3; // lines per scroll tick

/**
 * Estimate how many terminal rows a message will occupy.
 * Accounts for the role header line, content wrapping, and bottom margin.
 */
function estimateLines(msg: Message, columns: number): number {
  const usable = Math.max(columns - 2, 20); // paddingX={1} takes 2 columns

  if (msg.role === "tool") {
    const headerLen =
      (msg.toolName ?? "tool").length +
      2 +
      (msg.toolArgs ? msg.toolArgs.length + 3 : 0);
    const headerLines = Math.ceil(headerLen / usable);
    const resultLines = Math.ceil((msg.content.length + 2) / usable) || 1;
    return headerLines + resultLines + 1; // +1 for marginBottom
  }

  const headerLines = 1;
  const contentLines = msg.content.split("\n").reduce((acc, line) => {
    return acc + Math.max(1, Math.ceil(line.length / usable));
  }, 0);
  return headerLines + contentLines + 1; // +1 for marginBottom
}

export function ConversationScreen({
  conversationId,
  messages,
  availableHeight,
}: Props) {
  const { stdout } = useStdout();
  const columns = stdout?.columns ?? 80;

  // scrollOffset = how many lines we've scrolled UP from the bottom.
  // 0 means pinned to the latest messages.
  const [scrollOffset, setScrollOffset] = useState(0);

  // Compute total content height
  const lineEstimates = messages.map((m) => estimateLines(m, columns));
  const totalLines = lineEstimates.reduce((a, b) => a + b, 0);
  const maxOffset = Math.max(0, totalLines - availableHeight);

  // Auto-scroll to bottom when new messages arrive or conversation changes
  useEffect(() => {
    setScrollOffset(0);
  }, [messages.length, conversationId]);

  const handleScroll = useCallback(
    (direction: "up" | "down") => {
      setScrollOffset((prev) => {
        if (direction === "up") {
          return Math.min(maxOffset, prev + SCROLL_SPEED);
        } else {
          return Math.max(0, prev - SCROLL_SPEED);
        }
      });
    },
    [maxOffset]
  );

  useMouseScroll({ onScroll: handleScroll, isActive: messages.length > 0 });

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

  // The viewport shows lines [viewStart .. viewStart + availableHeight)
  // where viewStart is measured from the top of all content.
  // scrollOffset=0 means we're at the bottom, so:
  const viewStart = Math.max(0, totalLines - availableHeight - scrollOffset);
  const viewEnd = viewStart + availableHeight;

  // Walk messages and figure out which are visible (even partially)
  let linePos = 0;
  const visible: { msg: Message; idx: number }[] = [];
  let linesBeforeVisible = 0;

  for (let i = 0; i < messages.length; i++) {
    const msgHeight = lineEstimates[i]!;
    const msgEnd = linePos + msgHeight;

    if (msgEnd > viewStart && linePos < viewEnd) {
      visible.push({ msg: messages[i]!, idx: i });
    } else if (msgEnd <= viewStart) {
      linesBeforeVisible = i + 1;
    }

    linePos += msgHeight;
  }

  const truncatedAbove = visible.length > 0 && visible[0]!.idx > 0;
  const scrolledUp = scrollOffset > 0;

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
