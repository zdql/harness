// ---------------------------------------------------------------------------
// keybindings/keyBindings.ts
//
// The Command enum + default key bindings, ported from gemini-cli
// (ui/key/keyBindings.ts). Changes:
//   - Removed `loadCustomKeybindings()` (file I/O via core `Storage`, zod,
//     comment-json). Add back with your own config path when needed.
//   - Command enum trimmed to the ones that make sense for a generic
//     agent frontend (dropped shell/extension/approval-mode entries).
// ---------------------------------------------------------------------------

import type { Key } from "../keypress/useKeypress.ts";

export enum Command {
  // Basic Controls
  RETURN = "basic.confirm",
  ESCAPE = "basic.cancel",
  QUIT = "basic.quit",
  EXIT = "basic.exit",

  // Cursor Movement
  HOME = "cursor.home",
  END = "cursor.end",
  MOVE_UP = "cursor.up",
  MOVE_DOWN = "cursor.down",
  MOVE_LEFT = "cursor.left",
  MOVE_RIGHT = "cursor.right",
  MOVE_WORD_LEFT = "cursor.wordLeft",
  MOVE_WORD_RIGHT = "cursor.wordRight",

  // Editing
  KILL_LINE_RIGHT = "edit.deleteRightAll",
  KILL_LINE_LEFT = "edit.deleteLeftAll",
  CLEAR_INPUT = "edit.clear",
  DELETE_WORD_BACKWARD = "edit.deleteWordLeft",
  DELETE_WORD_FORWARD = "edit.deleteWordRight",
  DELETE_CHAR_LEFT = "edit.deleteLeft",
  DELETE_CHAR_RIGHT = "edit.deleteRight",
  UNDO = "edit.undo",
  REDO = "edit.redo",

  // Scrolling
  SCROLL_UP = "scroll.up",
  SCROLL_DOWN = "scroll.down",
  SCROLL_HOME = "scroll.home",
  SCROLL_END = "scroll.end",
  PAGE_UP = "scroll.pageUp",
  PAGE_DOWN = "scroll.pageDown",

  // History & Search
  HISTORY_UP = "history.previous",
  HISTORY_DOWN = "history.next",
  REVERSE_SEARCH = "history.search.start",

  // Navigation
  NAVIGATION_UP = "nav.up",
  NAVIGATION_DOWN = "nav.down",
  DIALOG_NAVIGATION_UP = "nav.dialog.up",
  DIALOG_NAVIGATION_DOWN = "nav.dialog.down",
  DIALOG_NEXT = "nav.dialog.next",
  DIALOG_PREV = "nav.dialog.previous",

  // Suggestions & Completions
  ACCEPT_SUGGESTION = "suggest.accept",
  COMPLETION_UP = "suggest.focusPrevious",
  COMPLETION_DOWN = "suggest.focusNext",

  // Text Input
  SUBMIT = "input.submit",
  NEWLINE = "input.newline",
  PASTE_CLIPBOARD = "input.paste",

  // App Controls
  CLEAR_SCREEN = "app.clearScreen",
  SUSPEND_APP = "app.suspend",
}

/**
 * Data-driven key binding. Parses patterns like "ctrl+shift+z" in the
 * constructor, then `.matches(key)` returns whether a parsed Key matches.
 */
export class KeyBinding {
  private static readonly VALID_LONG_KEYS = new Set([
    ...Array.from({ length: 35 }, (_, i) => `f${i + 1}`),
    ...Array.from({ length: 10 }, (_, i) => `numpad${i}`),
    "left", "up", "right", "down",
    "pageup", "pagedown", "end", "home",
    "tab", "enter", "escape", "space", "backspace", "delete",
    "clear", "pausebreak", "capslock", "insert", "numlock",
    "scrolllock", "printscreen",
    "numpad_multiply", "numpad_add", "numpad_separator",
    "numpad_subtract", "numpad_decimal", "numpad_divide",
  ]);

  readonly name: string;
  readonly shift: boolean;
  readonly alt: boolean;
  readonly ctrl: boolean;
  readonly cmd: boolean;

  constructor(pattern: string) {
    let remains = pattern.trim();
    let shift = false;
    let alt = false;
    let ctrl = false;
    let cmd = false;

    let matched: boolean;
    do {
      matched = false;
      const lower = remains.toLowerCase();
      if (lower.startsWith("ctrl+")) { ctrl = true; remains = remains.slice(5); matched = true; }
      else if (lower.startsWith("shift+")) { shift = true; remains = remains.slice(6); matched = true; }
      else if (lower.startsWith("alt+")) { alt = true; remains = remains.slice(4); matched = true; }
      else if (lower.startsWith("option+")) { alt = true; remains = remains.slice(7); matched = true; }
      else if (lower.startsWith("opt+")) { alt = true; remains = remains.slice(4); matched = true; }
      else if (lower.startsWith("cmd+")) { cmd = true; remains = remains.slice(4); matched = true; }
      else if (lower.startsWith("meta+")) { cmd = true; remains = remains.slice(5); matched = true; }
    } while (matched);

    const key = remains;
    const isSingleChar = [...key].length === 1;

    if (!isSingleChar && !KeyBinding.VALID_LONG_KEYS.has(key.toLowerCase())) {
      throw new Error(
        `Invalid keybinding key: "${key}" in "${pattern}". ` +
          `Must be a single character or one of: ${[...KeyBinding.VALID_LONG_KEYS].join(", ")}`,
      );
    }

    this.name = key.toLowerCase();
    this.shift = shift || (isSingleChar && this.name !== key);
    this.alt = alt;
    this.ctrl = ctrl;
    this.cmd = cmd;
  }

