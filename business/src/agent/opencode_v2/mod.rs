//! The OpenCode 2 chat engine: one private `opencode serve` per chat, driven over its `/api`
//! routes and event stream ([plan](../../../../docs/plans/opencode-v2.md)). Not available yet:
//! [`ChatProtocol::OpenCodeServerV2`](crate::harness::ChatProtocol::OpenCodeServerV2) fails to
//! start until the engine is complete.

mod approvals;
mod dialogs;
mod history;
mod ops;
mod probe;
mod prompts;
mod reports;
mod runs;
mod settings;
mod startup;
mod stream;
