// ---------------------------------------------------------------------------
// keypress/KeypressContext.tsx
//
// Ported from gemini-cli (ui/contexts/KeypressContext.tsx).
//
// This is the heart of the input pipeline:
//   stdin 'data' chunks
//     → emitKeys() generator parses CSI / SS3 / Kitty-CSI-u / OSC 52 / etc.
//     → bufferPaste / bufferBackslashEnter / bufferFastReturn middleware
//     → nonKeyboardEventFilter drops mouse/focus bytes
//     → broadcast() to priority-ordered subscribers
//     → useKeypress() consumers in components
//
// Subscribers are called in descending priority; within a priority level the
// most-recently-subscribed handler runs first (stack). Any handler returning
// `true` halts propagation.
//
// Changes from the original:
//   - Removed `debugLogger`, `Config`, `appEvents` imports (Gemini-specific).
//   - Removed `useSettingsStore` — debugKeystrokeLogging is now a prop.
//   - Replaced `mnemonist.MultiMap` with a plain Map<number, Set<Handler>>
//     to avoid the extra dependency.
//   - Updated import paths to local terminal/ + ./useFocus.ts.
// ---------------------------------------------------------------------------

import { useStdin } from "ink";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  type ReactNode,
} from "react";

import { ESC } from "../terminal/input.ts";
import { parseMouseEvent } from "../terminal/mouse.ts";
import { FOCUS_IN, FOCUS_OUT } from "./useFocus.ts";
import { terminalCapabilityManager } from "../terminal/terminalCapabilityManager.ts";

export const BACKSLASH_ENTER_TIMEOUT = 5;
export const ESC_TIMEOUT = 50;
export const PASTE_TIMEOUT = 30_000;
export const FAST_RETURN_TIMEOUT = 30;

export enum KeypressPriority {
  Low = -100,
  Normal = 0,
  High = 100,
  Critical = 200,
}

// -------- Lookup tables ----------------------------------------------------

const KEY_INFO_MAP: Record<
  string,
  { name: string; shift?: boolean; ctrl?: boolean }
> = {
  "[200~": { name: "paste-start" },
  "[201~": { name: "paste-end" },
  "[[A": { name: "f1" },
  "[[B": { name: "f2" },
  "[[C": { name: "f3" },
  "[[D": { name: "f4" },
  "[[E": { name: "f5" },
  "[1~": { name: "home" },
  "[2~": { name: "insert" },
  "[3~": { name: "delete" },
  "[4~": { name: "end" },
  "[5~": { name: "pageup" },
  "[6~": { name: "pagedown" },
  "[7~": { name: "home" },
  "[8~": { name: "end" },
  "[11~": { name: "f1" },
  "[12~": { name: "f2" },
  "[13~": { name: "f3" },
  "[14~": { name: "f4" },
  "[15~": { name: "f5" },
  "[17~": { name: "f6" },
  "[18~": { name: "f7" },
  "[19~": { name: "f8" },
  "[20~": { name: "f9" },
  "[21~": { name: "f10" },
  "[23~": { name: "f11" },
  "[24~": { name: "f12" },
  "[25~": { name: "f13" },
  "[26~": { name: "f14" },
  "[28~": { name: "f15" },
  "[29~": { name: "f16" },
  "[31~": { name: "f17" },
  "[32~": { name: "f18" },
  "[33~": { name: "f19" },
  "[34~": { name: "f20" },
  "[A": { name: "up" },
  "[B": { name: "down" },
  "[C": { name: "right" },
  "[D": { name: "left" },
  "[E": { name: "clear" },
  "[F": { name: "end" },
  "[H": { name: "home" },
  "[P": { name: "f1" },
  "[Q": { name: "f2" },
  "[R": { name: "f3" },
  "[S": { name: "f4" },
  OA: { name: "up" },
  OB: { name: "down" },
  OC: { name: "right" },
  OD: { name: "left" },
  OE: { name: "clear" },
  OF: { name: "end" },
  OH: { name: "home" },
  OP: { name: "f1" },
  OQ: { name: "f2" },
  OR: { name: "f3" },
  OS: { name: "f4" },
  OZ: { name: "tab", shift: true },
  "[[5~": { name: "pageup" },
  "[[6~": { name: "pagedown" },
  "[a": { name: "up", shift: true },
  "[b": { name: "down", shift: true },
  "[c": { name: "right", shift: true },
  "[d": { name: "left", shift: true },
  "[e": { name: "clear", shift: true },
  "[2$": { name: "insert", shift: true },
  "[3$": { name: "delete", shift: true },
  "[5$": { name: "pageup", shift: true },
  "[6$": { name: "pagedown", shift: true },
  "[7$": { name: "home", shift: true },
  "[8$": { name: "end", shift: true },
  "[Z": { name: "tab", shift: true },
  Oa: { name: "up", ctrl: true },
  Ob: { name: "down", ctrl: true },
  Oc: { name: "right", ctrl: true },
  Od: { name: "left", ctrl: true },
  Oe: { name: "clear", ctrl: true },
  "[2^": { name: "insert", ctrl: true },
  "[3^": { name: "delete", ctrl: true },
  "[5^": { name: "pageup", ctrl: true },
  "[6^": { name: "pagedown", ctrl: true },
  "[7^": { name: "home", ctrl: true },
  "[8^": { name: "end", ctrl: true },
};

