// ---------------------------------------------------------------------------
// textbuffer/useTextBuffer.ts
//
// React hook that owns a TextBufferState and wires keypresses into its
// actions via the Command matchers. Components hand this to <InputPrompt>
// (or any consumer) to get multi-line editing, undo/redo, word ops, and
// grapheme-correct cursor movement "for free".
//
// The hook's *only* external concern is submission — it calls onSubmit(text)
// when the user presses Enter (without a newline modifier) and the buffer
// is non-empty, then resets.
// ---------------------------------------------------------------------------

import { useCallback, useReducer } from "react";
import { useKeypress, type Key } from "../keypress/useKeypress.ts";
import { useKeyMatchers } from "../keybindings/useKeyMatchers.tsx";
import { Command } from "../keybindings/keyBindings.ts";
import * as TB from "./textBuffer.ts";
import type { TextBufferState } from "./textBuffer.ts";

// ===== Actions =============================================================

type Action =
  | { type: "insert"; text: string }
  | { type: "newline" }
  | { type: "backspace" }
  | { type: "delete" }
  | { type: "delWordBack" }
  | { type: "delWordForward" }
  | { type: "killRight" }
  | { type: "killLeft" }
  | { type: "clear" }
  | { type: "moveLeft" }
  | { type: "moveRight" }
  | { type: "moveUp" }
  | { type: "moveDown" }
  | { type: "moveWordLeft" }
  | { type: "moveWordRight" }
  | { type: "moveLineStart" }
  | { type: "moveLineEnd" }
  | { type: "moveBufferStart" }
  | { type: "moveBufferEnd" }
  | { type: "undo" }
  | { type: "redo" }
  | { type: "reset" };

function reducer(s: TextBufferState, a: Action): TextBufferState {
  switch (a.type) {
    case "insert":          return TB.insert(s, a.text);
    case "newline":         return TB.insertNewline(s);
    case "backspace":       return TB.deleteCharBackward(s);
    case "delete":          return TB.deleteCharForward(s);
    case "delWordBack":     return TB.deleteWordBackward(s);
    case "delWordForward":  return TB.deleteWordForward(s);
    case "killRight":       return TB.killLineRight(s);
    case "killLeft":        return TB.killLineLeft(s);
    case "clear":           return TB.clear(s);
    case "moveLeft":        return TB.moveLeft(s);
    case "moveRight":       return TB.moveRight(s);
    case "moveUp":          return TB.moveUp(s);
    case "moveDown":        return TB.moveDown(s);
    case "moveWordLeft":    return TB.moveWordLeft(s);
    case "moveWordRight":   return TB.moveWordRight(s);
    case "moveLineStart":   return TB.moveLineStart(s);
    case "moveLineEnd":     return TB.moveLineEnd(s);
    case "moveBufferStart": return TB.moveBufferStart(s);
    case "moveBufferEnd":   return TB.moveBufferEnd(s);
    case "undo":            return TB.undo(s);
    case "redo":            return TB.redo(s);
    case "reset":           return TB.EMPTY_STATE;
  }
}

// ===== Hook ================================================================

export interface UseTextBufferOpts {
  /** Subscribe to keys only while this is true. */
  focused: boolean;
  /** Called with the full text when the user submits. */
  onSubmit: (text: string) => void;
  /** Initial text (for restoring drafts, etc.). */
  initialText?: string;
}

export interface UseTextBufferResult {
  state: TextBufferState;
  /** Programmatic submit (clears buffer). */
  submit: () => void;
  /** Programmatic reset to empty. */
  reset: () => void;
  /** Programmatic insert. */
  insert: (text: string) => void;
}

