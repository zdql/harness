// ---------------------------------------------------------------------------
// terminal/mouse.ts — parse SGR + X11 mouse escape sequences.
// Used by KeypressContext's nonKeyboardEventFilter to drop mouse bytes from
// the keyboard event stream.
//
// Copied from gemini-cli (ui/utils/mouse.ts); the single import from
// gemini-cli-core was replaced with a local import from ./controlSequences.ts.
// ---------------------------------------------------------------------------

import { enableMouseEvents, disableMouseEvents } from "./controlSequences.ts";
import {
  SGR_MOUSE_REGEX,
  X11_MOUSE_REGEX,
  SGR_EVENT_PREFIX,
  X11_EVENT_PREFIX,
  couldBeMouseSequence as inputCouldBeMouseSequence,
} from "./input.ts";

export type MouseEventName =
  | "left-press"
  | "left-release"
  | "right-press"
  | "right-release"
  | "middle-press"
  | "middle-release"
  | "scroll-up"
  | "scroll-down"
  | "scroll-left"
  | "scroll-right"
  | "move"
  | "double-click";

export const DOUBLE_CLICK_THRESHOLD_MS = 400;
export const DOUBLE_CLICK_DISTANCE_TOLERANCE = 2;

export interface MouseEvent {
  name: MouseEventName;
  col: number;
  row: number;
  shift: boolean;
  meta: boolean;
  ctrl: boolean;
  button: "left" | "middle" | "right" | "none";
}

export type MouseHandler = (event: MouseEvent) => void | boolean;

export function getMouseEventName(
  buttonCode: number,
  isRelease: boolean,
): MouseEventName | null {
  const isMove = (buttonCode & 32) !== 0;

  if (buttonCode === 66) return "scroll-left";
  if (buttonCode === 67) return "scroll-right";
  if ((buttonCode & 64) === 64) {
    return (buttonCode & 1) === 0 ? "scroll-up" : "scroll-down";
  }
  if (isMove) return "move";

  const button = buttonCode & 3;
  const type = isRelease ? "release" : "press";
  switch (button) {
    case 0:
      return `left-${type}` as MouseEventName;
    case 1:
      return `middle-${type}` as MouseEventName;
    case 2:
      return `right-${type}` as MouseEventName;
    default:
      return null;
  }
}

function getButtonFromCode(code: number): MouseEvent["button"] {
  const button = code & 3;
  switch (button) {
    case 0:
      return "left";
    case 1:
      return "middle";
    case 2:
      return "right";
    default:
      return "none";
  }
}

export function parseSGRMouseEvent(
  buffer: string,
): { event: MouseEvent; length: number } | null {
  const match = buffer.match(SGR_MOUSE_REGEX);
  if (!match) return null;

  const buttonCode = parseInt(match[1]!, 10);
  const col = parseInt(match[2]!, 10);
  const row = parseInt(match[3]!, 10);
  const action = match[4];
  const isRelease = action === "m";

  const shift = (buttonCode & 4) !== 0;
  const meta = (buttonCode & 8) !== 0;
  const ctrl = (buttonCode & 16) !== 0;

  const name = getMouseEventName(buttonCode, isRelease);
  if (!name) return null;

  return {
    event: {
      name,
      ctrl,
      meta,
      shift,
      col,
      row,
      button: getButtonFromCode(buttonCode),
    },
    length: match[0].length,
  };
}

export function parseX11MouseEvent(
  buffer: string,
): { event: MouseEvent; length: number } | null {
  const match = buffer.match(X11_MOUSE_REGEX);
  if (!match) return null;

  const b = match[1]!.charCodeAt(0) - 32;
  const col = match[1]!.charCodeAt(1) - 32;
  const row = match[1]!.charCodeAt(2) - 32;

  const shift = (b & 4) !== 0;
  const meta = (b & 8) !== 0;
  const ctrl = (b & 16) !== 0;
  const isMove = (b & 32) !== 0;
  const isWheel = (b & 64) !== 0;

  let name: MouseEventName | null = null;

  if (isWheel) {
    const button = b & 3;
    if (button === 0) name = "scroll-up";
    else if (button === 1) name = "scroll-down";
  } else if (isMove) {
    name = "move";
  } else {
    const button = b & 3;
    if (button === 3) {
      name = "left-release";
    } else if (button === 0) {
      name = "left-press";
    } else if (button === 1) {
      name = "middle-press";
    } else if (button === 2) {
      name = "right-press";
    }
  }

  if (!name) return null;

  let button = getButtonFromCode(b);
  if (name === "left-release" && button === "none") {
    button = "left";
  }

  return {
    event: { name, ctrl, meta, shift, col, row, button },
    length: match[0].length,
  };
}

export function parseMouseEvent(
  buffer: string,
): { event: MouseEvent; length: number } | null {
  return parseSGRMouseEvent(buffer) || parseX11MouseEvent(buffer);
}

export function isIncompleteMouseSequence(buffer: string): boolean {
  if (!inputCouldBeMouseSequence(buffer)) return false;
  if (parseMouseEvent(buffer)) return false;

  if (buffer.startsWith(X11_EVENT_PREFIX)) {
    return buffer.length < X11_EVENT_PREFIX.length + 3;
  }

  if (buffer.startsWith(SGR_EVENT_PREFIX)) {
    return !/[mM]/.test(buffer) && buffer.length < 50;
  }

  return true;
}

export { enableMouseEvents, disableMouseEvents };
