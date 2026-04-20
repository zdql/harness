// ---------------------------------------------------------------------------
// ui/hud/BranchDisplay.tsx — current git branch chip.
// ---------------------------------------------------------------------------

import { Text } from "ink";
import { useEffect, useState } from "react";
import type { RpcClient } from "../../rpc/client.ts";

interface Props {
  rpc: RpcClient;
}

export function BranchDisplay({ rpc }: Props): React.JSX.Element {
  const [branch, setBranch] = useState<string>("");

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const res = await rpc.call("hud.currentGitBranch.get", {});
      if (!cancelled && res.ok) setBranch(res.value.branch);
    })();
    return () => {
      cancelled = true;
    };
  }, [rpc]);

  return (
    <Text>
      <Text color="gray" dimColor>
        BRANCH{" "}
      </Text>
      <Text color="cyan" bold>
        {branch}
      </Text>
    </Text>
  );
}
