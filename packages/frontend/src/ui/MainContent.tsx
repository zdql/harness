// ---------------------------------------------------------------------------
// ui/MainContent.tsx
//
// The Static + pending split. This is the key Ink pattern for a scrollback
// TUI: items inside <Static> are rendered once, written to stdout, and live
// in the terminal's scrollback naturally. Only items below <Static>
// re-render on each update (the currently-streaming assistant message, etc.)
//
// Slimmed port of gemini-cli's MainContent.tsx (no virtualization, no
// alternate-buffer branch; we can add those later).
// ---------------------------------------------------------------------------

import { Static, Box } from "ink";
import { useHistory } from "../state/historyStore.ts";
import { HistoryItemDisplay } from "./HistoryItemDisplay.tsx";

export function MainContent(): React.JSX.Element {
  const { history, pendingHistoryItems, historyRemountKey } = useHistory();

  return (
    <>
      <Static key={historyRemountKey} items={history}>
        {(item) => <HistoryItemDisplay key={item.id} item={item} />}
      </Static>
      {pendingHistoryItems.length > 0 && (
        <Box flexDirection="column">
          {pendingHistoryItems.map((item) => (
            <HistoryItemDisplay key={item.id} item={item} />
          ))}
        </Box>
      )}
    </>
  );
}
