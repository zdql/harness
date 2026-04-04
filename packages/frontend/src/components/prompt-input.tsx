// ---------------------------------------------------------------------------
// components/prompt-input.tsx — Text input fixed at the bottom of the screen
//
// Uses the custom keypress system (adapted from Gemini CLI) for reliable
// modifier detection via Kitty keyboard protocol. Supports multi-line editing:
//   Shift+Enter / Ctrl+Enter / Alt+Enter   Insert newline
//   Enter                                   Submit
//   Ctrl+U                                  Clear to start of line
//   Ctrl+K                                  Clear to end of line
//   Ctrl+W / Ctrl+Backspace / Alt+Backspace Delete word backward
//   Ctrl+Delete / Alt+Delete / Alt+D        Delete word forward
//   Backspace                               Delete character
// ---------------------------------------------------------------------------

import React, { useState, useCallback } from "react";
import { Box, Text } from "ink";
import {
  useKeypress,
  keyMatchers,
  Command,
  type Key,
} from "../input/index.ts";

interface Props {
  isActive: boolean;
  onSubmit: (text: string) => void;
}

/** Delete the last word from a string (whitespace-delimited). */
function deleteWordBackward(text: string): string {
  const trimmed = text.replace(/\s+$/, "");
  const lastSpace = trimmed.lastIndexOf(" ");
  if (lastSpace === -1) return "";
  return trimmed.slice(0, lastSpace + 1);
}

/** Delete the first word after cursor (simplified: operates on full buffer). */
function deleteWordForward(text: string): string {
  // For a simple end-of-line buffer, this is a no-op
  return text;
}

export function PromptInput({ isActive, onSubmit }: Props) {
  const [buffer, setBuffer] = useState("");
  const [cursorVisible, setCursorVisible] = useState(true);

  // Blink cursor
  React.useEffect(() => {
    const interval = setInterval(() => setCursorVisible((v) => !v), 530);
    return () => clearInterval(interval);
  }, []);

  const handler = useCallback(
    (key: Key): boolean | void => {
      // Newline (Shift+Enter, Ctrl+Enter, Alt+Enter, Ctrl+J)
      if (keyMatchers[Command.NEWLINE](key)) {
        setBuffer((b) => b + "\n");
        return true;
      }

      // Submit (Enter)
      if (keyMatchers[Command.SUBMIT](key)) {
        const text = buffer.trim();
        if (text) {
          onSubmit(text);
          setBuffer("");
        }
        return true;
      }

      // Kill line left (Ctrl+U)
      if (keyMatchers[Command.KILL_LINE_LEFT](key)) {
        setBuffer("");
        return true;
      }

      // Kill line right (Ctrl+K)
      if (keyMatchers[Command.KILL_LINE_RIGHT](key)) {
        setBuffer("");
        return true;
      }

      // Delete word backward (Ctrl+Backspace, Alt+Backspace, Ctrl+W)
      if (keyMatchers[Command.DELETE_WORD_BACKWARD](key)) {
        setBuffer((b) => deleteWordBackward(b));
        return true;
      }

      // Delete word forward (Ctrl+Delete, Alt+Delete, Alt+D)
      if (keyMatchers[Command.DELETE_WORD_FORWARD](key)) {
        setBuffer((b) => deleteWordForward(b));
        return true;
      }

      // Delete character left
      if (keyMatchers[Command.DELETE_CHAR_LEFT](key)) {
        setBuffer((b) => b.slice(0, -1));
        return true;
      }

      // Delete character right
      if (keyMatchers[Command.DELETE_CHAR_RIGHT](key)) {
        // no-op for end-of-buffer cursor
        return true;
      }

      // Paste event (from bracketed paste)
      if (key.name === "paste") {
        setBuffer((b) => b + key.sequence);
        return true;
      }

      // Ignore remaining control/escape sequences
      if (key.ctrl || key.cmd || key.name === "escape") return false;

      // Accept printable input
      if (key.insertable && key.sequence.length >= 1) {
        setBuffer((b) => b + key.sequence);
        return true;
      }

      return false;
    },
    [buffer, onSubmit],
  );

  useKeypress(handler, { isActive });

  const cursor = cursorVisible ? "\u2588" : " ";
  const lines = buffer.split("\n");
  const isMultiLine = lines.length > 1;

  return (
    <Box
      borderStyle="round"
      borderColor={isActive ? "green" : "gray"}
      paddingX={1}
      flexDirection="column"
    >
      <Box>
        <Text color="green" bold>
          {"\u276F "}
        </Text>
        <Box flexDirection="column">
          {lines.map((line, i) => (
            <Text key={i}>
              {line}
              {i === lines.length - 1 && isActive ? (
                <Text color="green">{cursor}</Text>
              ) : null}
            </Text>
          ))}
        </Box>
        {!buffer && isActive && (
          <Text dimColor>Type a message...</Text>
        )}
      </Box>
      {isMultiLine && (
        <Text dimColor>Enter to submit · Shift+Enter for newline</Text>
      )}
    </Box>
  );
}