  matches(key: Key): boolean {
    return (
      key.name === this.name &&
      !!key.shift === !!this.shift &&
      !!key.alt === !!this.alt &&
      !!key.ctrl === !!this.ctrl &&
      !!key.cmd === !!this.cmd
    );
  }

  equals(other: KeyBinding): boolean {
    return (
      this.name === other.name &&
      this.shift === other.shift &&
      this.alt === other.alt &&
      this.ctrl === other.ctrl &&
      this.cmd === other.cmd
    );
  }
}

export type KeyBindingConfig = Map<Command, readonly KeyBinding[]>;

/**
 * Default bindings. Matches gemini-cli's defaults closely — you can override
 * any of these at runtime by constructing your own config.
 */
export const defaultKeyBindingConfig: KeyBindingConfig = new Map([
  // Basic Controls
  [Command.RETURN, [new KeyBinding("enter")]],
  [Command.ESCAPE, [new KeyBinding("escape"), new KeyBinding("ctrl+[")]],
  [Command.QUIT, [new KeyBinding("ctrl+c")]],
  [Command.EXIT, [new KeyBinding("ctrl+d")]],

  // Cursor Movement
  [Command.HOME, [new KeyBinding("ctrl+a"), new KeyBinding("home")]],
  [Command.END, [new KeyBinding("ctrl+e"), new KeyBinding("end")]],
  [Command.MOVE_UP, [new KeyBinding("up")]],
  [Command.MOVE_DOWN, [new KeyBinding("down")]],
  [Command.MOVE_LEFT, [new KeyBinding("left")]],
  [Command.MOVE_RIGHT, [new KeyBinding("right"), new KeyBinding("ctrl+f")]],
  [Command.MOVE_WORD_LEFT, [
    new KeyBinding("ctrl+left"),
    new KeyBinding("alt+left"),
    new KeyBinding("alt+b"),
  ]],
  [Command.MOVE_WORD_RIGHT, [
    new KeyBinding("ctrl+right"),
    new KeyBinding("alt+right"),
    new KeyBinding("alt+f"),
  ]],

  // Editing
  [Command.KILL_LINE_RIGHT, [new KeyBinding("ctrl+k")]],
  [Command.KILL_LINE_LEFT, [new KeyBinding("ctrl+u")]],
  [Command.CLEAR_INPUT, [new KeyBinding("ctrl+c")]],
  [Command.DELETE_WORD_BACKWARD, [
    new KeyBinding("ctrl+backspace"),
    new KeyBinding("alt+backspace"),
    new KeyBinding("ctrl+w"),
  ]],
  [Command.DELETE_WORD_FORWARD, [
    new KeyBinding("ctrl+delete"),
    new KeyBinding("alt+delete"),
    new KeyBinding("alt+d"),
  ]],
  [Command.DELETE_CHAR_LEFT, [new KeyBinding("backspace"), new KeyBinding("ctrl+h")]],
  [Command.DELETE_CHAR_RIGHT, [new KeyBinding("delete"), new KeyBinding("ctrl+d")]],
  [Command.UNDO, [new KeyBinding("cmd+z"), new KeyBinding("alt+z")]],
  [Command.REDO, [
    new KeyBinding("ctrl+shift+z"),
    new KeyBinding("cmd+shift+z"),
    new KeyBinding("alt+shift+z"),
  ]],

  // Scrolling
  [Command.SCROLL_UP, [new KeyBinding("shift+up")]],
  [Command.SCROLL_DOWN, [new KeyBinding("shift+down")]],
  [Command.SCROLL_HOME, [new KeyBinding("ctrl+home"), new KeyBinding("shift+home")]],
  [Command.SCROLL_END, [new KeyBinding("ctrl+end"), new KeyBinding("shift+end")]],
  [Command.PAGE_UP, [new KeyBinding("pageup")]],
  [Command.PAGE_DOWN, [new KeyBinding("pagedown")]],

  // History
  [Command.HISTORY_UP, [new KeyBinding("ctrl+p")]],
  [Command.HISTORY_DOWN, [new KeyBinding("ctrl+n")]],
  [Command.REVERSE_SEARCH, [new KeyBinding("ctrl+r")]],

  // Navigation
  [Command.NAVIGATION_UP, [new KeyBinding("up")]],
  [Command.NAVIGATION_DOWN, [new KeyBinding("down")]],
  [Command.DIALOG_NAVIGATION_UP, [new KeyBinding("up"), new KeyBinding("k")]],
  [Command.DIALOG_NAVIGATION_DOWN, [new KeyBinding("down"), new KeyBinding("j")]],
  [Command.DIALOG_NEXT, [new KeyBinding("tab")]],
  [Command.DIALOG_PREV, [new KeyBinding("shift+tab")]],

  // Suggestions & Completions
  [Command.ACCEPT_SUGGESTION, [new KeyBinding("tab"), new KeyBinding("enter")]],
  [Command.COMPLETION_UP, [new KeyBinding("up"), new KeyBinding("ctrl+p")]],
  [Command.COMPLETION_DOWN, [new KeyBinding("down"), new KeyBinding("ctrl+n")]],

  // Text Input
  [Command.SUBMIT, [new KeyBinding("enter")]],
  [Command.NEWLINE, [
    new KeyBinding("ctrl+enter"),
    new KeyBinding("cmd+enter"),
    new KeyBinding("alt+enter"),
    new KeyBinding("shift+enter"),
    new KeyBinding("ctrl+j"),
  ]],
  [Command.PASTE_CLIPBOARD, [
    new KeyBinding("ctrl+v"),
    new KeyBinding("cmd+v"),
    new KeyBinding("alt+v"),
  ]],

  // App Controls
  [Command.CLEAR_SCREEN, [new KeyBinding("ctrl+l")]],
  [Command.SUSPEND_APP, [new KeyBinding("ctrl+z")]],
]);
