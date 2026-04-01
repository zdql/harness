// ---------------------------------------------------------------------------
// components/prompt-input.tsx — Text input fixed at the bottom of the screen
//
// Handles character input, backspace, and submit (Enter).
// The parent controls focus via `isActive`.
// ---------------------------------------------------------------------------

import React, { useState } from "react";
import { Box, Text, useInput } from "ink";

interface Props {
  isActive: boolean;
  onSubmit: (text: string) => void;
}

export function PromptInput({ isActive, onSubmit }: Props) {
  const [buffer, setBuffer] = useState("");
  const [cursorVisible, setCursorVisible] = useState(true);

  // Blink cursor
  React.useEffect(() => {
    const interval = setInterval(() => setCursorVisible((v) => !v), 530);
    return () => clearInterval(interval);
  }, []);

  useInput(
    (input, key) => {
      if (key.return) {
        const text = buffer.trim();
        if (text) {
          onSubmit(text);
          setBuffer("");
        }
        return;
      }

      if (key.backspace || key.delete) {
        setBuffer((b) => b.slice(0, -1));
        return;
      }

      // Ignore control sequences
      if (key.ctrl || key.meta || key.escape) return;

      if (input) {
        setBuffer((b) => b + input);
      }
    },
    { isActive }
  );

  const cursor = cursorVisible ? "█" : " ";

  return (
    <Box
      borderStyle="round"
      borderColor={isActive ? "green" : "gray"}
      paddingX={1}
    >
      <Text color="green" bold>
        {"❯ "}
      </Text>
      <Text>
        {buffer}
        {isActive ? <Text color="green">{cursor}</Text> : null}
      </Text>
      {!buffer && isActive && (
        <Text dimColor>Type a message...</Text>
      )}
    </Box>
  );
}
