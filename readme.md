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


It dawned on me recently that it would be advantageous to have an intimiate understanding of how a modern "Agent" works. As the singularity approaches, I might find some respite in the fact that I have read at least most of the implementation that my personal singularity runs on.

I spend the occasional weekend on this project and, as I improve it, find myself using it for actual programming work more and more often. I attempt to reimplement from first principles any feature i find particularly interesting that Codex or Claude Code use. Most are fairly trivial to figure out, others more complex (like sub-agents). My long term vision is that this architecture supports sub-agents speaking with one another as they work. Currently, sub-agents vaguely work, to be clear. 

The rest of this is fairly accurate AI slop. Feel free to read to understand your way around the repository.

Harness is a TUI-based AI assistant that lives in your terminal. Ask it to write code, debug issues, explore a codebase, or run commands — it has direct access to your filesystem and shell through a suite of built-in tools. A Rust backend handles agent logic and persistence while a React/Ink frontend provides a responsive terminal interface.

## ✨ Features

- **Agentic tool-use loop** — The AI calls tools iteratively to gather context and take action, then responds with a final answer.
- **Built-in tools** — `bash`, `read`, `write`, `edit`, `glob`, `grep`, `addition`, and `start_subagent` — everything an AI needs to navigate, edit, and automate within a codebase.
- **Subagents** — Spawn parallel child agents to work on focused subtasks concurrently. Each subagent runs independently and reports back when complete.
- **Conversation compaction** — When conversations grow too long, a summarization step reclaims context budget by replacing old messages with a dense summary.
- **Context budget tracking** — Per-model token limits with automatic estimation to prevent context overflow. Supports GPT-4, Claude, Gemini, DeepSeek, Llama, and more.
- **Conversation history** — Conversations are automatically saved and can be browsed or resumed from the history screen.
- **Configurable model** — Use any model available on [OpenRouter](https://openrouter.ai) (default: `anthropic/claude-sonnet-4`).
- **Custom instructions** — Set persistent system-prompt instructions to tailor the agent's behavior.
- **HUD (Heads Up Display)** — Live git branch, diff stats, context token usage, and current model shown in the UI.
- **Single command launch** — One script builds the server (if needed) and starts the UI.

## 🏗️ Architecture

```
┌──────────────────┐  JSON-RPC 2.0 over stdio  ┌──────────────────┐
│  Ink/React TUI   │ ◀────────────────────────▸ │  Rust Server     │
│  (TypeScript)    │                             │  (Tokio)         │
└──────────────────┘                             └──────────────────┘
                                                   ├─ agent   (LLM + tool loop)
                                                   │    ├─ tools (bash, read, write, edit, glob, grep, ...)
                                                   │    ├─ subagents (parallel child agents)
                                                   │    └─ compaction (conversation summarization)
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

### Chat Screen

| Key | Action |
|-----|--------|
| `Enter` | Send message |
| `Shift+Enter` | New line |
| `Escape` | Back to home |
| `/help` | See available commands | 


## 🛠️ Built-in Tools

| Tool | Description |
|------|-------------|
| **Bash** | Execute shell commands and return stdout/stderr |
| **Read** | Read file contents with optional line offset & limit |
| **Write** | Create or overwrite files (creates parent directories) |
| **Edit** | Edit files by replacing exact strings (preferred for modifications) |
| **Glob** | Find files matching a glob pattern |
| **Grep** | Search file contents with regex, with optional path and file-type filters |
| **Addition** | Add two numbers together (utility tool) |
| **Start Subagent** | Spawn a parallel child agent to work on a focused subtask |

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
│   ├── agent/                     # LLM client, tool executor, agent loop
│   │   └── src/
│   │       ├── tools/             # bash, read, write, edit, glob, grep, addition, start_subagent
│   │       ├── subagents/         # Parallel child agent spawning & registry
│   │       ├── conversation/      # Compaction & summarization
│   │       └── context/           # Token budget tracking per model
│   ├── storage/                   # Settings & conversation persistence
│   └── server/                    # JSON-RPC stdio server binary
│       └── src/
│           ├── handlers/          # RPC method handlers (conversation, settings, HUD)
│           └── rpc/               # JSON-RPC dispatch & method registry
└── packages/
    └── frontend/                  # Ink (React) terminal UI
        └── src/
            ├── rpc/               # Typed JSON-RPC client
            └── screens/           # Home, Chat, Settings, History screens
```

## 🤖 Subagent Architecture

Harness supports spawning parallel child agents via the `start_subagent` tool. This enables the AI to delegate focused subtasks and continue working while subagents execute concurrently.

### How it works

1. **Spawning** — When the LLM calls `start_subagent`, the tool spawns a tokio task that runs a fresh agent loop against its own `Conversation` (persisted under `<parent_conv>/subagent/<subagent_id>/`).

2. **Communication** — Each agent owns a `SubagentInbox` (an `mpsc::UnboundedReceiver` of `SubagentResult`s). The `StartSubagentTool` holds the matching sender. When a subagent finishes, it sends its result back via the channel and emits a `SubagentCompleted` event.

3. **Parent loop** — The parent agent drains the inbox at the top of every loop iteration, injecting results as user messages. When the LLM produces no tool calls, the parent waits on `inbox.recv().await` if subagents are still pending — keeping the agent loop alive until every in-flight subagent reports back.

4. **Recursion** — Subagents can spawn their own children. Each level creates its own inbox + `StartSubagentTool`, so grandchildren's results funnel back to their direct parent, not the top-level caller.

5. **Registry** — A process-wide `SubagentRegistry` tracks running subagents, recording the spawn tree for observability.

## 📄 License

MIT
