// ---------------------------------------------------------------------------
// input/commands.ts — Command enum, key bindings, and matchers
//
// Adapted from Gemini CLI's keyBindings.ts and keyMatchers.ts.
// Defines all available keyboard commands and their default bindings.
// ---------------------------------------------------------------------------

import type { Key } from "./keys.ts";

// ---- Command enum ----------------------------------------------------------

export enum Command {
  // Basic controls
  RETURN = "basic.confirm",
  ESCAPE = "basic.cancel",
  QUIT = "basic.quit",
  EXIT = "basic.exit",

  // Cursor movement
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

  // Scrolling
  SCROLL_UP = "scroll.up",
  SCROLL_DOWN = "scroll.down",
  PAGE_UP = "scroll.pageUp",
  PAGE_DOWN = "scroll.pageDown",

  // Text input
  SUBMIT = "input.submit",
  NEWLINE = "input.newline",

  // App controls
  NEW_CONVERSATION = "app.newConversation",
  TOGGLE_SETTINGS = "app.toggleSettings",
  CLEAR_SCREEN = "app.clearScreen",

  // Dialog navigation
  DIALOG_UP = "nav.dialog.up",
  DIALOG_DOWN = "nav.dialog.down",
  DIALOG_SELECT = "nav.dialog.select",
  DIALOG_DISMISS = "nav.dialog.dismiss",
}

// ---- KeyBinding class ------------------------------------------------------

class KeyBinding {
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
      if (lower.startsWith("ctrl+")) {
        ctrl = true;
        remains = remains.slice(5);
        matched = true;
      } else if (lower.startsWith("shift+")) {
        shift = true;
        remains = remains.slice(6);
        matched = true;
      } else if (lower.startsWith("alt+") || lower.startsWith("opt+")) {
        alt = true;
        remains = remains.slice(4);
        matched = true;
      } else if (lower.startsWith("option+")) {
        alt = true;
        remains = remains.slice(7);
        matched = true;
      } else if (lower.startsWith("cmd+") || lower.startsWith("meta+")) {
        cmd = true;
        remains = remains.slice(4 + (lower.startsWith("meta+") ? 1 : 0));
        matched = true;
      }
    } while (matched);

    const key = remains;
    const isSingleChar = [...key].length === 1;

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
}

// Helper
function kb(pattern: string): KeyBinding {
  return new KeyBinding(pattern);
}

// ---- Default key binding configuration -------------------------------------

const defaultBindings = new Map<Command, readonly KeyBinding[]>([
  // Basic controls
  [Command.RETURN, [kb("enter")]],
  [Command.ESCAPE, [kb("escape"), kb("ctrl+[")]],
  [Command.QUIT, [kb("ctrl+c")]],
  [Command.EXIT, [kb("ctrl+d")]],

  // Cursor movement
  [Command.HOME, [kb("ctrl+a"), kb("home")]],
  [Command.END, [kb("ctrl+e"), kb("end")]],
  [Command.MOVE_UP, [kb("up")]],
  [Command.MOVE_DOWN, [kb("down")]],
  [Command.MOVE_LEFT, [kb("left")]],
  [Command.MOVE_RIGHT, [kb("right"), kb("ctrl+f")]],
  [Command.MOVE_WORD_LEFT, [kb("ctrl+left"), kb("alt+left"), kb("alt+b")]],
  [Command.MOVE_WORD_RIGHT, [kb("ctrl+right"), kb("alt+right"), kb("alt+f")]],

  // Editing
  [Command.KILL_LINE_RIGHT, [kb("ctrl+k")]],
  [Command.KILL_LINE_LEFT, [kb("ctrl+u")]],
  [Command.CLEAR_INPUT, [kb("ctrl+c")]],
  [
    Command.DELETE_WORD_BACKWARD,
    [kb("ctrl+backspace"), kb("alt+backspace"), kb("ctrl+w")],
  ],
  [
    Command.DELETE_WORD_FORWARD,
    [kb("ctrl+delete"), kb("alt+delete"), kb("alt+d")],
  ],
  [Command.DELETE_CHAR_LEFT, [kb("backspace"), kb("ctrl+h")]],
  [Command.DELETE_CHAR_RIGHT, [kb("delete"), kb("ctrl+d")]],

  // Scrolling
  [Command.SCROLL_UP, [kb("shift+up")]],
  [Command.SCROLL_DOWN, [kb("shift+down")]],
  [Command.PAGE_UP, [kb("pageup")]],
  [Command.PAGE_DOWN, [kb("pagedown")]],

  // Text input
  [Command.SUBMIT, [kb("enter")]],
  [
    Command.NEWLINE,
    [
      kb("ctrl+enter"),
      kb("cmd+enter"),
      kb("alt+enter"),
      kb("shift+enter"),
      kb("ctrl+j"),
    ],
  ],

  // App controls
  [Command.NEW_CONVERSATION, [kb("ctrl+n")]],
  [Command.TOGGLE_SETTINGS, [kb("ctrl+s")]],
  [Command.CLEAR_SCREEN, [kb("ctrl+l")]],

  // Dialog navigation
  [Command.DIALOG_UP, [kb("up"), kb("k")]],
  [Command.DIALOG_DOWN, [kb("down"), kb("j")]],
  [Command.DIALOG_SELECT, [kb("enter")]],
  [Command.DIALOG_DISMISS, [kb("escape")]],
]);

// ---- Key matchers ----------------------------------------------------------

type KeyMatcher = (key: Key) => boolean;

export type KeyMatchers = {
  readonly [C in Command]: KeyMatcher;
};

function createKeyMatchers(): KeyMatchers {
  const matchers = {} as { [C in Command]: KeyMatcher };
  for (const command of Object.values(Command)) {
    const bindings = defaultBindings.get(command) ?? [];
    matchers[command] = (key: Key) => bindings.some((b) => b.matches(key));
  }
  return matchers as KeyMatchers;
}

export const keyMatchers: KeyMatchers = createKeyMatchers();
