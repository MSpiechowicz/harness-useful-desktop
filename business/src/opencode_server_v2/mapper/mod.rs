//! OpenCode 2 server events as chat events.
//!
//! - [`turns`]: text and reasoning deltas, steps, executions and the session status.
//! - [`tools`]: tool calls as tool cards, with the v2 tool ids and touched paths.
//! - [`asks`]: permission requests and forms as dialogs.
//! - [`quiet`]: compaction and shell runs that show no turn.

pub mod asks;
pub mod quiet;
pub mod tools;
pub mod turns;
