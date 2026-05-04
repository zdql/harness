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
  title?: string;
}

export interface SwitchParams {
  id: string;
}

export interface SwitchResult {
  id: string;
}

export interface GetParams {
  id: string;
}

export interface GetResult {
  id: string;
  title?: string;
  messages: MessageEntry[];
}

export interface MessageEntry {
  role: string;
  content: string;
  tool_name?: string;
  tool_args?: string;
}

export interface SendParams {
  id: string;
  message: string;
}

export interface SendResult {
  reply: string;
  tool_calls?: ToolCallInfo[];
}

export interface ToolCallInfo {
  name: string;
  arguments: string;
  result: string;
}

export interface SetModelParams {
  id: string;
  model: string;
}

export interface SetModelResult {
  id: string;
  model: string | null;
}
