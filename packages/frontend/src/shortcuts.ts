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

/** Try to match a keypress to a global action. Returns null if no match. */
export function matchShortcut(input: string, key: Key): Action | null {
  // Cmd+S → open settings
  if (key.meta && input === "s") return { type: "open_settings" };

  // Cmd+T → new conversation
  if (key.meta && input === "t") return { type: "new_conversation" };

  // Esc → close overlay (if one is open; app decides whether to consume)
  if (key.escape) return { type: "close_overlay" };

  // Ctrl+C or q → quit
  if (key.ctrl && input === "c") return { type: "quit" };
  if (input === "q" && !key.ctrl && !key.meta) return { type: "quit" };

  return null;
}
