// ---------------------------------------------------------------------------
// keypress/useKeypress.ts
// Subscribe to Key events from KeypressProvider. Copied verbatim from
// gemini-cli (ui/hooks/useKeypress.ts); only the import path changed.
// ---------------------------------------------------------------------------

import { useEffect } from "react";
import {
  useKeypressContext,
  type KeypressHandler,
  type Key,
  type KeypressPriority,
} from "./KeypressContext.tsx";

export type { Key };

/**
 * A hook that listens for keypress events from stdin.
 *
 * @param onKeypress - Callback invoked for each Key event.
 * @param options.isActive - Subscribe only while true.
 * @param options.priority - Handlers with higher priority run first; returning
 *                           `true` from a handler stops propagation.
 */
export function useKeypress(
  onKeypress: KeypressHandler,
  {
    isActive,
    priority,
  }: { isActive: boolean; priority?: KeypressPriority | boolean },
): void {
  const { subscribe, unsubscribe } = useKeypressContext();

  useEffect(() => {
    if (!isActive) return;

    subscribe(onKeypress, priority);
    return () => {
      unsubscribe(onKeypress);
    };
  }, [isActive, onKeypress, subscribe, unsubscribe, priority]);
}
