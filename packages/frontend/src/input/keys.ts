// ---------------------------------------------------------------------------
// input/keys.ts — Raw stdin key parser
//
// Adapted from Gemini CLI's KeypressContext.tsx.
// Parses ANSI escape sequences + Kitty keyboard protocol (CSI u) into
// structured Key objects. This replaces Ink's useInput for reliable
// modifier detection (Cmd+Backspace, Option+Delete, Shift+Enter, etc.).
// ---------------------------------------------------------------------------

const ESC = "\x1b";

// ---- Key type --------------------------------------------------------------

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

// ---- Standard ANSI escape code → key name map -----------------------------

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

// ---- Kitty keyboard protocol (CSI u) code → key name map ------------------

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
  57414: { name: "enter" },
};

// Numpad keys in Application Keypad Mode (SS3 sequences)
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

// macOS Option+key produces special Unicode characters; map them back
const MAC_ALT_KEY_MAP: Record<string, string> = {
  "\u222B": "b", // ∫ — Option+B (backward word)
  "\u0192": "f", // ƒ — Option+F (forward word)
  "\u00B5": "m", // µ — Option+M
  "\u03A9": "z", // Ω — Option+Z
  "\u00B8": "Z", // ¸ — Option+Shift+Z
  "\u2202": "d", // ∂ — Option+D (delete word forward)
};

// ---- UTF-16 helpers --------------------------------------------------------

const UTF16_SURROGATE_THRESHOLD = 0x10000;
function charLengthAt(str: string, i: number): number {
  if (str.length <= i) return 1;
  const code = str.codePointAt(i);
  return code !== undefined && code >= UTF16_SURROGATE_THRESHOLD ? 2 : 1;
}

// ---- Escape-sequence timeout -----------------------------------------------

const ESC_TIMEOUT = 50;
const PASTE_TIMEOUT = 30_000;
const BACKSLASH_ENTER_TIMEOUT = 5;
const FAST_RETURN_TIMEOUT = 30;

// ---- emitKeys generator (core parser) --------------------------------------

/**
 * Generator that consumes individual characters from stdin and yields
 * structured Key objects. Feed it one character at a time; send an empty
 * string to signal an escape-sequence timeout.
 */
function* emitKeys(
  keypressHandler: KeypressHandler,
): Generator<void, void, string> {
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
        // OSC sequence — read until BEL, ESC \, or timeout
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
            const decoded = Buffer.from(match[1]!, "base64").toString("utf-8");
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
            // ignore malformed
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
          // SGR mouse mode — consume and skip
          ch = yield;
          sequence += ch;
          while (ch === "" || ch === ";" || (ch >= "0" && ch <= "9")) {
            ch = yield;
            sequence += ch;
          }
        } else if (ch === "M") {
          // X11 mouse mode
          ch = yield;
          sequence += ch;
          ch = yield;
          sequence += ch;
          ch = yield;
          sequence += ch;
        }

        const cmd_seq = sequence.slice(cmdStart);
        let match;

        if (
          (match = /^(\d+)(?:;(\d+))?(?:;(\d+))?([~^$u])$/.exec(cmd_seq))
        ) {
          if (match[1] === "27" && match[3] && match[4] === "~") {
            // modifyOtherKeys format
            code += match[3] + "u";
            modifier = parseInt(match[2] ?? "1", 10) - 1;
          } else {
            code += (match[1] ?? "") + (match[4] ?? "");
            modifier = parseInt(match[2] ?? "1", 10) - 1;
          }
        } else if (
          (match = /^(\d+)?(?:;(\d+))?([A-Za-z])$/.exec(cmd_seq))
        ) {
          code += match[3];
          modifier = parseInt(match[2] ?? match[1] ?? "1", 10) - 1;
        } else {
          code += cmd_seq;
        }
      }

      // Parse modifier bitmask
      shift = !!(modifier & 1);
      alt = !!(modifier & 2);
      ctrl = !!(modifier & 4);
      cmd = !!(modifier & 8);

      const keyInfo = KEY_INFO_MAP[code!];
      if (keyInfo) {
        name = keyInfo.name;
        if (keyInfo.shift) shift = true;
        if (keyInfo.ctrl) ctrl = true;
        if (name === "space" && !ctrl && !cmd && !alt) {
          sequence = " ";
          insertable = true;
        }
      } else {
        const numpadChar = NUMPAD_MAP[code!];
        if (numpadChar) {
          name = numpadChar;
          if (!ctrl && !cmd && !alt) {
            sequence = numpadChar;
            insertable = true;
          }
        } else {
          name = "undefined";
          if (code!.endsWith("u") || code!.endsWith("~")) {
            const codeNumber = parseInt(code!.slice(1, -1), 10);
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
    } else if (MAC_ALT_KEY_MAP[ch]) {
      const mapped = MAC_ALT_KEY_MAP[ch]!;
      name = mapped.toLowerCase();
      shift = mapped !== name;
      alt = true;
    } else if (sequence === `${ESC}${ESC}`) {
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
      // Any other printable character
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
  }
}

// ---- Middleware: paste buffering (bracketed paste mode) ---------------------

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
        if (key === null || key.name === "paste-end") break;
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
  bufferer.next();
  return (key: Key) => {
    bufferer.next(key);
  };
}

