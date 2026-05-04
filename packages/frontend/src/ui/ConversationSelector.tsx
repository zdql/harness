// ---------------------------------------------------------------------------
// ui/ConversationSelector.tsx
//
// Full-screen conversation selector overlay. Rendered by DefaultAppLayout when
// `overlayStore.get() === "conversations"`. Loads list of historical conversations
// via RPC on mount. User navigates with arrows, selects to switch active convo.
//
// UX:
//   ↑ / ↓   move between conversations
//   Enter   select and load this conversation
//   esc/q   cancel and close
//
// On select: Calls `conversation.switch(id)` to set the active conversation,
// then closes and adds a success message to history.
//
// Scroll management: the list only renders enough items to fill the terminal
// viewport. Scroll offset starts at 0 (top) and auto-adjusts as the cursor
// moves, keeping it inside the visible window.
// ---------------------------------------------------------------------------

import { Box, Text, useStdout } from "ink";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { RpcClient } from "../rpc/client.ts";
import { useKeypress, type Key } from "../keypress/useKeypress.ts";
import { overlayStore } from "../state/overlayStore.ts";
import { historyStore } from "../state/historyStore.ts";

interface Conversation {
  id: string;
  title?: string;
  updated_at: number;
  subagent_count: number;
}

interface Props {
  rpc: RpcClient;
}

/** Fixed chrome lines (header + footer + margin) reserved above the list. */
const CHROME_LINES = 6;
const SCROLL_THROTTLE_DELAY = 50; // ms

