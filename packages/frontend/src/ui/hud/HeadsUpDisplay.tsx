// ---------------------------------------------------------------------------
// ui/hud/HeadsUpDisplay.tsx — single-row status line rendered inside the
// composer's border frame. Chips are separated by a dimmed vertical bar.
// ---------------------------------------------------------------------------

import { Box, Text } from "ink";
import { GithubDisplay } from "./GithubDisplay.tsx";
import { ContextDisplay } from "./ContextDisplay.tsx";
import type { RpcClient } from "../../rpc/client.ts";

interface Props {
  rpc: RpcClient;
}

export function HeadsUpDisplay({ rpc }: Props): React.JSX.Element {
  return (
    <Box flexDirection="row" flexShrink={0}>
      <GithubDisplay rpc={rpc} />
      <Text color="gray" dimColor>
        {"  │  "}
      </Text>
      <ContextDisplay rpc={rpc} />
    </Box>
  );
}
