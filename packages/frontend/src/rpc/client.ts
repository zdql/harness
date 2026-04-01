// ---------------------------------------------------------------------------
// rpc/client.ts — Typed JSON-RPC client over stdio
//
// Spawns the Rust `harness-server` binary as a child process and provides
// a fully-typed `call()` method that maps method names to their param/result
// types via the MethodRegistry.
// ---------------------------------------------------------------------------

import { spawn, type Subprocess, type FileSink } from "bun";
import { resolve } from "path";
import type { JsonRpcRequest, JsonRpcResponse, RequestId } from "./protocol.ts";
import type { MethodName, MethodRegistry } from "./methods/index.ts";

export interface RpcClientOptions {
  /** Path to the harness-server binary. Defaults to the cargo debug build. */
  serverBin?: string;
}

export class RpcClient {
  private process: Subprocess;
  private stdin: FileSink;
  private nextId = 1;
  private pending = new Map<
    RequestId,
    {
      resolve: (value: unknown) => void;
      reject: (error: Error) => void;
    }
  >();
  private buffer = "";
  private encoder = new TextEncoder();

  constructor(opts: RpcClientOptions = {}) {
    const bin =
      opts.serverBin ??
      resolve(import.meta.dir, "../../../../target/debug/harness-server");

    this.process = spawn([bin], {
      stdin: "pipe",
      stdout: "pipe",
      stderr: "inherit", // let server debug logs pass through
    });

    // Bun gives us a FileSink for piped stdin
    this.stdin = this.process.stdin as FileSink;

    // Read stdout line-by-line and resolve pending calls
    this.readLoop();
  }

  // ---- Public API ---------------------------------------------------------

  /**
   * Call an RPC method with full type safety.
   *
   * ```ts
   * const settings = await client.call("settings.get", {});
   * //    ^? SettingsGetResult
   * ```
   */
  async call<M extends MethodName>(
    method: M,
    params: MethodRegistry[M]["params"]
  ): Promise<MethodRegistry[M]["result"]> {
    const id = this.nextId++;

    const request: JsonRpcRequest<MethodRegistry[M]["params"]> = {
      jsonrpc: "2.0",
      id,
      method,
      params,
    };

    const promise = new Promise<MethodRegistry[M]["result"]>(
      (resolve, reject) => {
        this.pending.set(id, {
          resolve: resolve as (value: unknown) => void,
          reject,
        });
      }
    );

    const line = JSON.stringify(request) + "\n";
    try {
      this.stdin.write(this.encoder.encode(line));
      this.stdin.flush();
    } catch (e) {
      const entry = this.pending.get(id);
      if (entry) {
        this.pending.delete(id);
        entry.reject(new Error(`failed to write to server: ${e}`));
      }
    }

    return promise;
  }

  /** Gracefully shut down the server process. */
  close(): void {
    try {
      this.stdin.end();
    } catch {
      // stdin may already be closed
    }
    this.process.kill();
  }

  // ---- Internal -----------------------------------------------------------

  private async readLoop(): Promise<void> {
    const stdout = this.process.stdout;
    if (!stdout) return;

    const reader = (stdout as ReadableStream<Uint8Array>).getReader();
    const decoder = new TextDecoder();

    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        this.buffer += decoder.decode(value, { stream: true });

        // Process complete lines
        let newlineIdx: number;
        while ((newlineIdx = this.buffer.indexOf("\n")) !== -1) {
          const line = this.buffer.slice(0, newlineIdx).trim();
          this.buffer = this.buffer.slice(newlineIdx + 1);

          if (line.length === 0) continue;

          try {
            const msg = JSON.parse(line) as JsonRpcResponse;
            const entry = this.pending.get(msg.id);
            if (!entry) continue;

            this.pending.delete(msg.id);

            if (msg.error) {
              entry.reject(
                new Error(`RPC error ${msg.error.code}: ${msg.error.message}`)
              );
            } else {
              entry.resolve(msg.result);
            }
          } catch {
            // Skip malformed lines
          }
        }
      }
    } catch {
      // Stream ended
    }
  }
}
