// ---------------------------------------------------------------------------
// textbuffer/textBuffer.ts
//
// A multi-line, grapheme-aware, undoable text buffer. Pure state + reducer;
// the React hook lives in useTextBuffer.ts.
//
// Model:
//   - `lines: string[][]` — each line is an array of grapheme clusters.
//     Storing graphemes (not code points or UTF-16 code units) is what makes
//     the cursor position correct for emoji, combining marks, CJK, flags.
//   - `cursorRow`, `cursorCol` — cursor is grapheme-indexed within its line.
//   - `undoStack` / `redoStack` — snapshots of {lines, cursor} taken before
//     mutations. Pushes coalesce adjacent inserts for a natural undo feel.
//
// All operations are pure functions of state. The reducer dispatches them.
// ---------------------------------------------------------------------------

// ===== Grapheme segmentation ==============================================

const segmenter = new Intl.Segmenter(undefined, { granularity: "grapheme" });

/** Split a string into grapheme clusters. "👋🏽abc" → ["👋🏽","a","b","c"]. */
export function toGraphemes(s: string): string[] {
  if (s.length === 0) return [];
  const out: string[] = [];
  for (const seg of segmenter.segment(s)) out.push(seg.segment);
  return out;
}

/** Join a line's grapheme array back to a string. */
export function lineToString(line: string[]): string {
  return line.join("");
}

// ===== State ==============================================================

export interface TextBufferState {
  lines: string[][]; // lines[row][col] = grapheme at (row, col)
  cursorRow: number;
  cursorCol: number;
  undoStack: Snapshot[];
  redoStack: Snapshot[];
  /** Classification of the last edit so adjacent inserts can coalesce. */
  lastEditKind: EditKind;
}

type EditKind = "none" | "insert-char" | "delete" | "structural";

interface Snapshot {
  lines: string[][];
  cursorRow: number;
  cursorCol: number;
}

export const EMPTY_STATE: TextBufferState = {
  lines: [[]],
  cursorRow: 0,
  cursorCol: 0,
  undoStack: [],
  redoStack: [],
  lastEditKind: "none",
};

const UNDO_LIMIT = 200;

function snapshot(s: TextBufferState): Snapshot {
  return {
    lines: s.lines.map((l) => l.slice()),
    cursorRow: s.cursorRow,
    cursorCol: s.cursorCol,
  };
}

function withSnapshot(
  s: TextBufferState,
  nextKind: EditKind,
): TextBufferState {
  // Coalesce consecutive inserts of single characters.
  const canCoalesce =
    nextKind === "insert-char" &&
    s.lastEditKind === "insert-char" &&
    s.undoStack.length > 0;
  if (canCoalesce) {
    return { ...s, redoStack: [], lastEditKind: nextKind };
  }
  const stack = [...s.undoStack, snapshot(s)];
  if (stack.length > UNDO_LIMIT) stack.shift();
  return { ...s, undoStack: stack, redoStack: [], lastEditKind: nextKind };
}

// ===== Construction =======================================================

export function createState(initial = ""): TextBufferState {
  if (initial === "") return EMPTY_STATE;
  const linesRaw = initial.split("\n");
  const lines = linesRaw.map(toGraphemes);
  const lastRow = lines.length - 1;
  return {
    lines,
    cursorRow: lastRow,
    cursorCol: lines[lastRow]!.length,
    undoStack: [],
    redoStack: [],
    lastEditKind: "none",
  };
}

export function getText(s: TextBufferState): string {
  return s.lines.map(lineToString).join("\n");
}

export function isEmpty(s: TextBufferState): boolean {
  return s.lines.length === 1 && s.lines[0]!.length === 0;
}

// ===== Cursor helpers =====================================================

function clampCursor(s: TextBufferState): TextBufferState {
  const row = Math.max(0, Math.min(s.cursorRow, s.lines.length - 1));
  const col = Math.max(0, Math.min(s.cursorCol, s.lines[row]!.length));
  if (row === s.cursorRow && col === s.cursorCol) return s;
  return { ...s, cursorRow: row, cursorCol: col };
}

// ===== Insert / newline ===================================================

/** Insert text at the cursor. Splits on "\n" to create new lines. */
export function insert(s: TextBufferState, text: string): TextBufferState {
  if (text === "") return s;
  const isSingleChar = text.length <= 2 && !text.includes("\n");
  const base = withSnapshot(s, isSingleChar ? "insert-char" : "structural");

  const parts = text.split("\n");
  const partsAsGraphemes = parts.map(toGraphemes);

  const curLine = base.lines[base.cursorRow]!;
  const before = curLine.slice(0, base.cursorCol);
  const after = curLine.slice(base.cursorCol);

  if (partsAsGraphemes.length === 1) {
    // Single-line insert.
    const inserted = partsAsGraphemes[0]!;
    const newLine = [...before, ...inserted, ...after];
    const newLines = base.lines.slice();
    newLines[base.cursorRow] = newLine;
    return {
      ...base,
      lines: newLines,
      cursorCol: base.cursorCol + inserted.length,
    };
  }

  // Multi-line insert.
  const first = partsAsGraphemes[0]!;
  const last = partsAsGraphemes[partsAsGraphemes.length - 1]!;
  const middle = partsAsGraphemes.slice(1, -1);

  const newLines = [
    ...base.lines.slice(0, base.cursorRow),
    [...before, ...first],
    ...middle,
    [...last, ...after],
    ...base.lines.slice(base.cursorRow + 1),
  ];
  return {
    ...base,
    lines: newLines,
    cursorRow: base.cursorRow + partsAsGraphemes.length - 1,
    cursorCol: last.length,
  };
}

