// ---------------------------------------------------------------------------
// hooks/mouse-filter.ts — Detect SGR mouse escape sequence fragments
//
// When ANSI mouse reporting is enabled, mouse events arrive as escape
// sequences like ESC[<64;101;31M. Ink's useInput strips the ESC byte and
// delivers the remainder as regular input, sometimes split across multiple
// events. This utility detects those fragments so input handlers can drop
// them.
// ---------------------------------------------------------------------------

/** Returns true if the input string is (or contains) an SGR mouse sequence fragment. */
export function isMouseSequence(input: string): boolean {
  // Full or partial fragment with bracket/angle prefix: [<64;54;36M or <64;54;36M
  if (/[\[<]\d+;\d+;\d+[Mm]/.test(input)) return true;
  // Fragment without prefix (split delivery): 64;54;36M
  if (/^\d+;\d+;\d+[Mm]/.test(input)) return true;
  // Arbitrary split: only contains chars from mouse sequences, with digits and semicolons
  if (/^[<;\dMm\[]+$/.test(input) && /\d/.test(input) && /[;]/.test(input)) return true;
  return false;
}
