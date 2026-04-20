// ---------------------------------------------------------------------------
// ui/hud/ClockDisplay.tsx — wall clock chip.
// Manages its own 1-second polling interval.
// ---------------------------------------------------------------------------

import { Box, Text } from "ink";
import { useEffect, useState } from "react";

const CLOCK_REFRESH_MS = 1_000;

export function ClockDisplay(): React.JSX.Element {
  const [now, setNow] = useState<Date>(() => new Date());

  useEffect(() => {
    const t = setInterval(() => setNow(new Date()), CLOCK_REFRESH_MS);
    return () => clearInterval(t);
  }, []);

  const timeStr = now.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });

  return (
    <Box
      borderStyle="round"
      borderColor="yellow"
      paddingX={1}
      marginRight={1}
      flexShrink={1}
      overflowX="hidden"
    >
      <Text wrap="truncate-end">
        <Text color="gray" dimColor>
          TIME{" "}
        </Text>
        <Text color="yellow" bold>
          {timeStr}
        </Text>
      </Text>
    </Box>
  );
}
