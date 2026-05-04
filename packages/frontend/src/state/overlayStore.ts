// ---------------------------------------------------------------------------
// state/overlayStore.ts
//
// A tiny store tracking which full-screen overlay (if any) is currently
// displayed over the normal chat layout. Overlays own keyboard focus and
// replace the composer while they're visible.
// ---------------------------------------------------------------------------

import { useSyncExternalStore } from "react";

export type Overlay = "none" | "settings" | "conversations";

type Listener = () => void;

function createStore() {
  let overlay: Overlay = "none";
  const listeners = new Set<Listener>();

  function emit(): void {
    for (const l of listeners) l();
  }

  return {
    get(): Overlay {
      return overlay;
    },
    set(next: Overlay): void {
      if (overlay === next) return;
      overlay = next;
      emit();
    },
    subscribe(l: Listener): () => void {
      listeners.add(l);
      return () => listeners.delete(l);
    },
  };
}

export const overlayStore = createStore();

export function useOverlay(): Overlay {
  return useSyncExternalStore(
    (l) => overlayStore.subscribe(l),
    () => overlayStore.get(),
    () => overlayStore.get(),
  );
}