// ---- Middleware: backslash-enter → shift+enter ------------------------------

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
        keypressHandler({ ...nextKey, shift: true, sequence: "\r" });
      } else {
        keypressHandler(key);
        keypressHandler(nextKey);
      }
    }
  })();
  bufferer.next();
  return (key: Key) => {
    bufferer.next(key);
  };
}

// ---- Middleware: fast-return → shift+enter (non-kitty terminals) ------------

function bufferFastReturn(keypressHandler: KeypressHandler): KeypressHandler {
  let lastKeyTime = 0;
  return (key: Key) => {
    const now = Date.now();
    if (key.name === "enter" && now - lastKeyTime <= FAST_RETURN_TIMEOUT) {
      keypressHandler({
        ...key,
        name: "enter",
        shift: true,
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

// ---- Middleware: filter mouse + focus events --------------------------------

function nonKeyboardFilter(keypressHandler: KeypressHandler): KeypressHandler {
  // SGR mouse sequences: ESC [ < ... M/m
  // Focus events: ESC [ I / ESC [ O
  const MOUSE_RE = /^\x1b\[<[\d;]+[Mm]$/;
  const FOCUS_IN = "\x1b[I";
  const FOCUS_OUT = "\x1b[O";

  return (key: Key) => {
    if (
      !MOUSE_RE.test(key.sequence) &&
      key.sequence !== FOCUS_IN &&
      key.sequence !== FOCUS_OUT
    ) {
      keypressHandler(key);
    }
  };
}

// ---- Public: create a data listener from a keypress handler ----------------

/**
 * Returns a function that accepts raw stdin strings (from `data` events)
 * and feeds them through the escape-sequence parser, paste buffering, and
 * middleware pipeline, finally calling `handler` with structured Key objects.
 *
 * @param handler  - called for each resolved keypress
 * @param kittyEnabled - set true to skip the fast-return heuristic
 */
export function createInputPipeline(
  handler: KeypressHandler,
  kittyEnabled: boolean,
): (data: string) => void {
  // Build pipeline from inside out
  let processor: KeypressHandler = nonKeyboardFilter(handler);
  if (!kittyEnabled) {
    processor = bufferFastReturn(processor);
  }
  processor = bufferBackslashEnter(processor);
  processor = bufferPaste(processor);

  const parser = emitKeys(processor);
  parser.next(); // prime

  let timeoutId: ReturnType<typeof setTimeout>;
  return (data: string) => {
    clearTimeout(timeoutId);
    for (const char of data) {
      parser.next(char);
    }
    if (data.length !== 0) {
      timeoutId = setTimeout(() => parser.next(""), ESC_TIMEOUT);
    }
  };
}

