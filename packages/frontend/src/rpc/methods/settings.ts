// ---------------------------------------------------------------------------
// rpc/methods/settings.ts — Settings RPC types
// ---------------------------------------------------------------------------

export interface GetParams {}

export interface GetResult {
  model: string | null;
  conversation: string | null;
}

export interface UpdateParams {
  model?: string;
  conversation?: string;
}

export interface UpdateResult {
  model: string | null;
  conversation: string | null;
}