export function insertNewline(s: TextBufferState): TextBufferState {
  return insert(s, "\n");
}

// ===== Deletion ===========================================================

export function deleteCharBackward(s: TextBufferState): TextBufferState {
  if (s.cursorRow === 0 && s.cursorCol === 0) return s;
  const base = withSnapshot(s, "delete");

  if (base.cursorCol === 0) {
    // Join with previous line.
    const prev = base.lines[base.cursorRow - 1]!;
    const cur = base.lines[base.cursorRow]!;
    const newLines = [
      ...base.lines.slice(0, base.cursorRow - 1),
      [...prev, ...cur],
      ...base.lines.slice(base.cursorRow + 1),
    ];
    return {
      ...base,
      lines: newLines,
      cursorRow: base.cursorRow - 1,
      cursorCol: prev.length,
    };
  }

  const cur = base.lines[base.cursorRow]!;
  const newLine = [
    ...cur.slice(0, base.cursorCol - 1),
    ...cur.slice(base.cursorCol),
  ];
  const newLines = base.lines.slice();
  newLines[base.cursorRow] = newLine;
  return { ...base, lines: newLines, cursorCol: base.cursorCol - 1 };
}

export function deleteCharForward(s: TextBufferState): TextBufferState {
  const cur = s.lines[s.cursorRow]!;
  const atEndOfLine = s.cursorCol >= cur.length;
  const atLastLine = s.cursorRow >= s.lines.length - 1;
  if (atEndOfLine && atLastLine) return s;
  const base = withSnapshot(s, "delete");

  if (atEndOfLine) {
    // Join next line up.
    const next = base.lines[base.cursorRow + 1]!;
    const newLines = [
      ...base.lines.slice(0, base.cursorRow),
      [...cur, ...next],
      ...base.lines.slice(base.cursorRow + 2),
    ];
    return { ...base, lines: newLines };
  }

  const newLine = [
    ...cur.slice(0, base.cursorCol),
    ...cur.slice(base.cursorCol + 1),
  ];
  const newLines = base.lines.slice();
  newLines[base.cursorRow] = newLine;
  return { ...base, lines: newLines };
}

// ===== Word boundaries ====================================================
// "Word" = run of word-characters (letters, digits, underscore, marks).

function isWordChar(g: string): boolean {
  // A grapheme's first code point tells us enough.
  return /^[\p{L}\p{N}\p{M}_]/u.test(g);
}

/** Return the grapheme column to the left of `col` at a previous word start. */
function prevWordBoundary(line: string[], col: number): number {
  let i = col;
  // Skip non-word runs (spaces, punct) leftward.
  while (i > 0 && !isWordChar(line[i - 1]!)) i--;
  // Then skip word chars leftward.
  while (i > 0 && isWordChar(line[i - 1]!)) i--;
  return i;
}

function nextWordBoundary(line: string[], col: number): number {
  let i = col;
  // Skip word chars rightward.
  while (i < line.length && isWordChar(line[i]!)) i++;
  // Then skip non-word runs rightward.
  while (i < line.length && !isWordChar(line[i]!)) i++;
  return i;
}

export function deleteWordBackward(s: TextBufferState): TextBufferState {
  if (s.cursorRow === 0 && s.cursorCol === 0) return s;
  const base = withSnapshot(s, "delete");
  const cur = base.lines[base.cursorRow]!;
  if (base.cursorCol === 0) {
    // At line start: delete the line break (same as backspace).
    return deleteCharBackward(s);
  }
  const to = prevWordBoundary(cur, base.cursorCol);
  const newLine = [...cur.slice(0, to), ...cur.slice(base.cursorCol)];
  const newLines = base.lines.slice();
  newLines[base.cursorRow] = newLine;
  return { ...base, lines: newLines, cursorCol: to };
}

export function deleteWordForward(s: TextBufferState): TextBufferState {
  const cur = s.lines[s.cursorRow]!;
  if (s.cursorCol >= cur.length) {
    if (s.cursorRow >= s.lines.length - 1) return s;
    return deleteCharForward(s);
  }
  const base = withSnapshot(s, "delete");
  const to = nextWordBoundary(cur, base.cursorCol);
  const newLine = [...cur.slice(0, base.cursorCol), ...cur.slice(to)];
  const newLines = base.lines.slice();
  newLines[base.cursorRow] = newLine;
  return { ...base, lines: newLines };
}

// ===== Line kill / clear ==================================================

