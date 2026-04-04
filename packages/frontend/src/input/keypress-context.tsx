// ---------------------------------------------------------------------------
// input/keypress-context.tsx — React context for keyboard input
//
// Adapted from Gemini CLI's KeypressContext.tsx.
// Provides a priority-based keypress dispatch system. Components subscribe
// via useKeypress(); the highest-priority handler that returns `true`
// consumes the event.
// ---------------------------------------------------------------------------

import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
} from "react";
import { useStdin } from "ink";
import { createInputPipeline, type Key, type KeypressHandler } from "./keys.ts";
import { terminalCapabilityManager } from "./terminal-capability.ts";

// ---- Priority levels -------------------------------------------------------

export enum KeypressPriority {
  Low = -100,
  Normal = 0,
  High = 100,
  Critical = 200,
}

// ---- Context ---------------------------------------------------------------

interface KeypressContextValue {
  subscribe: (handler: KeypressHandler, priority?: KeypressPriority) => void;
  unsubscribe: (handler: KeypressHandler) => void;
}

const KeypressCtx = createContext<KeypressContextValue | undefined>(undefined);

export function useKeypressContext() {
  const ctx = useContext(KeypressCtx);
  if (!ctx) throw new Error("useKeypressContext requires KeypressProvider");
  return ctx;
}

// ---- Provider --------------------------------------------------------------

export function KeypressProvider({ children }: { children: React.ReactNode }) {
  const { stdin, setRawMode } = useStdin();

  // Map handler → priority, and multimap priority → Set<handler>
  const handlerPriority = useRef(new Map<KeypressHandler, number>()).current;
  const priorityHandlers = useRef(
    new Map<number, Set<KeypressHandler>>(),
  ).current;
  const sortedPriorities = useRef<number[]>([]);

  const subscribe = useCallback(
    (handler: KeypressHandler, priority: KeypressPriority = KeypressPriority.Normal) => {
      handlerPriority.set(handler, priority);
      let set = priorityHandlers.get(priority);
      if (!set) {
        set = new Set();
        priorityHandlers.set(priority, set);
        sortedPriorities.current = Array.from(priorityHandlers.keys()).sort(
          (a, b) => b - a,
        );
      }
      set.add(handler);
    },
    [handlerPriority, priorityHandlers],
  );

  const unsubscribe = useCallback(
    (handler: KeypressHandler) => {
      const p = handlerPriority.get(handler);
      if (p === undefined) return;
      handlerPriority.delete(handler);
      const set = priorityHandlers.get(p);
      if (set) {
        set.delete(handler);
        if (set.size === 0) {
          priorityHandlers.delete(p);
          sortedPriorities.current = Array.from(priorityHandlers.keys()).sort(
            (a, b) => b - a,
          );
        }
      }
    },
    [handlerPriority, priorityHandlers],
  );

  const broadcast = useCallback(
    (key: Key) => {
      for (const p of sortedPriorities.current) {
        const set = priorityHandlers.get(p);
        if (!set) continue;
        // Within a priority level: last subscribed handles first (stack)
        for (const handler of Array.from(set).reverse()) {
          if (handler(key) === true) return;
        }
      }
    },
    [priorityHandlers],
  );

  useEffect(() => {
    // Enable whatever the terminal supports (detected before render)
    terminalCapabilityManager.enableSupportedModes();

    const wasRaw = stdin.isRaw;
    if (!wasRaw) setRawMode(true);

    process.stdin.setEncoding("utf8");

    const kittyEnabled = terminalCapabilityManager.isKittyProtocolEnabled();
    const dataListener = createInputPipeline(broadcast, kittyEnabled);
    stdin.on("data", dataListener);

    return () => {
      stdin.removeListener("data", dataListener);
      if (!wasRaw) setRawMode(false);
      terminalCapabilityManager.disableModes();
    };
  }, [stdin, setRawMode, broadcast]);

  const value = useMemo(
    () => ({ subscribe, unsubscribe }),
    [subscribe, unsubscribe],
  );

  return (
    <KeypressCtx.Provider value={value}>{children}</KeypressCtx.Provider>
  );
}

// ---- useKeypress hook ------------------------------------------------------

/**
 * Subscribe to keypress events with optional priority.
 * Return `true` from the handler to consume the event.
 */
export function useKeypress(
  handler: KeypressHandler,
  opts: { isActive: boolean; priority?: KeypressPriority },
) {
  const { subscribe, unsubscribe } = useKeypressContext();

  useEffect(() => {
    if (!opts.isActive) return;
    subscribe(handler, opts.priority);
    return () => {
      unsubscribe(handler);
    };
  }, [opts.isActive, handler, subscribe, unsubscribe, opts.priority]);
}
