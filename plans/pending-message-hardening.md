# Plan: Harden Pending Message System

## Problem

The current "pending" message system has multiple glitches that cause broken UX — dropped messages, flickering renders, swallowed streaming content, and no actual queue for messages submitted while the model is busy.

---

## Identified Issues

| # | Issue | Location | Severity |
|---|---|---|---|
| 1 | `setPending` assigns new IDs every call → unstable React keys | `historyStore.setPending()` | High |
| 2 | `content_delta` events are swallowed — no live text streaming | `index.tsx` event handler | High |
| 3 | `commitPending()` then `addItem()` = two state updates → flicker | `index.tsx` lines 198–203 & 144–145 | Medium |
| 4 | `suspended` path resets `completed = []` → drops tool items | `index.tsx` line 190 | High |
| 5 | No pending message queue — input is rejected & lost when suspended | `index.tsx` onSubmit | High |
| 6 | `suspended` flag can get stuck `true` on error paths | `index.tsx` | Medium |
| 7 | Pending items get unstable keys on every `pushPending()` call | `MainContent.tsx` + `historyStore` | High (same root as #1) |

---

## Fix Plan

### Phase 1: Stabilize the store (fixes #1, #3, #7)

**Goal:** `setPending` must produce stable IDs across calls so React keys don't churn.

**Changes to `src/state/historyStore.ts`:**

- Remove the ID-allocation logic from `setPending`. Instead, `setPending` should accept items that *already have IDs* (full `HistoryItem`, not `HistoryItemInput`).
- Introduce a `streamingItems: HistoryItem[]` concept — items accumulate across a streaming session with stable IDs assigned once on first appearance.
- Replace the two-step `commitPending()` + `addItem()` with a single `commitAndAdd(item)` that does both in one state transition (eliminates flicker from #3).
- Alternatively: batch both mutations and let React see a single state snapshot via a `startTransition` or by coalescing the two `emit()` calls into one.

**Specific approach for stable IDs:**

The caller (`index.tsx`) should own a `Map<string, HistoryItem>` for the current streaming session. When a new item appears (e.g., a tool call starts), it gets an ID assigned once and is stored in the map. Subsequent updates mutate the item in-place (e.g., appending to `result` field). The map values are passed to `setPending` each tick. IDs never change within a session.

### Phase 2: Fix the streaming state machine (fixes #2, #4, #6)

**Goal:** Don't lose data, don't get stuck, and show live content.

**Changes to `src/index.tsx`:**

- **Fix #2 (content_delta):** Maintain a `contentBuf: string` alongside `reasoningBuf`. On `content_delta`, append to it. Add a `streamingAssistant` item to the pending list whose `text` field grows with `contentBuf`. This gives live text streaming.
- **Fix #4 (dropped tools on suspend):** Don't reset `completed = []` when entering suspended state. The tool items that already completed during the main request should remain visible. Only reset the spinner.
- **Fix #6 (stuck suspended flag):** Add error-path cleanup. If `conversation.send` returns an error and `suspended` is true, or if the RPC connection dies, reset `suspended = false`. Also add a safety timeout: if no `continuation_done` arrives within N minutes, force-unlock.

### Phase 3: Add the pending message queue (fixes #5)

**Goal:** Messages submitted while the model is busy are queued and replayed when the model becomes available.

**New module: `src/state/pendingQueue.ts`**

```ts
// Tiny FIFO queue — no deps, plain useSyncExternalStore
interface PendingQueueState {
  messages: string[];     // ordered FIFO
  isActive: boolean;      // is the model currently processing?
}

// Actions:
//   enqueue(text): add a message to the queue
//   dequeue(): string | undefined — pop the next message
//   peek(): string | undefined — look at next without removing
//   setActive(isActive): set whether model is busy
//   clear(): wipe the queue
```

**Changes to `src/index.tsx`:**

- On submit: if model is busy (`suspended` or `pendingQueue.isActive`), call `pendingQueue.enqueue(text)` instead of rejecting. Also add a `historyStore.addItem({type: "info", text: "Message queued (1 pending)"})` so the user sees feedback.
- When the model signals readiness (after `continuation_done` or successful non-suspended `conversation.send`):
  1. Set `pendingQueue.setActive(false)`
  2. Check `pendingQueue.peek()`
  3. If a message is waiting, dequeue it and immediately call `onSubmit(dequeued)` — this starts the next send automatically
  4. Mark `pendingQueue.setActive(true)` again since we're now processing

**Changes to `src/ui/Composer.tsx` or `src/ui/DefaultAppLayout.tsx`:**

- Show a queue indicator when `pendingQueue.messages.length > 0`. E.g., a small `Box` above the composer: `⟳ 2 messages queued`
- This can be a simple `<Text dimColor>⟳ {count} queued</Text>` inside the Composer border

### Phase 4: Wire it together — the emit/collect loop

The key integration point: defining **when the model is "ready"** to accept more messages. This is where the queue drains.

**Readiness signals (pick one or combine):**

| Signal | Meaning |
|---|---|
| `conversation.send` returns non-suspended | Model finished, no subagents |
| `continuation_done` notification | Background subagents finished |
| New RPC method `conversation.ready` | Explicit poll — does the model want more input? |

**For now**, the simplest correct approach: the model is ready when either:
- A `conversation.send` call returns with `suspended: false`, OR
- A `continuation_done` notification arrives

After either of these, drain one item from the queue (if any) and fire a new `conversation.send`.

---

## Execution Order

1. **Phase 1** — store stabilization (unblocks correct rendering, easiest to test)
2. **Phase 2** — streaming state machine fixes (fixes visible data-loss bugs)
3. **Phase 3** — pending queue (new feature, self-contained)
4. **Phase 4** — integration wiring (depends on 1–3)

Each phase should be independently shippable. Phase 1 alone is a meaningful improvement.

---

## Files Touched

| File | Change |
|---|---|
| `src/state/historyStore.ts` | Stable ID allocation, `commitAndAdd` action |
| `src/state/pendingQueue.ts` | **NEW** — FIFO queue store |
| `src/index.tsx` | Fix streaming state machine, add queue integration, wire readiness signals |
| `src/ui/MainContent.tsx` | Minor — may adjust pending render if item shape changes |
| `src/ui/Composer.tsx` | Add queue count indicator |
| `src/ui/types.ts` | Possibly add a `"queued"` info variant or leave as-is |

---

## Open Questions (defer to implementation)

1. **Should the queue have a max size?** E.g., 10 messages max — reject after that. Prevents runaway queuing if the model gets stuck.
2. **Should queued messages show their content in the UI?** E.g., a collapsible list of what's queued. Or just a count.
3. **Readiness signal:** Do we need an explicit `conversation.ready` RPC, or are the two existing signals (non-suspended return + `continuation_done`) sufficient?
4. **Content streaming format:** When we show `content_delta` live, do we render raw text or do we need a basic markdown renderer? (Current `HistoryItemDisplay` is text-only.)