export function killLineRight(s: TextBufferState): TextBufferState {
  const cur = s.lines[s.cursorRow]!;
  if (s.cursorCol >= cur.length) {
    // Nothing right of cursor on this line: join next line.
    if (s.cursorRow >= s.lines.length - 1) return s;
    return deleteCharForward(s);
  }
  const base = withSnapshot(s, "delete");
  const newLines = base.lines.slice();
  newLines[base.cursorRow] = cur.slice(0, base.cursorCol);
  return { ...base, lines: newLines };
}

export function killLineLeft(s: TextBufferState): TextBufferState {
  if (s.cursorCol === 0) return s;
  const base = withSnapshot(s, "delete");
  const cur = base.lines[base.cursorRow]!;
  const newLines = base.lines.slice();
  newLines[base.cursorRow] = cur.slice(base.cursorCol);
  return { ...base, lines: newLines, cursorCol: 0 };
}

export function clear(s: TextBufferState): TextBufferState {
  if (isEmpty(s)) return s;
  const base = withSnapshot(s, "structural");
  return { ...base, lines: [[]], cursorRow: 0, cursorCol: 0 };
}

// ===== Movement ===========================================================

export function moveLeft(s: TextBufferState): TextBufferState {
  if (s.cursorCol > 0)
    return { ...s, cursorCol: s.cursorCol - 1, lastEditKind: "none" };
  if (s.cursorRow > 0) {
    const prev = s.lines[s.cursorRow - 1]!;
    return {
      ...s,
      cursorRow: s.cursorRow - 1,
      cursorCol: prev.length,
      lastEditKind: "none",
    };
  }
  return s;
}

export function moveRight(s: TextBufferState): TextBufferState {
  const cur = s.lines[s.cursorRow]!;
  if (s.cursorCol < cur.length)
    return { ...s, cursorCol: s.cursorCol + 1, lastEditKind: "none" };
  if (s.cursorRow < s.lines.length - 1)
    return {
      ...s,
      cursorRow: s.cursorRow + 1,
      cursorCol: 0,
      lastEditKind: "none",
    };
  return s;
}

export function moveUp(s: TextBufferState): TextBufferState {
  if (s.cursorRow === 0) return s;
  const target = s.lines[s.cursorRow - 1]!;
  return {
    ...s,
    cursorRow: s.cursorRow - 1,
    cursorCol: Math.min(s.cursorCol, target.length),
    lastEditKind: "none",
  };
}

export function moveDown(s: TextBufferState): TextBufferState {
  if (s.cursorRow >= s.lines.length - 1) return s;
  const target = s.lines[s.cursorRow + 1]!;
  return {
    ...s,
    cursorRow: s.cursorRow + 1,
    cursorCol: Math.min(s.cursorCol, target.length),
    lastEditKind: "none",
  };
}

export function moveLineStart(s: TextBufferState): TextBufferState {
  return { ...s, cursorCol: 0, lastEditKind: "none" };
}

export function moveLineEnd(s: TextBufferState): TextBufferState {
  return {
    ...s,
    cursorCol: s.lines[s.cursorRow]!.length,
    lastEditKind: "none",
  };
}

export function moveWordLeft(s: TextBufferState): TextBufferState {
  const cur = s.lines[s.cursorRow]!;
  if (s.cursorCol === 0) return moveLeft(s);
  return {
    ...s,
    cursorCol: prevWordBoundary(cur, s.cursorCol),
    lastEditKind: "none",
  };
}

export function moveWordRight(s: TextBufferState): TextBufferState {
  const cur = s.lines[s.cursorRow]!;
  if (s.cursorCol >= cur.length) return moveRight(s);
  return {
    ...s,
    cursorCol: nextWordBoundary(cur, s.cursorCol),
    lastEditKind: "none",
  };
}

export function moveBufferStart(s: TextBufferState): TextBufferState {
  return { ...s, cursorRow: 0, cursorCol: 0, lastEditKind: "none" };
}

export function moveBufferEnd(s: TextBufferState): TextBufferState {
  const row = s.lines.length - 1;
  return {
    ...s,
    cursorRow: row,
    cursorCol: s.lines[row]!.length,
    lastEditKind: "none",
  };
}

// ===== Undo / Redo ========================================================

export function undo(s: TextBufferState): TextBufferState {
  if (s.undoStack.length === 0) return s;
  const prev = s.undoStack[s.undoStack.length - 1]!;
  const current = snapshot(s);
  return {
    lines: prev.lines,
    cursorRow: prev.cursorRow,
    cursorCol: prev.cursorCol,
    undoStack: s.undoStack.slice(0, -1),
    redoStack: [...s.redoStack, current],
    lastEditKind: "none",
  };
}

export function redo(s: TextBufferState): TextBufferState {
  if (s.redoStack.length === 0) return s;
  const next = s.redoStack[s.redoStack.length - 1]!;
  const current = snapshot(s);
  return {
    lines: next.lines,
    cursorRow: next.cursorRow,
    cursorCol: next.cursorCol,
    undoStack: [...s.undoStack, current],
    redoStack: s.redoStack.slice(0, -1),
    lastEditKind: "none",
  };
}

// Clamp for safety at the boundary of hook-driven updates.
export { clampCursor };
