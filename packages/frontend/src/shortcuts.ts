// ---------------------------------------------------------------------------
// shortcuts.ts — Global keyboard shortcuts
//
// Centralizes all keybindings so they're easy to find, change, and document.
// Each shortcut maps a key combo to a named action. The app dispatches
// actions — shortcuts just declare the mapping.
// ---------------------------------------------------------------------------

export type Action =
  | { type: "open_settings" }
  | { type: "close_overlay" }
  | { type: "new_conversation" }
  | { type: "quit" };

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

interface MatchOptions {
  /** When true, single-key shortcuts like `q` are suppressed. */
  promptFocused?: boolean;
}

/** Try to match a keypress to a global action. Returns null if no match. */
export function matchShortcut(
  input: string,
  key: Key,
  opts: MatchOptions = {},
): Action | null {
  // Ctrl+S → open settings (Ctrl passes through terminals; ⌘ does not)
  if (key.ctrl && input === "s") return { type: "open_settings" };

  // Ctrl+N → new conversation
  if (key.ctrl && input === "n") return { type: "new_conversation" };

  // Esc → close overlay (if one is open; app decides whether to consume)
  if (key.escape) return { type: "close_overlay" };

  // Ctrl+C → quit (always works)
  if (key.ctrl && input === "c") return { type: "quit" };

  // q → quit only when prompt is not focused (so it doesn't eat typing)
  if (input === "q" && !key.ctrl && !key.meta && !opts.promptFocused) {
    return { type: "quit" };
  }

  return null;
}
