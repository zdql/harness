// ---------------------------------------------------------------------------
// ui/InputPrompt.tsx
//
// Multi-line, grapheme-aware, undoable input powered by `useTextBuffer`.
// The component itself only handles rendering — all key dispatch lives
// inside the hook, which calls `onSubmit` when the user hits Enter.
//
// Rendering model:
//   1. Lay out the logical lines into visual rows given the viewport width.
//   2. Render each visual row as its own <Text>; highlight the grapheme at
//      the cursor's (visualRow, visualCol) with `inverse`.
// ---------------------------------------------------------------------------

import { Box, Text, useStdout } from "ink";
import { useTextBuffer } from "../textbuffer/useTextBuffer.ts";
import { layoutLines, locateCursor } from "../textbuffer/layout.ts";

interface Props {
  focused: boolean;
  placeholder?: string;
  onSubmit: (text: string) => void;
}

export function InputPrompt({
  focused,
  placeholder = "",
  onSubmit,
}: Props): React.JSX.Element {
  const { state } = useTextBuffer({ focused, onSubmit });
  const { stdout } = useStdout();

  // Leave room for the "› " prompt (2 cols) and padding (2 cols).
  const termWidth = stdout?.columns ?? 80;
  const viewportWidth = Math.max(10, termWidth - 6);

  const visualRows = layoutLines(state.lines, viewportWidth);
  const { visualRow: cursorVR, visualCol: cursorVC } = locateCursor(
    visualRows,
    state.cursorRow,
    state.cursorCol,
  );

  // Empty-buffer placeholder.
  const totalGraphemes = state.lines.reduce((n, l) => n + l.length, 0);
  const showPlaceholder = totalGraphemes === 0 && placeholder.length > 0;

  return (
    <Box flexDirection="row">
      <Text color="cyan">{"› "}</Text>
      <Box flexDirection="column" flexGrow={1}>
        {showPlaceholder ? (
          <Text color="gray" dimColor>
            {placeholder}
          </Text>
        ) : (
          visualRows.map((vr, i) => (
            <VisualLine
              key={i}
              graphemes={vr.graphemes}
              cursorCol={i === cursorVR ? cursorVC : -1}
              focused={focused}
            />
          ))
        )}
      </Box>
    </Box>
  );
}

function VisualLine({
  graphemes,
  cursorCol,
  focused,
}: {
  graphemes: string[];
  cursorCol: number;
  focused: boolean;
}): React.JSX.Element {
  // No cursor on this row: render plain text (preserve an empty line's
  // height with a single space so Ink doesn't collapse it).
  if (cursorCol < 0) {
    return <Text>{graphemes.length === 0 ? " " : graphemes.join("")}</Text>;
  }

  // Cursor on this row: slice around the caret position.
  const before = graphemes.slice(0, cursorCol).join("");
  const at = graphemes[cursorCol] ?? " ";
  const after = graphemes.slice(cursorCol + 1).join("");

  return (
    <Text>
      {before}
      <Text inverse={focused}>{at}</Text>
      {after}
    </Text>
  );
}
