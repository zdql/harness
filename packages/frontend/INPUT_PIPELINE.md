# Input Pipeline & Render Tree — port from gemini-cli

This document inventories every file copied from `gemini-cli/packages/cli` into
`packages/frontend`, what it does, what was stripped, and how the layers
connect. Read top-down — the flow mirrors data flow from the terminal up
through React.

---

## 0. Directory layout (target)

```
packages/frontend/src/
├── index.tsx                    # entry point; spawns RPC client, renders <App>
├── terminal/                    # raw ANSI helpers + capability detection
│   ├── controlSequences.ts      # [inlined] enable/disable kitty, mouse, paste...
│   ├── input.ts                 # ESC const, mouse-seq regexes
│   ├── mouse.ts                 # mouse parse (used by keypress filter)
│   └── terminalCapabilityManager.ts  # one-shot probe; enables kitty at startup
├── keypress/                    # stdin → Key event pipeline
│   ├── KeypressContext.tsx      # parser + priority pub/sub (the big one)
│   ├── useKeypress.ts           # consumer hook
│   ├── useKittyKeyboardProtocol.ts
│   └── useFocus.ts              # focus-in/out, needed for keypress filter
├── keybindings/                 # semantic layer over raw keys
│   ├── keyBindings.ts           # Command enum + defaults + KeyBinding class
│   ├── keyMatchers.ts           # Command → key matcher fns
│   ├── keybindingUtils.ts       # display helpers (human-readable names)
│   └── useKeyMatchers.tsx       # React context/hook for matchers
├── rpc/                         # [pre-existing] JSON-RPC client to Rust server
├── state/
│   └── historyStore.ts          # reducer owning history + pendingHistoryItems
├── ui/
│   ├── App.tsx                  # entry, branches on quitting
│   ├── DefaultAppLayout.tsx     # the Box that arranges the full TUI
│   ├── MainContent.tsx          # <Static> + pending split (the core trick)
│   ├── Composer.tsx             # bottom input box wrapper
│   ├── InputPrompt.tsx          # naïve single-line text buffer + useKeypress
│   └── HistoryItemDisplay.tsx   # switches on item.type to render messages
└── types.ts                     # HistoryItem discriminated union
```

---

## 1. Terminal layer (`terminal/`)

### `controlSequences.ts` — NEW (inlined from gemini-cli-core/utils/terminal.ts)
Tiny wrappers around `stdout.write('\x1b[...')` for:
- `enableKittyKeyboardProtocol()` / `disableKittyKeyboardProtocol()`  — `\x1b[>1u` / `\x1b[<u`
- `enableModifyOtherKeys()` / `disableModifyOtherKeys()`              — xterm fallback
- `enableBracketedPasteMode()` / `disableBracketedPasteMode()`        — `\x1b[?2004h/l`
- `enableMouseEvents()` / `disableMouseEvents()`                      — SGR mouse

**Original:** `@google/gemini-cli-core` → `packages/core/src/utils/terminal.ts` (81 LOC).
**Stripped:** `writeToStdout` helper (replaced with `process.stdout.write`).

### `input.ts` — COPIED verbatim
- Source: `ui/utils/input.ts`
- Exports: `ESC`, mouse escape-sequence prefixes/regexes. Used by the CSI-parser
  in `KeypressContext` and by `mouse.ts`.
- No edits needed.

### `mouse.ts` — COPIED, core dep removed
- Source: `ui/utils/mouse.ts`
- Exports: `parseMouseEvent()`, SGR + X11 parsers. `nonKeyboardEventFilter` in
  `KeypressContext` uses this to drop mouse bytes from the keyboard stream.
- **Stripped:** `import { enableMouseEvents, disableMouseEvents } from '@google/gemini-cli-core'`
  — we import these from `./controlSequences.ts` instead.

### `terminalCapabilityManager.ts` — COPIED, core deps removed
- Source: `ui/utils/terminalCapabilityManager.ts`
- One-shot probe on startup:
  1. Raw mode on, batch writes `kitty?` + `osc11?` + `termName?` + `modifyOther?` + `deviceAttrs?`
  2. Reads stdin until the device-attributes sentinel replies (1s timeout).
  3. Records `kittySupported`, `terminalBackgroundColor`, `terminalName`, `modifyOtherKeysSupported`.
- `enableSupportedModes()` then turns on Kitty (or modifyOtherKeys fallback) +
  bracketed paste. Called from `KeypressProvider` useEffect.
- Registers `cleanupTerminalOnExit` on `exit`/`SIGINT`/`SIGTERM`.
- **Stripped:** `debugLogger` from core (replaced with `console.debug`), imports
  from core terminal funcs (now local).

---

## 2. Keypress pipeline (`keypress/`)

