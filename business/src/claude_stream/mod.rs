//! Talking to Claude Code's stream-json mode with its control protocol, the base of the Claude chat
//! engine ([plan](../../../docs/plans/claude-code.md)). Pure code, no I/O:
//!
//! - [`wire`]: the frames the CLI writes ([`wire::decode`] → [`wire::Inbound`]) and the encoders for
//!   what the app writes (user messages, control requests and replies).
//! - [`mapper`]: the frames as the chat's events, plus the hints the engine acts on (prompts taken
//!   from the CLI's queue, permission prompts, control replies, session changes).
//! - [`permissions`], [`allow_list`], [`questions`]: permission prompts and questions as chat
//!   dialogs, the answers to them and the Always ask hook.
//! - [`tools`]: Claude Code tool ids and the project files their calls touch.
//! - [`stats`]: session statistics and the context meter.

pub mod allow_list;
pub mod mapper;
pub mod permissions;
pub mod questions;
pub mod stats;
pub mod tools;
pub mod wire;

pub use mapper::{ClaudeMapper, Hint, Mapped, SendKind};
