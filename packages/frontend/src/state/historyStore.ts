// ---------------------------------------------------------------------------
// state/historyStore.ts
//
// A tiny external store with useSyncExternalStore. Owns the three fields
// that MainContent needs to run the <Static>/pending split:
//
//   history              — committed items, rendered inside <Static>
//   pendingHistoryItems  — streaming items, rendered below <Static>
//   historyRemountKey    — bump to force <Static> to re-flush scrollback
//
// Commit semantics: when streaming finishes, move pending items into history
// with `commitPending()`. That's what makes them "stick" to scrollback.
//
// Tool results are truncated at the store boundary (see
// MAX_TOOL_RESULT_CHARS) so large payloads never enter the history array.
// In-place compaction bounds the number of live items: once the committed
// history exceeds MAX_HISTORY_SIZE, old entries are replaced with tiny
// "evicted" stubs.  The array length never shrinks so Ink's <Static>
// (which tracks rendered items by array index) stays in sync, while the
// per-item payloads on evicted slots are freed.
// ---------------------------------------------------------------------------

import { useSyncExternalStore } from "react";
import type { HistoryItem } from "../ui/types.ts";

/**
 * Maximum number of *non-evicted* committed history items retained.
 * Once exceeded, the oldest entries are replaced in-place with
 * `{ id, type: "evicted" }` stubs, releasing their payloads.
 */
export const MAX_HISTORY_SIZE = 1000;

/**
 * Maximum character length for a tool result after whitespace
 * normalization.  Anything longer is truncated and an ellipsis is
 * appended.  This is applied at store time so the full payload never
 * enters the history array.
 */
export const MAX_TOOL_RESULT_CHARS = 120;

/**
 * Collapse whitespace to single spaces, trim, and hard-cap at
 * MAX_TOOL_RESULT_CHARS.  If the original (after collapse) exceeded
 * the cap, "…" is appended so the display knows it was truncated
 * without needing a separate flag.
 */
export function truncateToolResult(raw: string): string {
  const oneLine = raw.replace(/\s+/g, " ").trim();
  if (oneLine.length <= MAX_TOOL_RESULT_CHARS) return oneLine;
  return oneLine.slice(0, MAX_TOOL_RESULT_CHARS) + "…";
}

/**
 * Distributive Omit over a discriminated union. The built-in `Omit<U, K>`
 * collapses to the common keys only; this preserves each variant.
 */
type DistributiveOmit<T, K extends PropertyKey> = T extends unknown
  ? Omit<T, K>
  : never;

export type HistoryItemInput = DistributiveOmit<HistoryItem, "id">;

interface HistoryState {
  history: HistoryItem[];
  pendingHistoryItems: HistoryItem[];
  historyRemountKey: number;
  nextId: number;
}

type Listener = () => void;

function createStore() {
  let state: HistoryState = {
    history: [],
    pendingHistoryItems: [],
    historyRemountKey: 0,
    nextId: 1,
  };
  const listeners = new Set<Listener>();

  function emit(): void {
    for (const l of listeners) l();
  }

  /** Compact old history entries if non-evicted count exceeds the cap. */
  function maybeCompact(): void {
    let liveCount = 0;
    for (const item of state.history) {
      if (item.type !== "evicted") liveCount++;
    }
    if (liveCount <= MAX_HISTORY_SIZE) return;

    // Evict the oldest entries to bring liveCount down to MAX_HISTORY_SIZE.
    const excess = liveCount - MAX_HISTORY_SIZE;
    let evicted = 0;
    for (let i = 0; i < state.history.length && evicted < excess; i++) {
      const item = state.history[i]!;
      if (item.type !== "evicted") {
        state.history[i] = { id: item.id, type: "evicted" };
        evicted++;
      }
    }
  }

  return {
    getState: (): HistoryState => state,
    subscribe: (l: Listener): (() => void) => {
      listeners.add(l);
      return () => {
        listeners.delete(l);
      };
    },

    /** Append an item directly to committed history, compacting old entries if needed. */
    addItem(item: HistoryItemInput): number {
      const id = state.nextId;
      const normalized: HistoryItemInput =
        item.type === "tool"
          ? { ...item, result: truncateToolResult(item.result) }
          : item;
      state = {
        ...state,
        history: [...state.history, { ...normalized, id } as HistoryItem],
        nextId: id + 1,
      };
      maybeCompact();
      emit();
      return id;
    },

    /** Replace the pending list (use during streaming). */
    setPending(items: Array<HistoryItemInput>): void {
      const baseId = state.nextId;
      state = {
        ...state,
        pendingHistoryItems: items.map((item, i) => {
          const normalized =
            item.type === "tool"
              ? { ...item, result: truncateToolResult(item.result) }
              : item;
          return { ...normalized, id: baseId + i } as HistoryItem;
        }),
        nextId: baseId + items.length,
      };
      emit();
    },

    /** Commit all pending items into history and clear pending, compacting old entries if needed. */
    commitPending(): void {
      if (state.pendingHistoryItems.length === 0) return;
      state = {
        ...state,
        history: [...state.history, ...state.pendingHistoryItems],
        pendingHistoryItems: [],
      };
      maybeCompact();
      emit();
    },

    /** Clear everything and force a <Static> remount. */
    clear(): void {
      state = {
        ...state,
        history: [],
        pendingHistoryItems: [],
        historyRemountKey: state.historyRemountKey + 1,
      };
      emit();
    },
  };
}

export const historyStore = createStore();

export function useHistory(): HistoryState {
  return useSyncExternalStore(historyStore.subscribe, historyStore.getState);
}
