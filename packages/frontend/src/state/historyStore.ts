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
// ---------------------------------------------------------------------------

import { useSyncExternalStore } from "react";
import type { HistoryItem } from "../ui/types.ts";

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

  return {
    getState: (): HistoryState => state,
    subscribe: (l: Listener): (() => void) => {
      listeners.add(l);
      return () => {
        listeners.delete(l);
      };
    },

    /** Append an item directly to committed history. */
    addItem(item: HistoryItemInput): number {
      const id = state.nextId;
      state = {
        ...state,
        history: [...state.history, { ...item, id } as HistoryItem],
        nextId: id + 1,
      };
      emit();
      return id;
    },

    /** Replace the pending list (use during streaming). */
    setPending(items: Array<HistoryItemInput>): void {
      const baseId = state.nextId;
      state = {
        ...state,
        pendingHistoryItems: items.map(
          (item, i) => ({ ...item, id: baseId + i }) as HistoryItem,
        ),
        nextId: baseId + items.length,
      };
      emit();
    },

    /** Commit all pending items into history and clear pending. */
    commitPending(): void {
      if (state.pendingHistoryItems.length === 0) return;
      state = {
        ...state,
        history: [...state.history, ...state.pendingHistoryItems],
        pendingHistoryItems: [],
      };
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
