// ---------------------------------------------------------------------------
// terminal/terminalCapabilityManager.ts
//
// One-shot probe of terminal capabilities at startup. We send a batch of
// queries and wait for responses (sentinel = Primary Device Attributes).
//
// What we learn:
//   - Kitty keyboard protocol support  →  prefer this for keyboard input
//   - modifyOtherKeys level            →  xterm fallback if no kitty
//   - Terminal background color (OSC 11)
//   - Terminal name/version
//
// Then enableSupportedModes() turns on the best protocol + bracketed paste.
//
// Copied from gemini-cli (ui/utils/terminalCapabilityManager.ts) with
// gemini-cli-core imports replaced by local ./controlSequences.ts and
// debugLogger replaced by console.debug.
// ---------------------------------------------------------------------------

import * as fs from "node:fs";
import {
  enableKittyKeyboardProtocol,
  disableKittyKeyboardProtocol,
  enableModifyOtherKeys,
  disableModifyOtherKeys,
  enableBracketedPasteMode,
  disableBracketedPasteMode,
  disableMouseEvents,
} from "./controlSequences.ts";

export type TerminalBackgroundColor = string | undefined;

const TERMINAL_CLEANUP_SEQUENCE =
  "\x1b[<u\x1b[>4;0m\x1b[?2004l\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l";

export function cleanupTerminalOnExit(): void {
  try {
    if (process.stdout?.fd !== undefined) {
      fs.writeSync(process.stdout.fd, TERMINAL_CLEANUP_SEQUENCE);
      return;
    }
  } catch (e) {
    console.debug("Failed to synchronously cleanup terminal modes:", e);
  }

  disableKittyKeyboardProtocol();
  disableModifyOtherKeys();
  disableBracketedPasteMode();
  disableMouseEvents();
}

export class TerminalCapabilityManager {
  private static instance: TerminalCapabilityManager | undefined;

  private static readonly KITTY_QUERY = "\x1b[?u";
  private static readonly OSC_11_QUERY = "\x1b]11;?\x1b\\";
  private static readonly TERMINAL_NAME_QUERY = "\x1b[>q";
  private static readonly DEVICE_ATTRIBUTES_QUERY = "\x1b[c";
  private static readonly MODIFY_OTHER_KEYS_QUERY = "\x1b[>4;?m";
  private static readonly HIDDEN_MODE = "\x1b[8m";
  private static readonly CLEAR_LINE_AND_RETURN = "\x1b[2K\r";
  private static readonly RESET_ATTRIBUTES = "\x1b[0m";

