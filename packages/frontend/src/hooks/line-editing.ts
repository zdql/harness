// ---------------------------------------------------------------------------
// hooks/line-editing.ts — Common line-editing shortcuts for terminal input
//
// Handles the shortcuts that terminal users expect. These map to the actual
// escape sequences that terminals send (not GUI keybindings — e.g. Cmd+V
// is handled by the terminal emulator, not by us).
//
// Supported:
//   Ctrl+U          Clear entire line
//   Ctrl+W          Delete word backward
//   Ctrl+K          Clear from cursor to end (same as clear since no cursor pos)
//   Ctrl+A          Move to start (clears — no cursor position tracking yet)
//   Option+Backspace Delete word backward (macOS; arrives as meta+backspace)
// ---------------------------------------------------------------------------

interface Key {
  ctrl: boolean;
  meta: boolean;
  escape: boolean;
  return: boolean;
  upArrow: boolean;
  downArrow: boolean;
  backspace: boolean;
  delete: boolean;
}

export type EditAction =
  | { type: "clear_line" }
  | { type: "delete_word" }
  | null;

/** Try to match a keypress to a line-editing action. Returns null if no match. */
export function matchEditShortcut(input: string, key: Key): EditAction {
  // Ctrl+U or Ctrl+K → clear entire line
  if (key.ctrl && (input === "u" || input === "k")) {
    return { type: "clear_line" };
  }

  // Ctrl+W → delete word backward
  if (key.ctrl && input === "w") {
    return { type: "delete_word" };
  }

  // Option+Backspace → delete word backward (macOS)
  if (key.meta && key.backspace) {
    return { type: "delete_word" };
  }

  return null;
}

/** Delete the last word from a string (whitespace-delimited). */
export function deleteWord(text: string): string {
  // Trim trailing spaces, then remove the last word
  const trimmed = text.replace(/\s+$/, "");
  const lastSpace = trimmed.lastIndexOf(" ");
  if (lastSpace === -1) return "";
  return trimmed.slice(0, lastSpace + 1);
}
