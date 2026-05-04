// ---------------------------------------------------------------------------
// ui/DefaultAppLayout.tsx
//
// The full visible shell. One vertical <Box> containing MainContent (the
// scrollback) on top, and the Composer (bottom input) below. Dialogs/modals
// would swap in at the Composer's slot by gating the prop tree.
//
// Port of gemini-cli's DefaultAppLayout.tsx, stripped of background-task
// panel, notifications, alternate-buffer logic, and copy-mode warnings.
// ---------------------------------------------------------------------------

import { Box } from "ink";
import { MainContent } from "./MainContent.tsx";
import { Composer } from "./Composer.tsx";
import { SettingsView } from "./SettingsView.tsx";
import { ConversationSelector } from "./ConversationSelector.tsx";
import { useOverlay } from "../state/overlayStore.ts";
import type { RpcClient } from "../rpc/client.ts";

interface Props {
  composerFocused: boolean;
  onSubmit: (text: string) => void;
  placeholder?: string;
  rpc: RpcClient;
}

export function DefaultAppLayout({
  composerFocused,
  onSubmit,
  placeholder,
  rpc,
}: Props): React.JSX.Element {
  const overlay = useOverlay();

  if (overlay === "settings") {
    return (
      <Box flexDirection="column">
        <MainContent />
        <SettingsView rpc={rpc} />
      </Box>
    );
  }

  if (overlay === "conversations") {
    return (
      <Box flexDirection="column">
        <MainContent />
        <ConversationSelector rpc={rpc} />
      </Box>
    );
  }

  return (
    <Box flexDirection="column">
      <MainContent />
      <Composer
        rpc={rpc}
        focused={composerFocused}
        onSubmit={onSubmit}
        placeholder={placeholder}
      />
    </Box>
  );
}
