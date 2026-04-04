// ---------------------------------------------------------------------------
// index.tsx — Entry point
//
// 1. Detect terminal capabilities (Kitty protocol) BEFORE Ink starts
// 2. Enter alternate screen buffer
// 3. Render the Ink app
//
// Capability detection must happen first because it probes stdin directly.
// Once Ink calls render(), it takes over stdin management.
// ---------------------------------------------------------------------------

import React from "react";
import { render } from "ink";
import { App } from "./app.tsx";
import { terminalCapabilityManager } from "./input/index.ts";

const enterAltScreen = "\x1b[?1049h";
const leaveAltScreen = "\x1b[?1049l";

async function main() {
  // Step 1: Detect terminal capabilities on a clean stdin
  await terminalCapabilityManager.detectCapabilities();

  // Step 2: Enter alternate screen
  process.stdout.write(enterAltScreen);

  function cleanup() {
    process.stdout.write(leaveAltScreen);
  }
  process.on("exit", cleanup);
  process.on("SIGINT", () => { cleanup(); process.exit(0); });
  process.on("SIGTERM", () => { cleanup(); process.exit(0); });

  // Step 3: Render — no kittyKeyboard option; we manage protocols ourselves
  render(<App />, {
    exitOnCtrlC: false,
  });
}

main();
