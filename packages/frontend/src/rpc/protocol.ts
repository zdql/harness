// ---------------------------------------------------------------------------
// rpc/protocol.ts — JSON-RPC 2.0 wire types
//
// These mirror the Rust `rpc` module exactly. Every message crossing the
// stdio boundary is one of these shapes.
// ---------------------------------------------------------------------------

export type RequestId = number | string;

/** Frontend → Server */
export interface JsonRpcRequest<P = unknown> {
  jsonrpc: "2.0";
  id: RequestId;
  method: string;
  params: P;
}

/** Server → Frontend (success) */
export interface JsonRpcSuccessResponse<R = unknown> {
  jsonrpc: "2.0";
  id: RequestId;
  result: R;
  error?: undefined;
}

/** Server → Frontend (error) */
export interface JsonRpcErrorResponse {
  jsonrpc: "2.0";
  id: RequestId;
  result?: undefined;
  error: {
    code: number;
    message: string;
    data?: unknown;
  };
}

export type JsonRpcResponse<R = unknown> =
  | JsonRpcSuccessResponse<R>
  | JsonRpcErrorResponse;

/** Server → Frontend (no id, push notification) */
export interface JsonRpcNotification<P = unknown> {
  jsonrpc: "2.0";
  method: string;
  params: P;
}

// Standard error codes
export const PARSE_ERROR = -32700;
export const INVALID_REQUEST = -32600;
export const METHOD_NOT_FOUND = -32601;
export const INVALID_PARAMS = -32602;
export const INTERNAL_ERROR = -32603;
