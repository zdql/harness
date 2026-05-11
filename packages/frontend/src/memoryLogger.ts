// ---------------------------------------------------------------------------
// memoryLogger.ts
//
// Enabled by HARNESS_MEMORY_LOG=1 (set by `bin/harness --memory`).
// Every MEMORY_LOG_INTERVAL_MS, appends a CSV line to ~/.homebrewagent/memory.log:
//
//   iso_timestamp,bun_rss_kb,bun_heap_used_kb,bun_external_kb,server_rss_kb,history_len,pending_len
//
// Designed to be tail -f'd from another terminal. Doesn't touch stdout/stderr,
// so it won't disturb the TUI.
// ---------------------------------------------------------------------------

import { spawnSync } from "bun";
import { appendFile, mkdir } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import type { RpcClient } from "./rpc/client.ts";
import { historyStore } from "./state/historyStore.ts";

const MEMORY_LOG_INTERVAL_MS = 5_000;
const LOG_PATH = join(homedir(), ".homebrewagent", "memory.log");
const HEADER =
  "iso_timestamp,bun_rss_kb,bun_heap_used_kb,bun_external_kb,server_rss_kb,history_len,pending_len\n";

function rssKbForPid(pid: number | undefined): number {
  if (pid === undefined) return 0;
  try {
    const out = spawnSync(["ps", "-o", "rss=", "-p", String(pid)]);
    const text = new TextDecoder().decode(out.stdout).trim();
    const n = parseInt(text, 10);
    return Number.isFinite(n) ? n : 0;
  } catch {
    return 0;
  }
}

export async function startMemoryLogger(rpc: RpcClient): Promise<void> {
  await mkdir(dirname(LOG_PATH), { recursive: true });
  await appendFile(LOG_PATH, HEADER);

  const tick = async (): Promise<void> => {
    const mem = process.memoryUsage();
    const bunRssKb = Math.round(mem.rss / 1024);
    const heapKb = Math.round(mem.heapUsed / 1024);
    const extKb = Math.round((mem.external ?? 0) / 1024);
    const serverRssKb = rssKbForPid(rpc.serverPid);
    const { history, pendingHistoryItems } = historyStore.getState();
    const line =
      [
        new Date().toISOString(),
        bunRssKb,
        heapKb,
        extKb,
        serverRssKb,
        history.length,
        pendingHistoryItems.length,
      ].join(",") + "\n";
    try {
      await appendFile(LOG_PATH, line);
    } catch {
      // Swallow — logging must never break the TUI.
    }
  };

  void tick();
  setInterval(() => void tick(), MEMORY_LOG_INTERVAL_MS);
}
