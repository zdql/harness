// ---------------------------------------------------------------------------
// ui/types.ts — HistoryItem discriminated union
//
// A minimal item set for the MVP. Add more variants as your agent grows
// (e.g., 'thinking', 'tool-call-request', 'confirmation').
// ---------------------------------------------------------------------------

export type HistoryItem =
  | { id: number; type: "user"; text: string }
  | { id: number; type: "assistant"; text: string }
  | { id: number; type: "tool"; name: string; arguments?: string; result: string }
  | { id: number; type: "tool-running"; name: string; arguments?: string }
  | { id: number; type: "thinking"; label: string }
  | { id: number; type: "error"; message: string }
  | { id: number; type: "info"; text: string }
  | { id: number; type: "evicted" };

export type HistoryItemType = HistoryItem["type"];
