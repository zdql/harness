// ---------------------------------------------------------------------------
// ui/App.tsx
//
// Top-level component. Wraps the layout in the KeypressProvider so stdin is
// captured, and in the KeyMatchersProvider so child components can look up
// matchers by Command. Wires the composer's onSubmit to a caller-provided
// handler (typically: push user item + fire off RPC call).
// ---------------------------------------------------------------------------

import { useEffect, useState } from "react";
import { useApp } from "ink";
import { KeypressProvider } from "../keypress/KeypressContext.tsx";
import {
  KeyMatchersProvider,
  useKeyMatchers,
} from "../keybindings/useKeyMatchers.tsx";
import { defaultKeyMatchers } from "../keybindings/keyMatchers.ts";
import { Command } from "../keybindings/keyBindings.ts";
import { useKeypress, type Key } from "../keypress/useKeypress.ts";
import { DefaultAppLayout } from "./DefaultAppLayout.tsx";

interface AppProps {
  onSubmit: (text: string) => void;
  placeholder?: string;
}

/** Global key handler: quit on ctrl+c when composer is empty, suspend on ctrl+z. */
function GlobalKeyHandler({ onExit }: { onExit: () => void }): null {
  const matchers = useKeyMatchers();
  useKeypress(
    (key: Key): boolean | void => {
      if (matchers[Command.QUIT](key)) {
        // Let InputPrompt swallow this when it has content; we only see it
        // if no higher-priority consumer returned true.
        onExit();
        return true;
      }
      return;
    },
    { isActive: true, priority: false },
  );
  return null;
}

export function App({ onSubmit, placeholder }: AppProps): React.JSX.Element {
  const ink = useApp();
  const [focused] = useState(true);

  useEffect(() => {
    // no-op: room for future startup effects
  }, []);

  return (
    <KeypressProvider>
      <KeyMatchersProvider value={defaultKeyMatchers}>
        <GlobalKeyHandler onExit={() => ink.exit()} />
        <DefaultAppLayout
          composerFocused={focused}
          onSubmit={onSubmit}
          placeholder={placeholder}
        />
      </KeyMatchersProvider>
    </KeypressProvider>
  );
}
