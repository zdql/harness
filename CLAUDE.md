# Harness

## Architecture

```
┌──────────────────┐  stdin (JSON-RPC 2.0, line-delimited)  ┌──────────────────┐
│                  │ ──────────────────────────────────────▸ │                  │
│  Bun/Ink TUI     │                                         │  harness-server  │
│  packages/       │                                         │  crates/server/  │
│  frontend/       │ ◂────────────────────────────────────── │                  │
└──────────────────┘  stdout (JSON-RPC 2.0, line-delimited)  └──────────────────┘
```

The frontend spawns the Rust binary as a child process. All communication is JSON-RPC 2.0 over stdio — one JSON object per line, matched by `id`.

## Adding a new RPC method

1. `crates/server/src/rpc/methods.rs` — define Params + Result types, implement `RpcMethod`
2. `crates/server/src/handlers/<domain>.rs` — write the handler function
3. `crates/server/src/main.rs` — add a match arm in `dispatch()`
4. `packages/frontend/src/rpc/methods.ts` — add the method to `MethodRegistry`
