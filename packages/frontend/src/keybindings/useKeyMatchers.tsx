// ---------------------------------------------------------------------------
// keybindings/useKeyMatchers.tsx
//
// React context + hook for retrieving the active KeyMatchers. If no provider
// is mounted, falls back to `defaultKeyMatchers` so tests and simple apps
// work without explicit setup.
//
// Copied verbatim from gemini-cli (ui/hooks/useKeyMatchers.tsx).
// ---------------------------------------------------------------------------

import type React from "react";
import { createContext, useContext } from "react";
import { defaultKeyMatchers, type KeyMatchers } from "./keyMatchers.ts";

export const KeyMatchersContext =
  createContext<KeyMatchers>(defaultKeyMatchers);

export const KeyMatchersProvider = ({
  children,
  value,
}: {
  children: React.ReactNode;
  value: KeyMatchers;
}): React.JSX.Element => (
  <KeyMatchersContext.Provider value={value}>
    {children}
  </KeyMatchersContext.Provider>
);

export function useKeyMatchers(): KeyMatchers {
  return useContext(KeyMatchersContext);
}