// Kitty Keyboard Protocol (CSI u) code mappings
const KITTY_CODE_MAP: Record<number, { name: string; sequence?: string }> = {
  2: { name: "insert" },
  3: { name: "delete" },
  5: { name: "pageup" },
  6: { name: "pagedown" },
  9: { name: "tab" },
  13: { name: "enter" },
  14: { name: "up" },
  15: { name: "down" },
  16: { name: "right" },
  17: { name: "left" },
  27: { name: "escape" },
  32: { name: "space", sequence: " " },
  127: { name: "backspace" },
  57358: { name: "capslock" },
  57359: { name: "scrolllock" },
  57360: { name: "numlock" },
  57361: { name: "printscreen" },
  57362: { name: "pausebreak" },
  57409: { name: "numpad_decimal", sequence: "." },
  57410: { name: "numpad_divide", sequence: "/" },
  57411: { name: "numpad_multiply", sequence: "*" },
  57412: { name: "numpad_subtract", sequence: "-" },
  57413: { name: "numpad_add", sequence: "+" },
  57414: { name: "enter" },
  57416: { name: "numpad_separator", sequence: "," },
  ...Object.fromEntries(
    Array.from({ length: 23 }, (_, i) => [302 + i, { name: `f${13 + i}` }]),
  ),
  ...Object.fromEntries(
    Array.from({ length: 10 }, (_, i) => [
      57399 + i,
      { name: `numpad${i}`, sequence: String(i) },
    ]),
  ),
};

// SS3 numpad (Application Keypad Mode)
const NUMPAD_MAP: Record<string, string> = {
  Oj: "*",
  Ok: "+",
  Om: "-",
  Oo: "/",
  Op: "0",
  Oq: "1",
  Or: "2",
  Os: "3",
  Ot: "4",
  Ou: "5",
  Ov: "6",
  Ow: "7",
  Ox: "8",
  Oy: "9",
  On: ".",
};

const kUTF16SurrogateThreshold = 0x10000;
function charLengthAt(str: string, i: number): number {
  if (str.length <= i) return 1;
  const code = str.codePointAt(i);
  return code !== undefined && code >= kUTF16SurrogateThreshold ? 2 : 1;
}

// Mac Option+letter characters (some are Option hotkeys on macOS)
const MAC_ALT_KEY_CHARACTER_MAP: Record<string, string> = {
  "\u222B": "b", // ∫ — back one word
  "\u0192": "f", // ƒ — forward one word
  "\u00B5": "m", // µ — toggle markup view
  "\u03A9": "z", // Ω — Option+z
  "\u00B8": "Z", // ¸ — Option+Shift+z
  "\u2202": "d", // ∂ — delete word forward
};

// -------- Middleware wrappers ---------------------------------------------

