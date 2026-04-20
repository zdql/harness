// ---------------------------------------------------------------------------
// handlers — Business logic for each RPC method
//
// Each handler is a pure function:  Params → Result<Value, String>
//
// One sub-module per domain, mirroring rpc/methods/.
// The dispatcher in main.rs routes method names to the right handler.
// ---------------------------------------------------------------------------

pub mod conversation;
pub mod settings;
pub mod hud;