  // eslint-disable-next-line no-control-regex
  private static readonly KITTY_REGEX = /\x1b\[\?(\d+)u/;
  // eslint-disable-next-line no-control-regex
  private static readonly TERMINAL_NAME_REGEX = /\x1bP>\|(.+?)(\x1b\\|\x07)/;
  // eslint-disable-next-line no-control-regex
  private static readonly DEVICE_ATTRIBUTES_REGEX = /\x1b\[\?(\d+)(;\d+)*c/;
  static readonly OSC_11_REGEX =
    // eslint-disable-next-line no-control-regex
    /\x1b\]11;rgb:([0-9a-fA-F]{1,4})\/([0-9a-fA-F]{1,4})\/([0-9a-fA-F]{1,4})(\x1b\\|\x07)/;
  // eslint-disable-next-line no-control-regex
  private static readonly MODIFY_OTHER_KEYS_REGEX = /\x1b\[>4;(\d+)m/;

  private detectionComplete = false;
  private terminalBackgroundColor: TerminalBackgroundColor;
  private kittySupported = false;
  private kittyEnabled = false;
  private modifyOtherKeysSupported = false;
  private terminalName: string | undefined;

  private constructor() {}

  static getInstance(): TerminalCapabilityManager {
    if (!this.instance) {
      this.instance = new TerminalCapabilityManager();
    }
    return this.instance;
  }

  static resetInstanceForTesting(): void {
    this.instance = undefined;
  }

  /**
   * Detects terminal capabilities. Call once at app startup before rendering.
   */
  async detectCapabilities(): Promise<void> {
    if (this.detectionComplete) return;

    if (!process.stdin.isTTY || !process.stdout.isTTY) {
      this.detectionComplete = true;
      return;
    }

    process.off("exit", cleanupTerminalOnExit);
    process.off("SIGTERM", cleanupTerminalOnExit);
    process.off("SIGINT", cleanupTerminalOnExit);
    process.on("exit", cleanupTerminalOnExit);
    process.on("SIGTERM", cleanupTerminalOnExit);
    process.on("SIGINT", cleanupTerminalOnExit);

    return new Promise<void>((resolve) => {
      const originalRawMode = process.stdin.isRaw;
      if (!originalRawMode) {
        process.stdin.setRawMode(true);
      }

      let buffer = "";
      let kittyKeyboardReceived = false;
      let terminalNameReceived = false;
      let deviceAttributesReceived = false;
      let bgReceived = false;
      let modifyOtherKeysReceived = false;
      let timeoutId: NodeJS.Timeout | undefined;

      const cleanup = (): void => {
        if (timeoutId) clearTimeout(timeoutId);
        process.stdin.removeListener("data", onData);
        if (!originalRawMode) {
          process.stdin.setRawMode(false);
        }
        this.detectionComplete = true;
        resolve();
      };

      timeoutId = setTimeout(cleanup, 1000);

      const onData = (data: Buffer): void => {
        buffer += data.toString();

        if (!bgReceived) {
          const match = buffer.match(TerminalCapabilityManager.OSC_11_REGEX);
          if (match) {
            bgReceived = true;
            // We don't parse the rgb here — just record that we got one.
            // Store raw "r/g/b" triple; consumers can parse if needed.
            this.terminalBackgroundColor = `${match[1]}/${match[2]}/${match[3]}`;
          }
        }

        if (
          !kittyKeyboardReceived &&
          TerminalCapabilityManager.KITTY_REGEX.test(buffer)
        ) {
          kittyKeyboardReceived = true;
          this.kittySupported = true;
        }

        if (!modifyOtherKeysReceived) {
          const match = buffer.match(
            TerminalCapabilityManager.MODIFY_OTHER_KEYS_REGEX,
          );
          if (match) {
            modifyOtherKeysReceived = true;
            const level = parseInt(match[1]!, 10);
            this.modifyOtherKeysSupported = level >= 2;
          }
        }

        if (!terminalNameReceived) {
          const match = buffer.match(
            TerminalCapabilityManager.TERMINAL_NAME_REGEX,
          );
          if (match) {
            terminalNameReceived = true;
            this.terminalName = match[1];
          }
        }

        if (!deviceAttributesReceived) {
          const match = buffer.match(
            TerminalCapabilityManager.DEVICE_ATTRIBUTES_REGEX,
          );
          if (match) {
            deviceAttributesReceived = true;
            cleanup();
          }
        }
      };

      process.stdin.on("data", onData);

      try {
        fs.writeSync(
          process.stdout.fd,
          TerminalCapabilityManager.HIDDEN_MODE +
            TerminalCapabilityManager.KITTY_QUERY +
            TerminalCapabilityManager.OSC_11_QUERY +
            TerminalCapabilityManager.TERMINAL_NAME_QUERY +
            TerminalCapabilityManager.MODIFY_OTHER_KEYS_QUERY +
            TerminalCapabilityManager.DEVICE_ATTRIBUTES_QUERY +
            TerminalCapabilityManager.CLEAR_LINE_AND_RETURN +
            TerminalCapabilityManager.RESET_ATTRIBUTES,
        );
      } catch (e) {
        console.debug("Failed to write terminal capability queries:", e);
        cleanup();
      }
    });
  }

  enableSupportedModes(): void {
    try {
      if (this.kittySupported) {
        enableKittyKeyboardProtocol();
        this.kittyEnabled = true;
      } else if (this.modifyOtherKeysSupported) {
        enableModifyOtherKeys();
      }
      enableBracketedPasteMode();
    } catch (e) {
      console.debug("Failed to enable keyboard protocols:", e);
    }
  }

  getTerminalBackgroundColor(): TerminalBackgroundColor {
    return this.terminalBackgroundColor;
  }

  getTerminalName(): string | undefined {
    return this.terminalName;
  }

  isKittyProtocolEnabled(): boolean {
    return this.kittyEnabled;
  }
}

export const terminalCapabilityManager =
  TerminalCapabilityManager.getInstance();
