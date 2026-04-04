// ---------------------------------------------------------------------------
// input/terminal-capability.ts — Terminal capability detection & protocol mgmt
//
// Adapted from Gemini CLI's terminalCapabilityManager.ts.
// Probes the terminal at startup to detect Kitty keyboard protocol support,
// then enables the best available input protocol.
// Must be called BEFORE Ink's render() so detection happens on a clean stdin.
// ---------------------------------------------------------------------------

import * as fs from "node:fs";

// ---- Escape sequences for enabling/disabling protocols --------------------

const ENABLE_KITTY = "\x1b[>1u";
const DISABLE_KITTY = "\x1b[<u";
const ENABLE_BRACKETED_PASTE = "\x1b[?2004h";
const DISABLE_BRACKETED_PASTE = "\x1b[?2004l";

const TERMINAL_CLEANUP_SEQUENCE =
  "\x1b[<u\x1b[?2004l\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l";

function cleanupTerminalOnExit() {
  try {
    if (process.stdout?.fd !== undefined) {
      fs.writeSync(process.stdout.fd, TERMINAL_CLEANUP_SEQUENCE);
      return;
    }
  } catch {
    // ignore
  }
  process.stdout.write(DISABLE_KITTY);
  process.stdout.write(DISABLE_BRACKETED_PASTE);
}

// ---- Detection queries & response patterns --------------------------------

const KITTY_QUERY = "\x1b[?u";
const DEVICE_ATTRIBUTES_QUERY = "\x1b[c";
const HIDDEN_MODE = "\x1b[8m";
const CLEAR_LINE_AND_RETURN = "\x1b[2K\r";
const RESET_ATTRIBUTES = "\x1b[0m";

// eslint-disable-next-line no-control-regex
const KITTY_REGEX = /\x1b\[\?(\d+)u/;
// eslint-disable-next-line no-control-regex
const DEVICE_ATTRIBUTES_REGEX = /\x1b\[\?(\d+)(;\d+)*c/;

// ---- Singleton manager ----------------------------------------------------

class TerminalCapabilityManager {
  private detectionComplete = false;
  private kittySupported = false;
  private _kittyEnabled = false;

  /**
   * Probes the terminal for capability support.
   * Must be called BEFORE Ink's render() while stdin is uncontested.
   * Sends a Kitty query + Device Attributes sentinel, waits up to 1s
   * for responses, then records what the terminal supports.
   */
  async detectCapabilities(): Promise<void> {
    if (this.detectionComplete) return;

    if (!process.stdin.isTTY || !process.stdout.isTTY) {
      this.detectionComplete = true;
      return;
    }

    // Register cleanup handlers
    process.off("exit", cleanupTerminalOnExit);
    process.off("SIGTERM", cleanupTerminalOnExit);
    process.off("SIGINT", cleanupTerminalOnExit);
    process.on("exit", cleanupTerminalOnExit);
    process.on("SIGTERM", cleanupTerminalOnExit);
    process.on("SIGINT", cleanupTerminalOnExit);

    return new Promise((resolve) => {
      const originalRawMode = process.stdin.isRaw;
      if (!originalRawMode) {
        process.stdin.setRawMode(true);
      }

      let buffer = "";
      let kittyReceived = false;
      let daReceived = false;
      // eslint-disable-next-line prefer-const
      let timeoutId: ReturnType<typeof setTimeout>;

      const cleanup = () => {
        if (timeoutId) clearTimeout(timeoutId);
        process.stdin.removeListener("data", onData);
        if (!originalRawMode) {
          process.stdin.setRawMode(false);
        }
        this.detectionComplete = true;
        resolve();
      };

      timeoutId = setTimeout(cleanup, 1000);

      const onData = (data: Buffer) => {
        buffer += data.toString();

        if (!kittyReceived && KITTY_REGEX.test(buffer)) {
          kittyReceived = true;
          this.kittySupported = true;
        }

        // DA1 response is our sentinel — terminal has processed all queries
        if (!daReceived) {
          if (DEVICE_ATTRIBUTES_REGEX.test(buffer)) {
            daReceived = true;
            cleanup();
          }
        }
      };

      process.stdin.on("data", onData);

      try {
        fs.writeSync(
          process.stdout.fd,
          HIDDEN_MODE +
            KITTY_QUERY +
            DEVICE_ATTRIBUTES_QUERY +
            CLEAR_LINE_AND_RETURN +
            RESET_ATTRIBUTES,
        );
      } catch {
        cleanup();
      }
    });
  }

  /**
   * Enables the best available keyboard protocol based on detection results.
   * Called from inside KeypressProvider after Ink has started.
   */
  enableSupportedModes() {
    try {
      if (this.kittySupported) {
        process.stdout.write(ENABLE_KITTY);
        this._kittyEnabled = true;
      }
      // Always enable bracketed paste — ignored if unsupported
      process.stdout.write(ENABLE_BRACKETED_PASTE);
    } catch {
      // ignore
    }
  }

  /** Disable protocols (called on cleanup). */
  disableModes() {
    if (this._kittyEnabled) {
      process.stdout.write(DISABLE_KITTY);
    }
    process.stdout.write(DISABLE_BRACKETED_PASTE);
  }

  isKittyProtocolEnabled(): boolean {
    return this._kittyEnabled;
  }
}

export const terminalCapabilityManager = new TerminalCapabilityManager();