function nonKeyboardEventFilter(
  keypressHandler: KeypressHandler,
): KeypressHandler {
  return (key: Key) => {
    if (
      !parseMouseEvent(key.sequence) &&
      key.sequence !== FOCUS_IN &&
      key.sequence !== FOCUS_OUT
    ) {
      keypressHandler(key);
    }
  };
}

/**
 * Converts return keys pressed quickly after insertable keys into shift+return.
 * Used as a fallback for terminals without bracketed paste (so they don't
 * treat a pasted line's Enter as a submit).
 */
function bufferFastReturn(keypressHandler: KeypressHandler): KeypressHandler {
  let lastKeyTime = 0;
  return (key: Key) => {
    const now = Date.now();
    if (key.name === "enter" && now - lastKeyTime <= FAST_RETURN_TIMEOUT) {
      keypressHandler({
        ...key,
        name: "enter",
        shift: true, // newline, not submission
        alt: false,
        ctrl: false,
        cmd: false,
        sequence: "\r",
        insertable: true,
      });
    } else {
      keypressHandler(key);
    }
    lastKeyTime = key.insertable ? now : 0;
  };
}

/**
 * Buffers "\" keys to see if they are followed by Enter.
 * "\" + Enter within BACKSLASH_ENTER_TIMEOUT ms becomes shift+enter (newline).
 */
function bufferBackslashEnter(
  keypressHandler: KeypressHandler,
): KeypressHandler {
  const bufferer = (function* (): Generator<void, void, Key | null> {
    while (true) {
      const key = yield;

      if (key == null) continue;
      if (key.sequence !== "\\") {
        keypressHandler(key);
        continue;
      }

      const timeoutId = setTimeout(
        () => bufferer.next(null),
        BACKSLASH_ENTER_TIMEOUT,
      );
      const nextKey = yield;
      clearTimeout(timeoutId);

      if (nextKey === null) {
        keypressHandler(key);
      } else if (nextKey.name === "enter") {
        keypressHandler({
          ...nextKey,
          shift: true,
          sequence: "\r",
        });
      } else {
        keypressHandler(key);
        keypressHandler(nextKey);
      }
    }
  })();

  bufferer.next(); // prime

  return (key: Key) => {
    bufferer.next(key);
  };
}

/**
 * Buffers paste events between paste-start and paste-end into one paste event.
 */
function bufferPaste(keypressHandler: KeypressHandler): KeypressHandler {
  const bufferer = (function* (): Generator<void, void, Key | null> {
    while (true) {
      let key = yield;

      if (key === null) continue;
      if (key.name !== "paste-start") {
        keypressHandler(key);
        continue;
      }

      let buffer = "";
      while (true) {
        const timeoutId = setTimeout(() => bufferer.next(null), PASTE_TIMEOUT);
        key = yield;
        clearTimeout(timeoutId);

        if (key === null) break;
        if (key.name === "paste-end") break;
        buffer += key.sequence;
      }

      if (buffer.length > 0) {
        keypressHandler({
          name: "paste",
          shift: false,
          alt: false,
          ctrl: false,
          cmd: false,
          insertable: true,
          sequence: buffer,
        });
      }
    }
  })();
  bufferer.next(); // prime

  return (key: Key) => {
    bufferer.next(key);
  };
}

// -------- Raw char → Key generator ----------------------------------------

function createDataListener(
  keypressHandler: KeypressHandler,
): (data: string) => void {
  const parser = emitKeys(keypressHandler);
  parser.next(); // prime

  let timeoutId: NodeJS.Timeout | undefined;
  return (data: string) => {
    if (timeoutId) clearTimeout(timeoutId);
    for (const char of data) {
      parser.next(char);
    }
    if (data.length !== 0) {
      timeoutId = setTimeout(() => parser.next(""), ESC_TIMEOUT);
    }
  };
}

/**
 * State machine over raw characters; emits Key events. Buffers escape
 * sequences until a complete sequence arrives or an empty-string timeout.
 */
