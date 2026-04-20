// ---------------------------------------------------------------------------
// ui/Composer.tsx — bordered input box at the bottom of the TUI.
// Thin wrapper around <InputPrompt>. Corresponds roughly to gemini-cli's
// Composer.tsx but without mode indicators, status icons, or slash-menu.
// ---------------------------------------------------------------------------

import { Box } from "ink";
import { InputPrompt } from "./InputPrompt.tsx";
import { HeadsUpDisplay } from "./hud/HeadsUpDisplay.tsx";
import type { RpcClient } from "../rpc/client.ts";

interface Props {
  focused: boolean;
  onSubmit: (text: string) => void;
  placeholder?: string;
  rpc: RpcClient;
}

export function Composer({
  focused,
  onSubmit,
  placeholder,
  rpc,
}: Props): React.JSX.Element {
  return (
    <Box flexDirection="column" marginTop={1}>
      <Box
        borderStyle="round"
        borderColor={focused ? "cyan" : "gray"}
        paddingX={1}
        flexDirection="column"
      >
        <HeadsUpDisplay rpc={rpc} />
        <InputPrompt
          focused={focused}
          onSubmit={onSubmit}
          placeholder={placeholder}
        />
      </Box>
    </Box>
  );
}