export function ConversationSelector({ rpc }: Props): React.JSX.Element {
  const [conversations, setConversations] = useState<Conversation[] | null>(
    null,
  );
  const [cursor, setCursor] = useState(0);
  const [scrollOffset, setScrollOffset] = useState(0);
  const [status, setStatus] = useState<string>("loading…");
  const lastScrollTime = useRef<number>(0);
  const scrollTimeoutRef = useRef<NodeJS.Timeout | null>(null);

  const { stdout } = useStdout();
  const termRows = stdout?.rows ?? 24;
  // Ensure we always have a reasonable visible count even if terminal size is problematic
  const visibleCount = Math.max(1, Math.min(termRows - CHROME_LINES, 50));

  // Reset scroll when conversations or terminal size changes significantly
  useEffect(() => {
    setScrollOffset(prev => {
      if (!conversations || conversations.length === 0) return 0;
      // Ensure scroll offset remains valid
      const maxOffset = Math.max(0, conversations.length - visibleCount);
      return Math.min(Math.max(0, prev), maxOffset);
    });
    
    setCursor(prev => {
      if (!conversations || conversations.length === 0) return 0;
      // Ensure cursor remains valid
      return Math.min(Math.max(0, prev), conversations.length - 1);
    });
  }, [conversations?.length, visibleCount]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (scrollTimeoutRef.current) {
        clearTimeout(scrollTimeoutRef.current);
      }
    };
  }, []);

  // Load conversations on mount.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const res = await rpc.call("conversation.list", {});
      if (cancelled) return;
      if (!res.ok) {
        setStatus(`failed to load conversations: ${res.error.message}`);
        setConversations([]);
        return;
      }
      setConversations(res.value.conversations || []);
      setStatus("");
    })();
    return () => {
      cancelled = true;
    };
  }, [rpc]);

  // Derive visible slice from scroll offset.
  const visibleSlice = useMemo(() => {
    if (!conversations || conversations.length === 0) return [];
    const safeVisibleCount = Math.max(1, visibleCount); // Ensure at least 1
    const maxOffset = Math.max(0, conversations.length - safeVisibleCount);
    const clamped = Math.min(Math.max(0, scrollOffset), maxOffset);
    // Ensure we don't try to slice beyond the array bounds
    const start = Math.max(0, Math.min(clamped, conversations.length - 1));
    const end = Math.min(start + safeVisibleCount, conversations.length);
    return conversations.slice(start, end);
  }, [conversations, scrollOffset, visibleCount]);

  /**
   * Adjust scrollOffset so that `cursor` is inside the visible window.
   * Called on every cursor change.
   */
  const clampScrollToCursor = useCallback(
    (nextCursor: number) => {
      if (!conversations || conversations.length === 0) return;
      
      // Cancel any pending scroll update
      if (scrollTimeoutRef.current) {
        clearTimeout(scrollTimeoutRef.current);
      }
      
      // Debounce scroll updates to prevent state queuing issues
      scrollTimeoutRef.current = setTimeout(() => {
        setScrollOffset((prev) => {
          // Ensure we don't exceed valid bounds
          const maxOffset = Math.max(0, conversations.length - visibleCount);
          
          // If cursor is above the window, scroll up to show it at the top.
          if (nextCursor < prev) {
            return Math.max(0, nextCursor);
          }
          
          // If cursor is at or past the bottom of the window, scroll down to show it at the bottom.
          if (nextCursor >= prev + visibleCount) {
            return Math.min(maxOffset, nextCursor - visibleCount + 1);
          }
          
          // Otherwise, maintain current scroll position within valid bounds
          return Math.min(Math.max(0, prev), maxOffset);
        });
      }, SCROLL_THROTTLE_DELAY);
    },
    [conversations, visibleCount],
  );

  const close = (): void => {
    overlayStore.set("none");
  };

  const select = async (id: string): Promise<void> => {
    setStatus("switching…");
    const switchRes = await rpc.call("conversation.switch", { id });
    if (!switchRes.ok) {
      setStatus(`failed to switch: ${switchRes.error.message}`);
      return;
    }

    // Load the new conversation's messages.
    const getRes = await rpc.call("conversation.get", { id });
    if (!getRes.ok) {
      setStatus(`failed to load conversation: ${getRes.error.message}`);
      return;
    }

    // Clear existing history and load the new conversation's messages.
    historyStore.clear();
    
    // Add messages from the loaded conversation to history.
    for (const msg of getRes.value.messages) {
      switch (msg.role) {
        case "user":
          historyStore.addItem({ type: "user", text: msg.content });
          break;
        case "assistant":
          historyStore.addItem({ type: "assistant", text: msg.content });
          break;
        case "tool":
          if (msg.tool_name) {
            historyStore.addItem({
              type: "tool",
              name: msg.tool_name,
              result: msg.content,
            });
          }
          break;
      }
    }

    // Add a system info message confirming the switch.
    historyStore.addItem({
      type: "info",
      text: `Switched to conversation ${switchRes.value.id}.`,
    });

    close();
  };

  useKeypress(
    (key: Key): boolean | void => {
      // Esc/Q always works.
      if (key.name === "escape" || (key.name === "q" && !key.ctrl)) {
        close();
        return true;
      }
      if (!conversations) return;

      if (key.name === "up" || key.name === "k") {
        setCursor((c) => {
          if (!conversations || conversations.length === 0) return 0;
          
          // Cancel any pending cursor update
          if (scrollTimeoutRef.current) {
            clearTimeout(scrollTimeoutRef.current);
          }
          
          const next = c - 1 < 0 ? conversations.length - 1 : c - 1;
          clampScrollToCursor(next);
          return next;
        });
        return true;
      }
      if (key.name === "down" || key.name === "j") {
        setCursor((c) => {
          if (!conversations || conversations.length === 0) return 0;
          
          // Cancel any pending cursor update
          if (scrollTimeoutRef.current) {
            clearTimeout(scrollTimeoutRef.current);
          }
          
          const next = c + 1 >= conversations.length ? 0 : c + 1;
          clampScrollToCursor(next);
          return next;
        });
        return true;
      }
      if (
        (key.name === "enter" || key.name === "space") &&
        conversations && 
        conversations.length > 0 &&
        cursor >= 0 && 
        cursor < conversations.length
      ) {
        void select(conversations[cursor]?.id || '');
        return true;
      }
      return;
    },
    { isActive: true, priority: true },
  );

  const noConvos = conversations?.length === 0;
  const showingSlice =
    conversations !== null &&
    conversations.length > 0 &&
    conversations.length > visibleCount;

  return (
    <Box flexDirection="column" paddingX={1} paddingY={0}>
      <Box marginBottom={1}>
        <Text bold color="cyan">
          Conversations
        </Text>
        <Text dimColor>  ·  select to switch</Text>
      </Box>

      {conversations !== null ? (
        <Box flexDirection="column">
          {noConvos ? (
            <Text dimColor>No conversations yet. Start one to begin!</Text>
          ) : (
            visibleSlice.map((conv, sliceIdx) => {
              // Recover the global index so active highlight matches cursor.
              const offset = Math.min(
                Math.max(0, scrollOffset),
                Math.max(0, conversations!.length - visibleCount),
              );
              const globalIdx = offset + sliceIdx;
              const active = globalIdx === cursor;
              const title = conv.title || "(untitled)";
              const date = new Date(conv.updated_at * 1000).toLocaleString();
              const subLabel =
                conv.subagent_count > 0
                  ? `  ⚙ ${conv.subagent_count} subagent${conv.subagent_count !== 1 ? "s" : ""}`
                  : "";
              return (
                <Box key={conv.id}>
                  <Text color={active ? "cyan" : undefined}>
                    {active ? "▶ " : "  "}
                  </Text>
                  <Text
                    bold={active}
                    color={active ? "cyan" : "white"}
                  >
                    {title}
                  </Text>
                  <Text dimColor>
                    {"  "}
                    {conv.id.slice(-8)}
                    {"  "}
                    {date}
                    {subLabel}
                  </Text>
                </Box>
              );
            })
          )}
        </Box>
      ) : (
        <Text dimColor>{status || "loading…"}</Text>
      )}

      {status && conversations ? (
        <Box marginTop={1}>
          <Text color="yellow">{status}</Text>
        </Box>
      ) : null}

      <Box marginTop={1}>
        <Text dimColor>
          {showingSlice && conversations
            ? `↑/↓ move   Enter select   esc cancel   (${(cursor >= 0 && conversations ? Math.min(cursor + 1, conversations.length) : 1)}/${conversations!.length})`
            : "↑/↓ move   Enter select   esc cancel"}
        </Text>
      </Box>
    </Box>
  );
}
