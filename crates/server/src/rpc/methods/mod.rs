// ---------------------------------------------------------------------------
// rpc::methods — Typed method definitions, split by domain
//
// Each sub-module defines the RPC methods for one domain (settings,
// conversation, etc.). Every method implements the `RpcMethod` trait.
//
// To add a new domain:
//   1. Create a new file in this directory (e.g. `tools.rs`)
//   2. Re-export it here
//   3. Register its methods in the dispatcher (main.rs)
// ---------------------------------------------------------------------------

pub mod conversation;
pub mod settings;
pub mod hud;

use serde::{de::DeserializeOwned, Serialize};

/// Trait that every RPC method implements. This is the single source of truth
/// for what a method is called and what types it moves across the wire.
pub trait RpcMethod {
    /// The method name on the wire, e.g. `"settings.get"`.
    const NAME: &'static str;

    /// The params the frontend sends.
    type Params: DeserializeOwned + Send;

    /// The result the server sends back.
    type Result: Serialize + Send;
}
