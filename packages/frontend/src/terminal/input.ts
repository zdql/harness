// ---------------------------------------------------------------------------
// terminal/input.ts — ESC constant + mouse-sequence regexes
// Copied verbatim from gemini-cli (ui/utils/input.ts). No external deps.
// ---------------------------------------------------------------------------

export const ESC = "\u001B";
export const SGR_EVENT_PREFIX = `${ESC}[<`;
export const X11_EVENT_PREFIX = `${ESC}[M`;

// eslint-disable-next-line no-control-regex
export const SGR_MOUSE_REGEX = /^\x1b\[<(\d+);(\d+);(\d+)([mM])/;
// X11 is ESC [ M followed by 3 bytes.
// eslint-disable-next-line no-control-regex
export const X11_MOUSE_REGEX = /^\x1b\[M([\s\S]{3})/;

export function couldBeSGRMouseSequence(buffer: string): boolean {
  if (buffer.length === 0) return true;
  if (SGR_EVENT_PREFIX.startsWith(buffer)) return true;
  if (buffer.startsWith(SGR_EVENT_PREFIX)) return true;
  return false;
}

export function couldBeMouseSequence(buffer: string): boolean {
  if (buffer.length === 0) return true;
  if (
    SGR_EVENT_PREFIX.startsWith(buffer) ||
    buffer.startsWith(SGR_EVENT_PREFIX)
  )
    return true;
  if (
    X11_EVENT_PREFIX.startsWith(buffer) ||
    buffer.startsWith(X11_EVENT_PREFIX)
  )
    return true;
  return false;
}

/**
 * Checks if the buffer *starts* with a complete mouse sequence.
 * Returns the length of the sequence if matched, or 0 if not.
 */
export function getMouseSequenceLength(buffer: string): number {
  const sgrMatch = buffer.match(SGR_MOUSE_REGEX);
  if (sgrMatch) return sgrMatch[0].length;
  const x11Match = buffer.match(X11_MOUSE_REGEX);
  if (x11Match) return x11Match[0].length;
  return 0;
}
