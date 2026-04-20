// ---------------------------------------------------------------------------
// ui/hud/ContextDisplay.tsx — in-context token estimate chip.
// Polls `hud.contextTokens.get` — the backend owns the char→token estimate.
// ---------------------------------------------------------------------------

import { Text } from "ink";
import { useEffect, useState } from "react";
import type { RpcClient } from "../../rpc/client.ts";

const CONTEXT_REFRESH_MS = 10_000;

interface Props {
  rpc: RpcClient;
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return `${n}`;
}

export function ContextDisplay({ rpc }: Props): React.JSX.Element {
  const [tokens, setTokens] = useState<number>(0);

  useEffect(() => {
    let cancelled = false;

    async function refresh(): Promise<void> {
      const res = await rpc.call("hud.contextTokens.get", {});
      if (!cancelled && res.ok) setTokens(res.value.tokens);
    }

    void refresh();
    const t = setInterval(() => void refresh(), CONTEXT_REFRESH_MS);
    return () => {
      cancelled = true;
      clearInterval(t);
    };
  }, [rpc]);

  return (
    <Text>
      <Text color="gray" dimColor>
        CTX{" "}
      </Text>
      <Text color="magenta" bold>
        ~{formatTokens(tokens)}
      </Text>
    </Text>
  );
}
