# Harness Frontend — Architecture

Companion to `INPUT_PIPELINE.md` (which inventories what was copied from
gemini-cli). This file describes how the code is wired together **in this
repo**, after the port.

---

## High-level flow

```
┌──────────────────────────────────────────────────────────────────────────┐
│                         Terminal (kitty / iTerm / …)                     │
│       stdin bytes  ◀──raw mode──▶  stdout (ANSI diff from Ink)           │
└────────────┬────────────────────────────────────────────────┬────────────┘
             │                                                ▲
             ▼                                                │
┌──────────────────────────────────────────────────────────────────────────┐
│                        packages/frontend  (Bun + Ink)                    │
│                                                                          │
│   src/terminal/                                                          │
│     terminalCapabilityManager  ── one-shot probe, enables Kitty + paste  │
│     controlSequences.ts        ── ANSI write helpers                     │
│     input.ts, mouse.ts         ── prefix/regex + parser                  │
│                                                                          │
│                         ┌────────────────────┐                           │
│                         │  stdin 'data'      │                           │
│                         ▼                    │                           │
│   src/keypress/                              │                           │
│     KeypressProvider  ── emitKeys() parser   │                           │
│                        ── paste / backslash  │                           │
│                        ── fast-return buffers│                           │
│                        ── priority pub/sub   │                           │
│                               │              │                           │
│                               ▼              │                           │
│   src/keybindings/      useKeypress(handler) │                           │
│     keyBindings.ts           │ Key event     │                           │
│     keyMatchers.ts           ▼               │                           │
│     useKeyMatchers          matchers[Command.X](key)                     │
│                               │                                          │
│                               ▼                                          │
│   src/textbuffer/       useTextBuffer hook                               │
│                         ├── reducer over TextBufferState                 │
│                         │   (lines: grapheme[][], cursor, undo stack)    │
│                         └── on Enter → onSubmit(text), reset             │
│                               │                                          │
│                               ▼                                          │
│   src/ui/InputPrompt    renders visual rows (layoutLines + locateCursor) │
│                               │ onSubmit(text)                           │
│                               ▼                                          │
│   src/state/historyStore  addItem({type:'user', text})                   │
│                               │                                          │
│                               ▼                                          │
│                         RpcClient.call('conversation.send', …)           │
│                               │                                          │
│                               ▼                                          │
│   src/ui/MainContent    <Static>{committed history}</Static>             │
│                         {pendingHistoryItems}  ← streaming               │
│                         <Composer/>            ← bottom input            │
│                               │                                          │
│                               ▼                                          │
│                         Ink reconciles → stdout diff                     │
└────────────┬─────────────────────────────────────────────────────────────┘
             │ stdout  (JSON-RPC 2.0, line-delimited)
             ▼
┌──────────────────────────────────────────────────────────────────────────┐
│              crates/server/  —  harness-server (Rust)                    │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## The pipeline, step by step

### 1. Startup (`src/index.tsx`)

1. `terminalCapabilityManager.detectCapabilities()` runs **before** Ink
   mounts. It batches `CSI ?u` (kitty), `OSC 11` (bg color), `CSI >q` (term
   name), `CSI >4;?m` (modifyOtherKeys), and `CSI c` (device attrs). The
   device-attrs reply is the sentinel — when it arrives, we know all responses
   have flushed. Records `kittySupported`, `modifyOtherKeysSupported`,
   `terminalName`, `terminalBackgroundColor`.
2. `new RpcClient()` spawns the Rust `harness-server` binary as a subprocess
   (stdio pipes).
3. `rpc.call('conversation.create', {})` → we get a conversation id.
4. `render(<App … />)` mounts the React tree.

### 2. Raw stdin → Key events (`src/keypress/KeypressContext.tsx`)

`KeypressProvider`'s `useEffect` does:

- Calls `terminalCapabilityManager.enableSupportedModes()` — writes
  `\x1b[>1u` (Kitty) or the modifyOtherKeys fallback, plus bracketed paste.
- Puts stdin in raw mode, `utf8` encoding.
- Installs one `stdin.on('data', …)` listener wrapping the middleware chain:

  ```
  stdin bytes
    → createDataListener (50ms ESC-timeout debounce)
    → emitKeys generator
         (state machine for CSI / SS3 / OSC 52 / Kitty CSI-u / mac-Option chars)
    → bufferPaste             (collapses paste-start..paste-end)
    → bufferBackslashEnter    ("\" + Enter → shift+enter)
    → bufferFastReturn        (only when Kitty is OFF)
    → nonKeyboardEventFilter  (drops mouse + focus bytes)
    → broadcast()
  ```

`broadcast()` iterates subscribers in descending priority
(`Critical > High > Normal > Low`). Within a priority, most-recently-subscribed
runs first. Any handler that returns `true` stops propagation.

### 3. Semantic key matching (`src/keybindings/`)

Raw `Key` objects carry `{name, shift, alt, ctrl, cmd, sequence, insertable}`.
Handlers don't compare these directly — they ask:

```ts
const matchers = useKeyMatchers();
if (matchers[Command.DELETE_WORD_BACKWARD](key)) { … }
```

`defaultKeyBindingConfig` maps each `Command` to one or more `KeyBinding`
patterns (`"ctrl+w"`, `"alt+backspace"`, …). `createKeyMatchers` bakes those
into a record of matcher functions. Swap in a custom config when the user
adds rebindings.

### 4. Text buffer (`src/textbuffer/`)

A small, self-contained, multi-line, grapheme-aware, undoable text buffer.
Three files:

- `textBuffer.ts` — pure state + action functions. Lines are stored as
  `string[][]` where each line is an array of **grapheme clusters** (via
  `Intl.Segmenter`), so cursor indexing is always correct for emoji,
  combining marks, CJK, flag sequences, etc.
- `useTextBuffer.ts` — React hook built on `useReducer`. Subscribes to
  keypresses and maps them to actions via `matchers[Command.X](key)`. Calls
  `onSubmit(text)` on bare Enter and resets.
- `layout.ts` — visual wrap. `layoutLines(lines, viewportWidth)` chops
  logical lines into visual rows that fit the terminal width (using
  `string-width` for grapheme column widths). `locateCursor()` finds where
  the caret lands in visual space, so the cursor is always on the right
  character even when lines wrap.

Supported operations:

| Command | Keys | Behavior |
|---|---|---|
| SUBMIT | Enter | Fire `onSubmit(text)`, clear |
| NEWLINE | Shift/Ctrl/Alt/Cmd + Enter | Insert `\n` at cursor, split line |
| DELETE_CHAR_LEFT | Backspace, Ctrl+H | Delete grapheme left; joins lines at col 0 |
| DELETE_CHAR_RIGHT | Delete, Ctrl+D | Delete grapheme right; joins next line at EOL |
| DELETE_WORD_BACKWARD | Ctrl+W, Alt/Ctrl+Backspace | Delete previous word |
| DELETE_WORD_FORWARD | Alt+D, Alt/Ctrl+Delete | Delete next word |
| KILL_LINE_LEFT / RIGHT | Ctrl+U / Ctrl+K | Delete to line start / end |
| CLEAR_INPUT | Ctrl+C (non-empty) | Clear buffer (empty buffer → bubbles for quit) |
| UNDO / REDO | Cmd/Alt+Z, Ctrl/Cmd/Alt+Shift+Z | Restore prior state |
| MOVE_LEFT / RIGHT | ← / → , Ctrl+F | Move cursor one grapheme |
| MOVE_UP / DOWN | ↑ / ↓ | Move between logical lines (col clamped) |
| MOVE_WORD_LEFT / RIGHT | Alt+B/F, Ctrl+←/→ | Move by word |
| HOME / END | Home/End, Ctrl+A/E | Logical line start/end |
| SCROLL_HOME / END | Ctrl/Shift+Home/End | Buffer start/end |
| (paste) | Ctrl/Cmd+V, bracketed paste | Insert (splits on `\n` into multi-line) |

Insert operations **coalesce** — typing `a`, `b`, `c` produces one undo step,
not three. Structural edits (newline, paste, delete-word) break coalescing
so they become separate undo steps.

### 5. Rendering the buffer (`src/ui/InputPrompt.tsx`)

Now just a renderer. The hook owns editing state; the component owns layout:

```tsx
const { state } = useTextBuffer({ focused, onSubmit });
const { stdout } = useStdout();
const viewportWidth = Math.max(10, (stdout?.columns ?? 80) - 6);

const visualRows = layoutLines(state.lines, viewportWidth);
const { visualRow, visualCol } = locateCursor(visualRows, state.cursorRow, state.cursorCol);

// Render each visual row; highlight the grapheme at (visualRow, visualCol).
```

This keeps editing logic testable in isolation, and gives us the option to
swap renderers (e.g., add inline syntax highlighting) without touching the
state machine.

### 5. State (`src/state/historyStore.ts`)

A tiny external store (plain `useSyncExternalStore`, no deps). Owns:

- `history: HistoryItem[]` — committed, rendered in `<Static>`
- `pendingHistoryItems: HistoryItem[]` — streaming, rendered below
- `historyRemountKey: number` — bump to force `<Static>` flush

Actions: `addItem`, `setPending`, `commitPending`, `clear`.

`HistoryItem` is a discriminated union in `src/ui/types.ts`:
```ts
type HistoryItem =
  | { id: number; type: "user";      text: string }
  | { id: number; type: "assistant"; text: string }
  | { id: number; type: "tool";      name: string; result: string }
  | { id: number; type: "error";     message: string }
  | { id: number; type: "info";      text: string };
```

Add new kinds here as the agent grows; `HistoryItemDisplay` is a switch on
`item.type` so a new variant gets a compile error until you render it.

### 6. The `<Static>` + pending split (`src/ui/MainContent.tsx`)

The critical Ink pattern:

```tsx
<>
  <Static key={historyRemountKey} items={history}>
    {(item) => <HistoryItemDisplay key={item.id} item={item} />}
  </Static>
  {pendingHistoryItems.length > 0 && (
    <Box flexDirection="column">
      {pendingHistoryItems.map((item) => (
        <HistoryItemDisplay key={item.id} item={item} />
      ))}
    </Box>
  )}
</>
```

- Items passed to `<Static>` are written to stdout **once**. They scroll up
  into the terminal's real scrollback buffer and never re-render. This is
  why typing doesn't cause every old message to repaint.
- `pendingHistoryItems` sit below `<Static>`, re-rendering normally. When a
  streaming response finishes, call `historyStore.commitPending()` to move
  them into `history`. They become "permanent" scrollback.

### 7. Layout (`src/ui/DefaultAppLayout.tsx`)

```tsx
<Box flexDirection="column">
  <MainContent />      ← scrollback (grows)
  <Composer />         ← fixed bottom
</Box>
```

When you add modals/dialogs, swap the `<Composer>` slot for the dialog:

```tsx
{modal ? modal : <Composer …/>}
```

Dialog components subscribe to `useKeypress` at higher priority and return
`true` to swallow keys, so the composer keeps running underneath but sees
no input until the dialog closes.

### 8. Top level (`src/ui/App.tsx`)

Wraps the layout in providers:

```tsx
<KeypressProvider>
  <KeyMatchersProvider value={defaultKeyMatchers}>
    <GlobalKeyHandler onExit={…} />      ← ctrl+c at lowest priority
    <DefaultAppLayout … />
  </KeyMatchersProvider>
</KeypressProvider>
```

`GlobalKeyHandler` subscribes at `priority: false` (= `Normal`), so the
InputPrompt running at default `Normal` gets the key first (via stack order
within a priority). Only when the input is empty and doesn't swallow
ctrl+c does the global handler see it and exit.

### 9. Send loop (`src/index.tsx`)

```ts
async function onSubmit(text: string) {
  historyStore.addItem({ type: "user", text });
  const res = await rpc.call("conversation.send", { id, message: text });
  if (!res.ok) {
    historyStore.addItem({ type: "error", message: `…${res.error.message}` });
    return;
  }
  historyStore.addItem({ type: "assistant", text: res.value.reply });
  for (const call of res.value.tool_calls ?? []) {
    historyStore.addItem({ type: "tool", name: call.name, result: call.result });
  }
}
```

This is synchronous request/response today. To stream tokens:
1. Add a `conversation.stream` method that sends JSON-RPC notifications.
2. In `RpcClient`, route notifications to an emitter.
3. On stream start, `historyStore.setPending([{type:'assistant', text:''}])`.
4. On each chunk, append to `pending[0].text` via `setPending(...)`.
5. On stream end, `historyStore.commitPending()`.

---

## Directory map (final)

```
packages/frontend/
├── INPUT_PIPELINE.md            # what was copied from gemini-cli + why
├── ARCHITECTURE.md              # ← this file
├── package.json
├── tsconfig.json
└── src/
    ├── index.tsx                # entry: probe + RPC + render
    ├── terminal/
    │   ├── controlSequences.ts
    │   ├── input.ts
    │   ├── mouse.ts
    │   └── terminalCapabilityManager.ts
    ├── keypress/
    │   ├── KeypressContext.tsx
    │   ├── useKeypress.ts
    │   ├── useKittyKeyboardProtocol.ts
    │   └── useFocus.ts
    ├── keybindings/
    │   ├── keyBindings.ts
    │   ├── keyMatchers.ts
    │   ├── keybindingUtils.ts
    │   └── useKeyMatchers.tsx
    ├── textbuffer/
    │   ├── textBuffer.ts       # state + reducer actions (pure)
    │   ├── useTextBuffer.ts    # hook: keypress → actions
    │   └── layout.ts           # grapheme-aware visual wrap
    ├── state/
    │   └── historyStore.ts
    ├── rpc/                     # [pre-existing] JSON-RPC client
    │   ├── client.ts
    │   ├── protocol.ts
    │   ├── result.ts
    │   ├── index.ts
    │   └── methods/
    └── ui/
        ├── types.ts
        ├── HistoryItemDisplay.tsx
        ├── MainContent.tsx
        ├── InputPrompt.tsx
        ├── Composer.tsx
        ├── DefaultAppLayout.tsx
        └── App.tsx
```

---

## What each layer is responsible for (single-sentence version)

| Layer | Responsibility |
|---|---|
| `terminal/` | Probe what the emulator supports; turn the right control modes on/off. |
| `keypress/` | Turn raw stdin bytes into semantic `Key` events, broadcast to subscribers by priority. |
| `keybindings/` | Map `(pattern string) ↔ Key` and expose matchers keyed by `Command`. |
| `textbuffer/` | Multi-line, grapheme-aware, undoable input buffer + visual wrap. |
| `state/` | Hold the conversation history and streaming buffer. |
| `ui/` | Render the shell; wire keypresses into state via `Command` matchers. |
| `rpc/` | Talk to the Rust agent over stdio JSON-RPC. |
| `index.tsx` | Bootstrap sequence: probe → spawn server → create conversation → render. |

---

## Extension guide

**Add a new keyboard shortcut:**
1. Add a `Command` enum value in `src/keybindings/keyBindings.ts`.
2. Add it to `defaultKeyBindingConfig` with one or more `KeyBinding(pattern)` entries.
3. Call `matchers[Command.X](key)` in your component's `useKeypress` handler.

**Add a new message type:**
1. Add a variant to `HistoryItem` in `src/ui/types.ts`.
2. Add a `case` in `HistoryItemDisplay.tsx` (the switch is exhaustive — TypeScript will flag the missing case).
3. Push from wherever via `historyStore.addItem({ type: "new-type", … })`.

**Add a modal dialog:**
1. Build a component that uses `useKeypress(…, { priority: KeypressPriority.High })` and returns `true` from keys it handles.
2. Gate the Composer in `DefaultAppLayout`: `{modal ? modal : <Composer/>}`.

**Extend the text buffer:**
1. Add a pure action function to `textbuffer/textBuffer.ts` (e.g., `duplicateLine`).
2. Add an `Action` variant + reducer case in `useTextBuffer.ts`.
3. Dispatch it from a new `matchers[Command.X](key)` check, or expose it on the
   hook's return value for programmatic callers.

**Add a selection / kill-ring / vim mode:**
These are additive. Selection = `{anchorRow, anchorCol}` on state, a
`selecting: boolean` flag, and handlers for shift+movement. Kill ring = a
ref outside the reducer that `killLine*` writes to, `yank` reads. Vim mode
would mirror gemini-cli's `vim-buffer-actions.ts` pattern: a mode flag on
state + a separate reducer wing for normal-mode actions.
