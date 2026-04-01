// ---------------------------------------------------------------------------
// rpc/methods/conversation.ts — Conversation RPC types
// ---------------------------------------------------------------------------

export interface CreateParams {}

export interface CreateResult {
  id: string;
}

export interface ListParams {}

export interface ListResult {
  conversations: ConversationSummary[];
}

export interface ConversationSummary {
  id: string;
}

export interface SwitchParams {
  id: string;
}

export interface SwitchResult {
  id: string;
}
