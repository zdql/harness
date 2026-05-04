// ---------------------------------------------------------------------------
// ui/hud/ModelDisplay.tsx — current model chip.
//
// Shows the model the active conversation will use on its next send.
// Polls `hud.currentModel.get` so that running `/model …`, `/clear`, or
// editing settings is reflected without restarting the TUI.
// ---------------------------------------------------------------------------

import { Text } from "ink";
import { useEffect, useState } from "react";
import type { RpcClient } from "../../rpc/client.ts";

const MODEL_REFRESH_MS = 5_000;

interface Props {
  rpc: RpcClient;
}

/** Strip the provider prefix so the chip stays compact in narrow terminals. */
function shortName(model: string): string {
  const slash = model.indexOf("/");
  return slash === -1 ? model : model.slice(slash + 1);
}

export function ModelDisplay({ rpc }: Props): React.JSX.Element {
  const [model, setModel] = useState<string>("");

  useEffect(() => {
    let cancelled = false;

    async function refresh(): Promise<void> {
      const res = await rpc.call("hud.currentModel.get", {});
      if (!cancelled && res.ok) setModel(res.value.model);
    }

    void refresh();
    const t = setInterval(() => void refresh(), MODEL_REFRESH_MS);
    return () => {
      cancelled = true;
      clearInterval(t);
    };
  }, [rpc]);

  return (
    <Text>
      <Text color="gray" dimColor>
        MODEL{" "}
      </Text>
      <Text color="green" bold>
        {model ? shortName(model) : "…"}
      </Text>
    </Text>
  );
}
