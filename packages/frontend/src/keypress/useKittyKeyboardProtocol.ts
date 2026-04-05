// ---------------------------------------------------------------------------
// keypress/useKittyKeyboardProtocol.ts
// Exposes the cached Kitty protocol status to components. Detection runs
// once at app startup; this hook just reads the flag.
// ---------------------------------------------------------------------------

import { useState } from "react";
import { terminalCapabilityManager } from "../terminal/terminalCapabilityManager.ts";

export interface KittyProtocolStatus {
  enabled: boolean;
  checking: boolean;
}

export function useKittyKeyboardProtocol(): KittyProtocolStatus {
  const [status] = useState<KittyProtocolStatus>({
    enabled: terminalCapabilityManager.isKittyProtocolEnabled(),
    checking: false,
  });
  return status;
}