### `KeypressContext.tsx` — COPIED, stripped, ~900 LOC
- Source: `ui/contexts/KeypressContext.tsx`
- **The heart of the pipeline.** Contents:
  - `KEY_INFO_MAP`, `KITTY_CODE_MAP`, `NUMPAD_MAP` — lookup tables for CSI/SS3/Kitty CSI-u codes.
  - `emitKeys(handler)` — generator that walks raw chars from stdin, emits `Key{name, shift, alt, ctrl, cmd, sequence, insertable}` events. Understands:
    - CSI sequences with modifier bitmasks
    - SS3 numpad
    - OSC 52 clipboard paste
    - Kitty CSI-u codepoints (including PUA keys at 57344+)
    - modifyOtherKeys format (`CSI 27 ; mod ; key ~` → re-encoded as CSI-u)
    - Mac Option+letter characters
  - `createDataListener` — 50ms ESC-timeout debounce around the generator.
  - **Middleware wrappers** (applied in `KeypressProvider` useEffect):
    - `bufferPaste` → collects paste-start..paste-end into one `'paste'` event.
    - `bufferBackslashEnter` → `"\" + Enter` within 5ms → shift+enter (newline).
    - `bufferFastReturn` (kitty OFF only) → text-then-Enter within 30ms → newline.
    - `nonKeyboardEventFilter` → drops mouse/focus bytes.
  - `KeypressProvider` — React context, priority pub/sub (`KeypressPriority.Critical|High|Normal|Low`), broadcasts in priority order; handler returning `true` stops propagation.
- **Stripped:**
  - `debugLogger`, `Config` type from core → `console.debug`
  - `useSettingsStore` → stubbed to `{debugKeystrokeLogging: false}` (hook into your own later)
  - `appEvents.emit(AppEvent.PasteTimeout)` → removed (no consumers yet)
- **Fixed:** import paths now point to `../terminal/*` and `./useFocus.ts`.

### `useKeypress.ts` — COPIED verbatim
- Source: `ui/hooks/useKeypress.ts`
- 40 LOC. Thin hook: subscribe+unsubscribe via `useEffect`.

### `useKittyKeyboardProtocol.ts` — COPIED verbatim
- Source: `ui/hooks/useKittyKeyboardProtocol.ts`
- 20 LOC. Reads the capability flag; lets components know "did kitty win?"

### `useFocus.ts` — COPIED verbatim
- Source: `ui/hooks/useFocus.ts`
- Owns the `FOCUS_IN`/`FOCUS_OUT` constants that `KeypressContext` imports.
  Also exposes a `useFocus()` hook if you want to track terminal focus events.

---

## 3. Keybindings layer (`keybindings/`)

### `keyBindings.ts` — COPIED, file I/O stripped
- Source: `ui/key/keyBindings.ts`
- Exports:
  - `Command` enum (~70 entries: SUBMIT, NEWLINE, DELETE_WORD_BACKWARD, …)
  - `KeyBinding` class (parses patterns like `"ctrl+shift+z"` → `.matches(key)`)
  - `defaultKeyBindingConfig: Map<Command, KeyBinding[]>` — maps each command to one or more bindings
  - `commandCategories`, `commandDescriptions` (for UIs that show "Ctrl+W: Delete word")
- **Stripped:**
  - `loadCustomKeybindings()` which reads `~/.gemini/keybindings.json` via `Storage` from core. Not needed for MVP — add back with your own path later.
  - `zod` validation and `comment-json` parser that came with it.

### `keyMatchers.ts` — COPIED verbatim
- Source: `ui/key/keyMatchers.ts`
- `KeyMatchers = {[C in Command]: (key: Key) => boolean}` — a record of matcher
  functions. Consumers call `matchers[Command.SUBMIT](key)`.
- `defaultKeyMatchers` is built once from `defaultKeyBindingConfig`.
- **Removed:** `loadKeyMatchers()` (depended on file-loading in keyBindings.ts).

### `keybindingUtils.ts` — COPIED verbatim
- Source: `ui/key/keybindingUtils.ts`
- Pretty-printer: `Command.DELETE_WORD_BACKWARD → "Ctrl+W"` for help screens.
  No Gemini coupling.

### `useKeyMatchers.tsx` — COPIED verbatim
- Source: `ui/hooks/useKeyMatchers.tsx`
- React context + `useKeyMatchers()` hook. Defaults to `defaultKeyMatchers` if
  no provider wraps the tree, so tests and simple apps "just work".

---

## 4. State (`state/`)

### `historyStore.ts` — NEW
A tiny zustand-free reducer implementing the minimum shape `MainContent` needs:

```ts
interface HistoryState {
  history: HistoryItem[]           // committed → goes into <Static>
  pendingHistoryItems: HistoryItem[] // streaming → below <Static>, re-renders
  historyRemountKey: number         // bump to force <Static> re-flush
}
```