export function useTextBuffer({
  focused,
  onSubmit,
  initialText,
}: UseTextBufferOpts): UseTextBufferResult {
  const [state, dispatch] = useReducer(
    reducer,
    undefined,
    () => (initialText ? TB.createState(initialText) : TB.EMPTY_STATE),
  );
  const matchers = useKeyMatchers();

  const submit = useCallback(() => {
    const text = TB.getText(state);
    if (text.trim().length === 0) return;
    onSubmit(text);
    dispatch({ type: "reset" });
  }, [state, onSubmit]);

  const handleKey = useCallback(
    (key: Key): boolean | void => {
      // NEWLINE (any Enter with a modifier) — check *first* so Shift+Enter
      // doesn't race SUBMIT on terminals without kitty.
      if (matchers[Command.NEWLINE](key)) {
        dispatch({ type: "newline" });
        return true;
      }

      // SUBMIT: bare Enter.
      if (
        matchers[Command.SUBMIT](key) &&
        !key.shift && !key.alt && !key.ctrl && !key.cmd
      ) {
        if (TB.isEmpty(state) || TB.getText(state).trim().length === 0) {
          return true;
        }
        onSubmit(TB.getText(state));
        dispatch({ type: "reset" });
        return true;
      }

      // ----- Deletion -----
      if (matchers[Command.DELETE_WORD_BACKWARD](key)) {
        dispatch({ type: "delWordBack" });
        return true;
      }
      if (matchers[Command.DELETE_WORD_FORWARD](key)) {
        dispatch({ type: "delWordForward" });
        return true;
      }
      if (matchers[Command.DELETE_CHAR_LEFT](key)) {
        dispatch({ type: "backspace" });
        return true;
      }
      if (matchers[Command.DELETE_CHAR_RIGHT](key)) {
        dispatch({ type: "delete" });
        return true;
      }
      if (matchers[Command.KILL_LINE_LEFT](key)) {
        dispatch({ type: "killLeft" });
        return true;
      }
      if (matchers[Command.KILL_LINE_RIGHT](key)) {
        dispatch({ type: "killRight" });
        return true;
      }
      if (matchers[Command.CLEAR_INPUT](key)) {
        // ctrl+c on empty buffer → let it bubble for the quit handler.
        if (TB.isEmpty(state)) return;
        dispatch({ type: "clear" });
        return true;
      }

      // ----- Undo / Redo -----
      if (matchers[Command.UNDO](key)) {
        dispatch({ type: "undo" });
        return true;
      }
      if (matchers[Command.REDO](key)) {
        dispatch({ type: "redo" });
        return true;
      }

      // ----- Movement -----
      if (matchers[Command.MOVE_WORD_LEFT](key)) {
        dispatch({ type: "moveWordLeft" });
        return true;
      }
      if (matchers[Command.MOVE_WORD_RIGHT](key)) {
        dispatch({ type: "moveWordRight" });
        return true;
      }
      if (matchers[Command.MOVE_LEFT](key)) {
        dispatch({ type: "moveLeft" });
        return true;
      }
      if (matchers[Command.MOVE_RIGHT](key)) {
        dispatch({ type: "moveRight" });
        return true;
      }
      if (matchers[Command.MOVE_UP](key)) {
        dispatch({ type: "moveUp" });
        return true;
      }
      if (matchers[Command.MOVE_DOWN](key)) {
        dispatch({ type: "moveDown" });
        return true;
      }
      if (matchers[Command.HOME](key)) {
        dispatch({ type: "moveLineStart" });
        return true;
      }
      if (matchers[Command.END](key)) {
        dispatch({ type: "moveLineEnd" });
        return true;
      }
      if (matchers[Command.SCROLL_HOME](key)) {
        dispatch({ type: "moveBufferStart" });
        return true;
      }
      if (matchers[Command.SCROLL_END](key)) {
        dispatch({ type: "moveBufferEnd" });
        return true;
      }

      // ----- Paste (bracketed paste collapses to one event) -----
      if (key.name === "paste") {
        dispatch({ type: "insert", text: key.sequence });
        return true;
      }

      // ----- Fallback: printable character -----
      if (key.insertable && key.sequence) {
        dispatch({ type: "insert", text: key.sequence });
        return true;
      }

      return;
    },
    [state, matchers, onSubmit],
  );

  useKeypress(handleKey, { isActive: focused });

  return {
    state,
    submit,
    reset: () => dispatch({ type: "reset" }),
    insert: (text) => dispatch({ type: "insert", text }),
  };
}
