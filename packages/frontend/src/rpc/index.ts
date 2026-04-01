// ---------------------------------------------------------------------------
// rpc — Public API for the JSON-RPC layer
// ---------------------------------------------------------------------------

export { RpcClient } from "./client.ts";
export type { RpcClientOptions } from "./client.ts";

export type {
  JsonRpcRequest,
  JsonRpcResponse,
  JsonRpcSuccessResponse,
  JsonRpcErrorResponse,
  JsonRpcNotification,
  RequestId,
} from "./protocol.ts";

export type { MethodName, MethodRegistry } from "./methods/index.ts";
export type * as Settings from "./methods/settings.ts";
export type * as Conversation from "./methods/conversation.ts";
