// ---------------------------------------------------------------------------
// index.tsx — Entry point
//
// Enters the alternate screen buffer (like vim/htop) so we own the full
// terminal, then renders the Ink app.
// ---------------------------------------------------------------------------

import React from "react";
import { render } from "ink";
import { App } from "./app.tsx";

const enterAltScreen = "\x1b[?1049h";
const leaveAltScreen = "\x1b[?1049l";

// Enter alternate screen before Ink starts
process.stdout.write(enterAltScreen);

// Clean up on exit
function cleanup() {
  process.stdout.write(leaveAltScreen);
}
process.on("exit", cleanup);
process.on("SIGINT", () => { cleanup(); process.exit(0); });
process.on("SIGTERM", () => { cleanup(); process.exit(0); });

render(<App />);
