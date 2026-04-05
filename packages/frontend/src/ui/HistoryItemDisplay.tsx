// ---------------------------------------------------------------------------
// ui/HistoryItemDisplay.tsx
//
// Dispatches on item.type → renders a styled Box for that message kind.
// Slimmed port of gemini-cli's HistoryItemDisplay.tsx — no markdown, no
// tool confirmations, no inline expansion. Extend as the agent grows.
// ---------------------------------------------------------------------------

import { Box, Text } from "ink";
import { useEffect, useState } from "react";
import type { HistoryItem } from "./types.ts";

interface Props {
  item: HistoryItem;
}

// Braille spinner frames — same set gemini-cli / ora use. Kept local so we
// don't need to pull in ink-spinner just for this.
const SPINNER_FRAMES = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const SPINNER_INTERVAL_MS = 80;

function Spinner(): React.JSX.Element {
  const [frame, setFrame] = useState(0);
  useEffect(() => {
    const t = setInterval(
      () => setFrame((f) => (f + 1) % SPINNER_FRAMES.length),
      SPINNER_INTERVAL_MS,
    );
    return () => clearInterval(t);
  }, []);
  return <Text color="yellow">{SPINNER_FRAMES[frame]}</Text>;
}

export function HistoryItemDisplay({ item }: Props): React.JSX.Element {
  switch (item.type) {
    case "user":
      return (
        <Box flexDirection="column" marginTop={1}>
          <Text color="cyan" bold>
            {"> "}
            <Text color="cyanBright">{item.text}</Text>
          </Text>
        </Box>
      );

    case "assistant":
      return (
        <Box flexDirection="column" marginTop={1}>
          <Text color="white">{item.text}</Text>
        </Box>
      );

    case "tool": {
      const MAX_PREVIEW_CHARS = 120;
      // Collapse newlines so a multi-line result renders as a single line,
      // then hard-cap by character count to keep long one-liners (JSON
      // blobs, file dumps) from blowing up the scrollback.
      const oneLine = item.result.replace(/\s+/g, " ").trim();
      const preview = oneLine.slice(0, MAX_PREVIEW_CHARS);
      const truncated = oneLine.length > MAX_PREVIEW_CHARS;
      return (
        <Box flexDirection="column" marginTop={1} marginLeft={2}>
          <Text color="magenta" bold>
            ⚙ {item.name}
          </Text>
          <Text color="gray">
            {preview}
            {truncated ? "…" : ""}
          </Text>
        </Box>
      );
    }

    case "tool-running":
      return (
        <Box flexDirection="column" marginTop={1} marginLeft={2}>
          <Text color="magenta" bold>
            <Spinner /> <Text color="magenta">⚙ {item.name}</Text>
            <Text color="gray"> (running…)</Text>
          </Text>
        </Box>
      );

    case "thinking":
      return (
        <Box marginTop={1}>
          <Text>
            <Spinner /> <Text color="gray" italic>{item.label}</Text>
          </Text>
        </Box>
      );

    case "error":
      return (
        <Box flexDirection="column" marginTop={1}>
          <Text color="red" bold>
            ✗ {item.message}
          </Text>
        </Box>
      );

    case "info":
      return (
        <Box flexDirection="column" marginTop={1}>
          <Text color="gray" italic>
            {item.text}
          </Text>
        </Box>
      );
  }
}
