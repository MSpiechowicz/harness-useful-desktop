//! The OpenCode 2 server's JSON, read field by field.
//!
//! - [`envelope`]: the `{data}` response envelope and list cursors.
//! - [`session`]: the session object.
//! - [`catalog`]: the agent, command, model and provider lists.
//! - [`prompt`]: the prompt body (`text`, `files`, `delivery`).

pub mod catalog;
pub mod envelope;
pub mod prompt;
pub mod session;
