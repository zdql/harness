// ---------------------------------------------------------------------------
// components/prompt-input.tsx — Text input fixed at the bottom of the screen
//
// Handles character input, backspace, submit, line-editing shortcuts, and
// paste. Mouse escape sequence fragments are filtered out.
// ---------------------------------------------------------------------------

import React, { useState } from "react";
import { Box, Text, useInput } from "ink";
import { isMouseSequence } from "../hooks/mouse-filter.ts";
import { matchEditShortcut, deleteWord } from "../hooks/line-editing.ts";

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

      // Line-editing shortcuts (Ctrl+U, Ctrl+W, Option+Backspace, etc.)
      const edit = matchEditShortcut(input, key);
      if (edit) {
        switch (edit.type) {
          case "clear_line":
            setBuffer("");
            break;
          case "delete_word":
            setBuffer((b) => deleteWord(b));
            break;
        }
        return;
      }

      if (key.backspace || key.delete) {
        setBuffer((b) => b.slice(0, -1));
        return;
      }

      // Ignore remaining control sequences
      if (key.ctrl || key.escape) return;

      if (input) {
        // Drop mouse escape sequence fragments
        if (isMouseSequence(input)) return;

        // Accept single characters (typing) and multi-character strings
        // (paste from terminal). Filter non-printable chars from paste.
        const printable = input.replace(/[\x00-\x1F\x7F]/g, "");
        if (printable) {
          setBuffer((b) => b + printable);
        }
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
