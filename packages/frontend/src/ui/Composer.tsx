// ---------------------------------------------------------------------------
// ui/Composer.tsx — bordered input box at the bottom of the TUI.
// Thin wrapper around <InputPrompt>. Corresponds roughly to gemini-cli's
// Composer.tsx but without mode indicators, status icons, or slash-menu.
// ---------------------------------------------------------------------------

import { Box } from "ink";
import { InputPrompt } from "./InputPrompt.tsx";

interface Props {
  focused: boolean;
  onSubmit: (text: string) => void;
  placeholder?: string;
}

export function Composer({
  focused,
  onSubmit,
  placeholder,
}: Props): React.JSX.Element {
  return (
    <Box
      borderStyle="round"
      borderColor={focused ? "cyan" : "gray"}
      paddingX={1}
      marginTop={1}
    >
      <InputPrompt
        focused={focused}
        onSubmit={onSubmit}
        placeholder={placeholder}
      />
    </Box>
  );
}
