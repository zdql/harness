<p align="center">
  <h1 align="center">⚡ Harness</h1>
  <p align="center">A lightweight, terminal-native AI coding agent with built-in tools.</p>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-1.87+-orange?logo=rust" />
  <img src="https://img.shields.io/badge/bun-1.0+-white?logo=bun" />
  <img src="https://img.shields.io/badge/license-MIT-blue" />
</p>

---

Harness is a TUI-based AI assistant that lives in your terminal. Ask it to write code, debug issues, explore a codebase, or run commands — it has direct access to your filesystem and shell through a suite of built-in tools. A Rust backend handles agent logic and persistence while a React/Ink frontend provides a responsive terminal interface.

## ✨ Features

- **Agentic tool-use loop** — The AI calls tools iteratively to gather context and take action, then responds with a final answer.
- **Built-in tools** — `bash`, `read_file`, `write_file`, `glob`, and `grep` — everything an AI needs to navigate and edit a codebase.
- **Conversation history** — Conversations are automatically saved and can be browsed or resumed from the history screen.
- **Configurable model** — Use any model available on [OpenRouter](https://openrouter.ai) (default: `anthropic/claude-sonnet-4`).
- **Custom instructions** — Set persistent system-prompt instructions to tailor the agent's behavior.
- **Single command launch** — One script builds the server (if needed) and starts the UI.

## 🏗️ Architecture

```
┌──────────────────┐  JSON-RPC 2.0 over stdio  ┌──────────────────┐
│  Ink/React TUI   │ ◀────────────────────────▸ │  Rust Server     │
│  (TypeScript)    │                             │  (Tokio)         │
└──────────────────┘                             └──────────────────┘
                                                   ├─ agent   (LLM + tool loop)
                                                   ├─ storage (settings + conversations)
                                                   └─ server  (JSON-RPC dispatch)
```

The frontend spawns the Rust binary as a child process. All communication is **JSON-RPC 2.0** — one JSON object per line, matched by `id`.

## 📦 Prerequisites

- [Rust](https://rustup.rs/) 1.87+
- [Bun](https://bun.sh/) 1.0+
- An [OpenRouter](https://openrouter.ai) API key

## 🚀 Getting Started

```sh
# Clone the repo
git clone <repo-url> && cd harness

# Install frontend dependencies
cd packages/frontend && bun install && cd ../..

# Add your OpenRouter API key
echo 'OPENROUTER_API_KEY=sk-or-...' > .env

# Launch
./bin/harness
```

The launch script automatically compiles the Rust server on first run (and whenever source files change).

### Make it globally available

```sh
ln -s "$(pwd)/bin/harness" ~/.local/bin/harness
```

## ⌨️ Keybindings

### Home Screen

| Key | Action |
|-----|--------|
| `s` | Open settings |
| `h` | Open conversation history |
| `Enter` | Send message / start chat |

### Chat Screen

| Key | Action |
|-----|--------|
| `Enter` | Send message |
| `Shift+Enter` | New line |
| `Escape` | Back to home |

### Settings & History

| Key | Action |
|-----|--------|
| `j` / `↓` | Move selection down |
| `k` / `↑` | Move selection up |
| `Enter` / `e` | Edit field / open conversation |
| `d` | Delete conversation (history) |
| `Escape` / `b` / `q` | Go back |

## 🛠️ Built-in Tools

| Tool | Description |
|------|-------------|
| **Bash** | Execute shell commands and return stdout/stderr |
| **Read File** | Read file contents with optional line offset & limit |
| **Write File** | Create or overwrite files (creates parent directories) |
| **Glob** | Find files matching a glob pattern |
| **Grep** | Search file contents with regex, with optional path and file-type filters |

## ⚙️ Configuration

Settings are persisted at `~/.config/harness/settings.json` and can be edited from the Settings screen inside the app.

| Setting | Default | Description |
|---------|---------|-------------|
| `model` | `anthropic/claude-sonnet-4` | Any model ID available on OpenRouter |
| `custom_instructions` | `""` | Extra instructions prepended to the system prompt |

Conversations are stored as JSON files under `~/.config/harness/conversations/`.

## 📁 Project Structure

```
harness/
├── bin/harness                    # Launch script (build + run)
├── crates/
│   ├── agent/                     # LLM client, tool executor, provider abstraction
│   │   └── src/tools/             # bash, read_file, write_file, glob, grep
│   ├── storage/                   # Settings & conversation persistence
│   └── server/                    # JSON-RPC stdio server binary
│       └── src/handlers/          # RPC method handlers
└── packages/
    └── frontend/                  # Ink (React) terminal UI
        └── src/
            ├── rpc/               # Typed JSON-RPC client & method registry
            └── screens/           # Home, Chat, Settings, History screens
```

## 📄 License

MIT