function* emitKeys(
  keypressHandler: KeypressHandler,
): Generator<void, void, string> {
  const lang = process.env["LANG"] || "";
  const lcAll = process.env["LC_ALL"] || "";
  const isGreek = lang.startsWith("el") || lcAll.startsWith("el");

  while (true) {
    let ch = yield;
    let sequence = ch;
    let escaped = false;

    let name: string | undefined = undefined;
    let shift = false;
    let alt = false;
    let ctrl = false;
    let cmd = false;
    let code: string | undefined = undefined;
    let insertable = false;

    if (ch === ESC) {
      escaped = true;
      ch = yield;
      sequence += ch;

      if (ch === ESC) {
        ch = yield;
        sequence += ch;
      }
    }

    if (escaped && (ch === "O" || ch === "[" || ch === "]")) {
      code = ch;
      let modifier = 0;

      if (ch === "]") {
        // OSC sequence
        let buffer = "";
        while (true) {
          const next = yield;
          if (next === "" || next === "\u0007") break;
          if (next === ESC) {
            const afterEsc = yield;
            if (afterEsc === "" || afterEsc === "\\") break;
            buffer += next + afterEsc;
            continue;
          }
          buffer += next;
        }

        // OSC 52 clipboard response
        const match = /^52;[cp];(.*)$/.exec(buffer);
        if (match) {
          try {
            const base64Data = match[1]!;
            const decoded = Buffer.from(base64Data, "base64").toString("utf-8");
            keypressHandler({
              name: "paste",
              shift: false,
              alt: false,
              ctrl: false,
              cmd: false,
              insertable: true,
              sequence: decoded,
            });
          } catch {
            // swallow
          }
        }

        continue;
      } else if (ch === "O") {
        ch = yield;
        sequence += ch;

        if (ch >= "0" && ch <= "9") {
          modifier = parseInt(ch, 10) - 1;
          ch = yield;
          sequence += ch;
        }

        code += ch;
      } else if (ch === "[") {
        ch = yield;
        sequence += ch;

        if (ch === "[") {
          code += ch;
          ch = yield;
          sequence += ch;
        }

        const cmdStart = sequence.length - 1;

        while (ch >= "0" && ch <= "9") {
          ch = yield;
          sequence += ch;
        }

        if (ch === ";") {
          while (ch === ";") {
            ch = yield;
            sequence += ch;
            while (ch >= "0" && ch <= "9") {
              ch = yield;
              sequence += ch;
            }
          }
        } else if (ch === "<") {
          // SGR mouse mode
          ch = yield;
          sequence += ch;
          while (ch === "" || ch === ";" || (ch >= "0" && ch <= "9")) {
            ch = yield;
            sequence += ch;
          }
        } else if (ch === "M") {
          // X11 mouse mode: three bytes after 'M'
          ch = yield;
          sequence += ch;
          ch = yield;
          sequence += ch;
          ch = yield;
          sequence += ch;
        }

        const cmdSeq = sequence.slice(cmdStart);
        let match;

        if ((match = /^(\d+)(?:;(\d+))?(?:;(\d+))?([~^$u])$/.exec(cmdSeq))) {
          if (match[1] === "27" && match[3] && match[4] === "~") {
            // modifyOtherKeys: CSI 27 ; mod ; key ~  → treat as CSI u
            code += match[3] + "u";
            modifier = parseInt(match[2] ?? "1", 10) - 1;
          } else {
            code += (match[1] ?? "") + (match[4] ?? "");
            modifier = parseInt(match[2] ?? "1", 10) - 1;
          }
        } else if ((match = /^(\d+)?(?:;(\d+))?([A-Za-z])$/.exec(cmdSeq))) {
          code += match[3] ?? "";
          modifier = parseInt(match[2] ?? match[1] ?? "1", 10) - 1;
        } else {
          code += cmdSeq;
        }
      }

      // Decode modifier bitmask
      shift = !!(modifier & 1);
      alt = !!(modifier & 2);
      ctrl = !!(modifier & 4);
      cmd = !!(modifier & 8);

      const keyInfo = code ? KEY_INFO_MAP[code] : undefined;
      if (keyInfo) {
        name = keyInfo.name;
        if (keyInfo.shift) shift = true;
        if (keyInfo.ctrl) ctrl = true;
        if (name === "space" && !ctrl && !cmd && !alt) {
          sequence = " ";
          insertable = true;
        }
      } else {
        const numpadChar = code ? NUMPAD_MAP[code] : undefined;
        if (numpadChar) {
          name = numpadChar;
          if (!ctrl && !cmd && !alt) {
            sequence = numpadChar;
            insertable = true;
          }
        } else if (code) {
          name = "undefined";
          if (code.endsWith("u") || code.endsWith("~")) {
            // CSI-u or tilde-coded functional keys
            const codeNumber = parseInt(code.slice(1, -1), 10);
            const mapped = KITTY_CODE_MAP[codeNumber];
            if (mapped) {
              name = mapped.name;
              if (mapped.sequence && !ctrl && !cmd && !alt) {
                sequence = mapped.sequence;
                insertable = true;
              }
            } else if (
              codeNumber >= 33 &&
              codeNumber <= 0x10ffff &&
              (codeNumber < 0xd800 || codeNumber > 0xdfff)
            ) {
              const char = String.fromCodePoint(codeNumber);
              name = char.toLowerCase();
              if (char !== name) shift = true;
              if (!ctrl && !cmd && !alt) {
                sequence = char;
                insertable = true;
              }
            }
          }
        }
      }
    } else if (ch === "\r") {
      name = "enter";
      alt = escaped;
    } else if (escaped && ch === "\n") {
      name = "enter";
      alt = escaped;
    } else if (ch === "\t") {
      name = "tab";
      alt = escaped;
    } else if (ch === "\b" || ch === "\x7f") {
      name = "backspace";
      alt = escaped;
    } else if (ch === ESC) {
      name = "escape";
      alt = escaped;
    } else if (ch === " ") {
      name = "space";
      alt = escaped;
      insertable = true;
    } else if (!escaped && ch <= "\x1a") {
      // ctrl+letter
      name = String.fromCharCode(ch.charCodeAt(0) + "a".charCodeAt(0) - 1);
      ctrl = true;
    } else if (/^[0-9A-Za-z]$/.exec(ch) !== null) {
      name = ch.toLowerCase();
      shift = /^[A-Z]$/.exec(ch) !== null;
      alt = escaped;
      insertable = true;
    } else if (MAC_ALT_KEY_CHARACTER_MAP[ch]) {
      if (isGreek && ch === "\u03A9") {
        insertable = true;
      } else {
        const mapped = MAC_ALT_KEY_CHARACTER_MAP[ch]!;
        name = mapped.toLowerCase();
        shift = mapped !== name;
        alt = true;
      }
    } else if (sequence === `${ESC}${ESC}`) {
      // Double escape: emit first escape immediately, then continue.
      name = "escape";
      alt = false;
      keypressHandler({
        name: "escape",
        shift,
        alt,
        ctrl,
        cmd,
        insertable: false,
        sequence: ESC,
      });
    } else if (escaped) {
      name = ch.length ? undefined : "escape";
      alt = ch.length > 0;
    } else {
      name = ch.toLowerCase();
      if (ch !== name) shift = true;
      insertable = true;
    }

    if (
      (sequence.length !== 0 && (name !== undefined || escaped)) ||
      charLengthAt(sequence, 0) === sequence.length
    ) {
      keypressHandler({
        name: name || "",
        shift,
        alt,
        ctrl,
        cmd,
        insertable,
        sequence,
      });
    }
    // else: unrecognized/broken escape, don't emit
  }
}

