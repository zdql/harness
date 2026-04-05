// ---------------------------------------------------------------------------
// terminal/controlSequences.ts
//
// Inlined from gemini-cli-core/src/utils/terminal.ts — tiny wrappers around
// stdout.write() for ANSI control sequences. Keeping these local eliminates
// the core-package dependency and makes what's happening obvious to readers.
// ---------------------------------------------------------------------------

function write(seq: string): void {
  process.stdout.write(seq);
}

// --- Mouse ----------------------------------------------------------------

export function enableMouseEvents(): void {
  // ?1002h = button-event tracking (clicks + drags + scroll)
  // ?1006h = SGR extended coordinates
  write("\x1b[?1002h\x1b[?1006h");
}

export function disableMouseEvents(): void {
  write("\x1b[?1006l\x1b[?1002l");
}

// --- Kitty keyboard protocol ----------------------------------------------

export function enableKittyKeyboardProtocol(): void {
  write("\x1b[>1u");
}

export function disableKittyKeyboardProtocol(): void {
  write("\x1b[<u");
}

// --- xterm modifyOtherKeys (fallback when kitty is absent) ----------------

export function enableModifyOtherKeys(): void {
  write("\x1b[>4;2m");
}

export function disableModifyOtherKeys(): void {
  write("\x1b[>4;0m");
}

// --- Bracketed paste ------------------------------------------------------

export function enableBracketedPasteMode(): void {
  write("\x1b[?2004h");
}

export function disableBracketedPasteMode(): void {
  write("\x1b[?2004l");
}

// --- Alt screen / line wrapping (unused in MVP but free to keep) ----------

export function enableLineWrapping(): void {
  write("\x1b[?7h");
}

export function disableLineWrapping(): void {
  write("\x1b[?7l");
}

export function enterAlternateScreen(): void {
  write("\x1b[?1049h");
}

export function exitAlternateScreen(): void {
  write("\x1b[?1049l");
}
