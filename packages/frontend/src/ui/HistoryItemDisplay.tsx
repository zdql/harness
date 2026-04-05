// ---------------------------------------------------------------------------
// ui/HistoryItemDisplay.tsx
//
// Dispatches on item.type → renders a styled Box for that message kind.
// Slimmed port of gemini-cli's HistoryItemDisplay.tsx — no markdown, no
// tool confirmations, no inline expansion. Extend as the agent grows.
// ---------------------------------------------------------------------------

import { Box, Text } from "ink";
import type { HistoryItem } from "./types.ts";

interface Props {
  item: HistoryItem;
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

    case "tool":
      return (
        <Box flexDirection="column" marginTop={1} marginLeft={2}>
          <Text color="magenta" bold>
            ⚙ {item.name}
          </Text>
          <Text color="gray">{item.result}</Text>
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
