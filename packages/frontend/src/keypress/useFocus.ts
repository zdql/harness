// ---------------------------------------------------------------------------
// keypress/useFocus.ts
// Owns the FOCUS_IN / FOCUS_OUT constants that KeypressContext imports.
// Also exposes a useFocus() hook for components that care about terminal
// focus events (e.g., to pause animations when the window loses focus).
//
// Copied verbatim from gemini-cli (ui/hooks/useFocus.ts).
// ---------------------------------------------------------------------------

import { useStdin, useStdout } from "ink";
import { useEffect, useState } from "react";
import { useKeypress } from "./useKeypress.ts";

export const ENABLE_FOCUS_REPORTING = "\x1b[?1004h";
export const DISABLE_FOCUS_REPORTING = "\x1b[?1004l";

export const FOCUS_IN = "\x1b[I";
export const FOCUS_OUT = "\x1b[O";

export const useFocus = (): {
  isFocused: boolean;
  hasReceivedFocusEvent: boolean;
} => {
  const { stdin } = useStdin();
  const { stdout } = useStdout();
  const [isFocused, setIsFocused] = useState(true);
  const [hasReceivedFocusEvent, setHasReceivedFocusEvent] = useState(false);

  useEffect(() => {
    const handleData = (data: Buffer): void => {
      const sequence = data.toString();
      const lastFocusIn = sequence.lastIndexOf(FOCUS_IN);
      const lastFocusOut = sequence.lastIndexOf(FOCUS_OUT);

      if (lastFocusIn > lastFocusOut) {
        setHasReceivedFocusEvent(true);
        setIsFocused(true);
      } else if (lastFocusOut > lastFocusIn) {
        setHasReceivedFocusEvent(true);
        setIsFocused(false);
      }
    };

    stdout?.write(ENABLE_FOCUS_REPORTING);
    stdin?.on("data", handleData);

    return () => {
      stdout?.write(DISABLE_FOCUS_REPORTING);
      stdin?.removeListener("data", handleData);
    };
  }, [stdin, stdout]);

  useKeypress(
    () => {
      // Any keystroke means we can't be focused-out (tmux workaround).
      if (!isFocused) setIsFocused(true);
    },
    { isActive: true },
  );

  return { isFocused, hasReceivedFocusEvent };
};