Actions: `addItem`, `setPending`, `commitPending`, `clear`, `remount`.
Used via a custom `useHistory()` hook (plain `useSyncExternalStore`).

---

## 5. UI layer (`ui/`)

### `types.ts` — NEW
`HistoryItem` discriminated union:
```ts
type HistoryItem =
  | { id: number; type: 'user'; text: string }
  | { id: number; type: 'assistant'; text: string }
  | { id: number; type: 'tool'; name: string; result: string }
  | { id: number; type: 'error'; message: string }
  | { id: number; type: 'info'; text: string }
```

### `HistoryItemDisplay.tsx` — NEW (slimmed port of gemini-cli's version)
Pure dispatch on `item.type` → renders a `<Box>` with the right styling.
No markdown, no tool confirmations, no clickable surfaces — just text +
role-colored prefix. ~60 LOC.

### `MainContent.tsx` — NEW (slimmed port of `ui/components/MainContent.tsx`)
**The critical Static/pending split lives here:**
```tsx
return (
  <>
    <Static items={[...history]}>{(item) => <HistoryItemDisplay item={item}/>}</Static>
    {pendingItems.map(item => <HistoryItemDisplay key={item.id} item={item}/>)}
  </>
);
```
No virtualization, no alternate-buffer path — plain `<Static>` is enough for MVP.
~50 LOC.

### `InputPrompt.tsx` — NEW (reference impl using useKeypress + matchers)
Example consumer demonstrating the pattern:
```ts
useKeypress((key) => {
  if (matchers[Command.SUBMIT](key))             { onSubmit(text); return true; }
  if (matchers[Command.NEWLINE](key))            { insert('\n'); return true; }
  if (matchers[Command.DELETE_CHAR_LEFT](key))   { backspace(); return true; }
  if (matchers[Command.DELETE_WORD_BACKWARD](key)){ deleteWordLeft(); return true; }
  if (matchers[Command.CLEAR_INPUT](key))        { setText(''); return true; }
  if (key.insertable)                             { insert(key.sequence); return true; }
}, { isActive: true });
```
Naïve single-line `useState<string>` buffer. Swap for `text-buffer.ts` later if
you want multi-line + vim + undo. ~80 LOC.

### `Composer.tsx` — NEW (thin wrapper)
Bordered `<Box>` around `<InputPrompt>`. ~20 LOC.

### `DefaultAppLayout.tsx` — NEW (port of `ui/layouts/DefaultAppLayout.tsx`)
The full visible shell:
```tsx
<Box flexDirection="column" width={terminalWidth}>
  <MainContent />
  <Composer />
</Box>
```
~30 LOC.

### `App.tsx` — NEW (port of `ui/App.tsx`)
Branch point. Renders `<DefaultAppLayout>` wrapped in `KeypressProvider` +
`KeyMatchersProvider`. ~30 LOC.

---

## 6. Entry (`index.tsx`)

Spawns `RpcClient`, kicks off `terminalCapabilityManager.detectCapabilities()`,
calls Ink's `render(<App/>)`. Connects submitted composer text to
`rpc.call('conversation.send', ...)` and pushes the reply back into history.

---

## What was *not* ported (and why)

| Skipped | Reason |
|---|---|
| `text-buffer.ts` (4.3k LOC) | Multi-line editor + vim + undo; huge surface area, ~10 transitive core deps. Phase-2 upgrade. |
| Dialog components | User said one modal slot suffices; placeholder for now. |
| Themes, colors.ts | Trivial to rebuild per project aesthetics. |
| `useGeminiStream`, slash commands, tool scheduler | Gemini-specific; replaced by `RpcClient`. |
| ScrollableList, MaxSizedBox | Not needed until we want alt-buffer mode. |
| Messages renderers (markdown, code highlighter, …) | Rebuilt minimally in `HistoryItemDisplay`. |

---

## Data flow — the new pipeline

```
 stdin bytes ──▶ terminalCapabilityManager (one-shot probe + enable kitty)
                        │
                        ▼
                 KeypressProvider
                 │  emitKeys (CSI/SS3/Kitty/OSC parser)
                 │  bufferPaste → bufferBackslashEnter → bufferFastReturn? → filter
                 │  broadcast() to priority-ordered subscribers
                 ▼
                 useKeypress(handler) in InputPrompt
                        │  matchers[Command.X](key)
                        ▼
                 historyStore.addItem() / InputPrompt local state
                        │
                        ▼
  React re-renders ─▶ MainContent (<Static> keeps committed scrollback)
                   ─▶ Composer (re-renders every keystroke)
                        │
                        ▼
                 Ink diffs → stdout

 On submit:  InputPrompt.onSubmit(text)
                ──▶ historyStore.addItem({type:'user', text})
                ──▶ rpc.call('conversation.send', {id, message:text})
                ──▶ historyStore.addItem({type:'assistant', text: reply})
```
