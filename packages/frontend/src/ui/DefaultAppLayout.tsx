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

interface Props {
  composerFocused: boolean;
  onSubmit: (text: string) => void;
  placeholder?: string;
}

export function DefaultAppLayout({
  composerFocused,
  onSubmit,
  placeholder,
}: Props): React.JSX.Element {
  return (
    <Box flexDirection="column">
      <MainContent />
      <Composer
        focused={composerFocused}
        onSubmit={onSubmit}
        placeholder={placeholder}
      />
    </Box>
  );
}
