// ---------------------------------------------------------------------------
// rpc/methods/settings.ts — Settings RPC types
// ---------------------------------------------------------------------------

export interface GetParams {}

export interface GetResult {
  model: string | null;
  conversation: string | null;
  reasoning_effort: string | null;
  reasoning_summary: string | null;
}

export interface UpdateParams {
  model?: string;
  conversation?: string;
  reasoning_effort?: string;
  reasoning_summary?: string;
}

export interface UpdateResult {
  model: string | null;
  conversation: string | null;
  reasoning_effort: string | null;
  reasoning_summary: string | null;
}
