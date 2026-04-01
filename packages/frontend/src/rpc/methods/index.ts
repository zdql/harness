// ---------------------------------------------------------------------------
// rpc/methods — Method registry, assembled from per-domain modules
//
// Each domain (settings, conversation, …) has its own file defining the
// param/result types. This file re-exports everything and defines the
// unified MethodRegistry that the RPC client uses for type safety.
//
// To add a new domain:
//   1. Create a new file in this directory (e.g. `tools.ts`)
//   2. Re-export its types below
//   3. Add its methods to the MethodRegistry interface
// ---------------------------------------------------------------------------

export type * as Settings from "./settings.ts";
export type * as Conversation from "./conversation.ts";

import type * as Settings from "./settings.ts";
import type * as Conversation from "./conversation.ts";

// ===========================================================================
// Method registry — the single source of truth for all RPC methods
//
// Key   = method name on the wire (must match Rust's RpcMethod::NAME)
// Value = { params: <what frontend sends>, result: <what server returns> }
// ===========================================================================

export interface MethodRegistry {
  // -- Settings -------------------------------------------------------------
  "settings.get": {
    params: Settings.GetParams;
    result: Settings.GetResult;
  };
  "settings.update": {
    params: Settings.UpdateParams;
    result: Settings.UpdateResult;
  };

  // -- Conversation ---------------------------------------------------------
  "conversation.create": {
    params: Conversation.CreateParams;
    result: Conversation.CreateResult;
  };
  "conversation.list": {
    params: Conversation.ListParams;
    result: Conversation.ListResult;
  };
  "conversation.switch": {
    params: Conversation.SwitchParams;
    result: Conversation.SwitchResult;
  };
  "conversation.send": {
    params: Conversation.SendParams;
    result: Conversation.SendResult;
  };
}

/** All known method names. */
export type MethodName = keyof MethodRegistry;
