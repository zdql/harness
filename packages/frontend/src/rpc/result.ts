// ---------------------------------------------------------------------------
// rpc/result.ts — Lightweight Result type (neverthrow-inspired, zero deps)
//
// Forces callers to handle both success and failure explicitly.
// Errors are values, never thrown.
// ---------------------------------------------------------------------------

export type Ok<T> = { readonly ok: true; readonly value: T };
export type Err<E> = { readonly ok: false; readonly error: E };
export type Result<T, E> = Ok<T> | Err<E>;

export const ok = <T>(value: T): Ok<T> => ({ ok: true, value });
export const err = <E>(error: E): Err<E> => ({ ok: false, error });

/** Standard RPC error — shared across all call sites. */
export interface RpcError {
  code: number;
  message: string;
}
