// ---------------------------------------------------------------------------
// textbuffer/layout.ts
//
// Compute visual rows from logical lines given a viewport width, respecting
// grapheme widths (so CJK/emoji take the right number of columns). This
// lets us place the cursor at the correct visual column after wrap.
// ---------------------------------------------------------------------------

import stringWidth from "string-width";

export interface VisualRow {
  /** Which logical line (state.lines[row]) this visual row belongs to. */
  logicalRow: number;
  /** Grapheme index on the logical line where this visual row begins. */
  startCol: number;
  /** Graphemes shown on this visual row. */
  graphemes: string[];
}

/** Measure grapheme width with a floor of 1 for combining/zero-width chars. */
function widthOf(g: string): number {
  const w = stringWidth(g);
  return w <= 0 ? 1 : w;
}

/**
 * Split logical lines into visual rows that each fit within `viewportWidth`
 * columns. Empty logical lines produce one empty visual row (so the cursor
 * has somewhere to land).
 */
export function layoutLines(
  lines: string[][],
  viewportWidth: number,
): VisualRow[] {
  const width = Math.max(1, viewportWidth);
  const out: VisualRow[] = [];

  for (let row = 0; row < lines.length; row++) {
    const line = lines[row]!;
    if (line.length === 0) {
      out.push({ logicalRow: row, startCol: 0, graphemes: [] });
      continue;
    }

    let start = 0;
    let batch: string[] = [];
    let w = 0;

    for (let col = 0; col < line.length; col++) {
      const g = line[col]!;
      const gw = widthOf(g);
      if (batch.length > 0 && w + gw > width) {
        out.push({ logicalRow: row, startCol: start, graphemes: batch });
        batch = [];
        w = 0;
        start = col;
      }
      batch.push(g);
      w += gw;
    }
    out.push({ logicalRow: row, startCol: start, graphemes: batch });
  }

  return out;
}

/**
 * Find the visual row index + column where the cursor at (logicalRow, col)
 * should render. When the cursor sits exactly at a wrap point, it appears
 * at the start of the *next* visual row (matches standard editor behavior).
 */
export function locateCursor(
  visual: VisualRow[],
  logicalRow: number,
  col: number,
): { visualRow: number; visualCol: number } {
  for (let i = 0; i < visual.length; i++) {
    const vr = visual[i]!;
    if (vr.logicalRow !== logicalRow) continue;
    const end = vr.startCol + vr.graphemes.length;

    // Belongs to this visual row if col is inside [start, end]. At col===end,
    // prefer the next visual row of the same logical row (if one exists) so
    // the cursor sits at the start of the wrapped continuation.
    if (col >= vr.startCol && col <= end) {
      const hasContinuation =
        i + 1 < visual.length && visual[i + 1]!.logicalRow === logicalRow;
      if (col === end && hasContinuation) continue;
      return { visualRow: i, visualCol: col - vr.startCol };
    }
  }
  return { visualRow: 0, visualCol: 0 };
}
