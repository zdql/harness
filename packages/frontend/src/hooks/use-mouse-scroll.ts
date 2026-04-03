// ---------------------------------------------------------------------------
// hooks/use-mouse-scroll.ts — Mouse wheel scrolling for Ink
//
// Ink has no built-in mouse support. This hook enables SGR mouse reporting
// on the terminal, listens for scroll wheel events on raw stdin, and calls
// the provided onScroll callback with "up" or "down".
//
// The hook automatically enables mouse reporting on mount and disables it
// on unmount so the terminal is left clean.
// ---------------------------------------------------------------------------

import { useEffect } from "react";
import { useStdin } from "ink";

type ScrollDirection = "up" | "down";

interface Options {
  /** Called on each scroll tick. */
  onScroll: (direction: ScrollDirection) => void;
  /** Set to false to temporarily disable. Default: true. */
  isActive?: boolean;
}

// SGR mouse protocol escape sequences
const ENABLE_MOUSE = "\x1b[?1000h\x1b[?1006h"; // basic tracking + SGR extended
const DISABLE_MOUSE = "\x1b[?1000l\x1b[?1006l";

// SGR mouse report format: ESC [ < button ; x ; y M (press) or m (release)
// Scroll up:   button = 64
// Scroll down:  button = 65
const SGR_MOUSE_RE = /\x1b\[<(\d+);\d+;\d+[Mm]/g;

export function useMouseScroll({ onScroll, isActive = true }: Options) {
  const { stdin } = useStdin();

  useEffect(() => {
    if (!isActive) return;

    // Enable mouse reporting
    process.stdout.write(ENABLE_MOUSE);

    const handler = (data: Buffer) => {
      const str = data.toString("utf-8");
      let match: RegExpExecArray | null;
      SGR_MOUSE_RE.lastIndex = 0;
      while ((match = SGR_MOUSE_RE.exec(str)) !== null) {
        const button = parseInt(match[1]!, 10);
        if (button === 64) onScroll("up");
        else if (button === 65) onScroll("down");
      }
    };

    stdin.on("data", handler);

    return () => {
      stdin.removeListener("data", handler);
      process.stdout.write(DISABLE_MOUSE);
    };
  }, [stdin, isActive, onScroll]);
}
