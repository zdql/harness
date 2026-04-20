// ---------------------------------------------------------------------------
// ui/hud/DiffDisplay.tsx — working-tree diff chip.
// Polls `hud.diffCounts.get` and renders +/- line counts.
// ---------------------------------------------------------------------------

import { Text } from "ink";
import { useEffect, useState } from "react";
import type { RpcClient } from "../../rpc/client.ts";

const DIFF_REFRESH_MS = 10_000;

interface Props {
  rpc: RpcClient;
}

export function DiffDisplay({ rpc }: Props): React.JSX.Element {
  const [counts, setCounts] = useState<{ added: number; removed: number }>({
    added: 0,
    removed: 0,
  });

  useEffect(() => {
    let cancelled = false;

    async function refresh(): Promise<void> {
      const res = await rpc.call("hud.diffCounts.get", {});
      if (!cancelled && res.ok) {
        setCounts({ added: res.value.added, removed: res.value.removed });
      }
    }

    void refresh();
    const t = setInterval(() => void refresh(), DIFF_REFRESH_MS);
    return () => {
      cancelled = true;
      clearInterval(t);
    };
  }, [rpc]);

  const clean = counts.added === 0 && counts.removed === 0;

  return (
    <Text>
      <Text color="gray" dimColor>
        DIFF{" "}
      </Text>
      {clean ? (
        <Text color="gray">clean</Text>
      ) : (
        <Text>
          <Text color="green">+{counts.added}</Text>
          <Text> </Text>
          <Text color="red">-{counts.removed}</Text>
        </Text>
      )}
    </Text>
  );
}
