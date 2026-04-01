# harness

A terminal UI for controlling an AI agent, built with Ink (TypeScript) and Rust.

## Prerequisites

- [Rust](https://rustup.rs/) (1.87+)
- [Bun](https://bun.sh/) (1.0+)

## Setup

```sh
# Clone and enter the repo
git clone <repo-url> && cd harness

# Install frontend dependencies
cd packages/frontend && bun install && cd ../..
```

## Run

```sh
./bin/harness
```

This single command:
1. Builds the Rust server binary (if needed)
2. Launches the Ink terminal UI

To make it available globally as `harness`:

```sh
ln -s "$(pwd)/bin/harness" ~/.local/bin/harness
```

## Keybindings

| Key         | Action                  |
|-------------|-------------------------|
| `j` / `↓`  | Move selection down     |
| `k` / `↑`  | Move selection up       |
| `Enter`/`e` | Edit selected field    |
| `Esc`       | Cancel editing         |
| `q`         | Quit                   |

## Project structure

```
harness/
├── bin/harness                  # Launch script
├── crates/
│   ├── agent/                   # Agent logic (lib)
│   ├── storage/                 # Settings + conversation persistence
│   └── server/                  # JSON-RPC stdio server (binary)
└── packages/
    └── frontend/                # Ink terminal UI
        └── src/
            ├── rpc/             # Typed JSON-RPC client + protocol
            └── screens/         # UI screens
```
