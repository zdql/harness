// ---------------------------------------------------------------------------
// ui/hud/GithubDisplay.tsx — composes the git-related HUD chips inline.
// ---------------------------------------------------------------------------

import { Box, Text } from "ink";
import type { RpcClient } from "../../rpc/client.ts";
import { BranchDisplay } from "./BranchDisplay.tsx";
import { DiffDisplay } from "./DiffDisplay.tsx";

interface Props {
  rpc: RpcClient;
}

export function GithubDisplay({ rpc }: Props): React.JSX.Element {
  return (
    <Box flexDirection="row">
      <BranchDisplay rpc={rpc} />
      <Text color="gray" dimColor>
        {"  │  "}
      </Text>
      <DiffDisplay rpc={rpc} />
    </Box>
  );
}