// -------- Public types -----------------------------------------------------

export interface Key {
  name: string;
  shift: boolean;
  alt: boolean;
  ctrl: boolean;
  cmd: boolean;
  insertable: boolean;
  sequence: string;
}

export type KeypressHandler = (key: Key) => boolean | void;

interface KeypressContextValue {
  subscribe: (
    handler: KeypressHandler,
    priority?: KeypressPriority | boolean,
  ) => void;
  unsubscribe: (handler: KeypressHandler) => void;
}

const KeypressContext = createContext<KeypressContextValue | undefined>(
  undefined,
);

export function useKeypressContext(): KeypressContextValue {
  const context = useContext(KeypressContext);
  if (!context) {
    throw new Error(
      "useKeypressContext must be used within a KeypressProvider",
    );
  }
  return context;
}

// -------- Provider ---------------------------------------------------------

export function KeypressProvider({
  children,
  debugKeystrokeLogging = false,
}: {
  children: ReactNode;
  debugKeystrokeLogging?: boolean;
}): React.JSX.Element {
  const { stdin, setRawMode } = useStdin();

  // Priority → Set<Handler>. Replaces mnemonist.MultiMap to avoid the dep.
  const subscribersByPriority = useRef<Map<number, Set<KeypressHandler>>>(
    new Map(),
  ).current;
  const subscribersToPriority = useRef<Map<KeypressHandler, number>>(
    new Map(),
  ).current;
  const sortedPriorities = useRef<number[]>([]);

  const recomputeSortedPriorities = useCallback(() => {
    sortedPriorities.current = Array.from(subscribersByPriority.keys()).sort(
      (a, b) => b - a,
    );
  }, [subscribersByPriority]);

  const subscribe = useCallback(
    (
      handler: KeypressHandler,
      priority: KeypressPriority | boolean = KeypressPriority.Normal,
    ) => {
      const p =
        typeof priority === "boolean"
          ? priority
            ? KeypressPriority.High
            : KeypressPriority.Normal
          : priority;

      subscribersToPriority.set(handler, p);
      let set = subscribersByPriority.get(p);
      const wasNew = !set;
      if (!set) {
        set = new Set();
        subscribersByPriority.set(p, set);
      }
      set.add(handler);

      if (wasNew) recomputeSortedPriorities();
    },
    [subscribersByPriority, subscribersToPriority, recomputeSortedPriorities],
  );

  const unsubscribe = useCallback(
    (handler: KeypressHandler) => {
      const p = subscribersToPriority.get(handler);
      if (p === undefined) return;
      const set = subscribersByPriority.get(p);
      if (set) {
        set.delete(handler);
        if (set.size === 0) {
          subscribersByPriority.delete(p);
          recomputeSortedPriorities();
        }
      }
      subscribersToPriority.delete(handler);
    },
    [subscribersByPriority, subscribersToPriority, recomputeSortedPriorities],
  );

  const broadcast = useCallback(
    (key: Key) => {
      if (debugKeystrokeLogging) {
        console.debug("[keystroke]", JSON.stringify(key));
      }
      for (const p of sortedPriorities.current) {
        const set = subscribersByPriority.get(p);
        if (!set) continue;
        // Stack order: most-recently-subscribed first.
        const handlers = Array.from(set).reverse();
        for (const handler of handlers) {
          if (handler(key) === true) return;
        }
      }
    },
    [subscribersByPriority, debugKeystrokeLogging],
  );

  useEffect(() => {
    terminalCapabilityManager.enableSupportedModes();

    const wasRaw = stdin.isRaw;
    if (wasRaw === false) setRawMode(true);

    process.stdin.setEncoding("utf8");

    let processor = nonKeyboardEventFilter(broadcast);
    if (!terminalCapabilityManager.isKittyProtocolEnabled()) {
      processor = bufferFastReturn(processor);
    }
    processor = bufferBackslashEnter(processor);
    processor = bufferPaste(processor);
    let dataListener = createDataListener(processor);

    if (debugKeystrokeLogging) {
      const old = dataListener;
      dataListener = (data: string) => {
        if (data.length > 0) {
          console.debug(`[raw stdin] ${JSON.stringify(data)}`);
        }
        old(data);
      };
    }

    stdin.on("data", dataListener);
    return () => {
      stdin.removeListener("data", dataListener);
      if (wasRaw === false) setRawMode(false);
    };
  }, [stdin, setRawMode, debugKeystrokeLogging, broadcast]);

  const contextValue = useMemo(
    () => ({ subscribe, unsubscribe }),
    [subscribe, unsubscribe],
  );

  return (
    <KeypressContext.Provider value={contextValue}>
      {children}
    </KeypressContext.Provider>
  );
}
