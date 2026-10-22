//! Talking to `codex app-server` (JSON-RPC over stdio), the base of the Codex chat engine and the
//! Sessions view ([plan](../../../docs/plans/codex.md)). Pure code, no I/O:
//!
//! - [`wire`]: the frames Codex writes ([`wire::decode`] → [`wire::Inbound`]: responses,
//!   notifications, requests), the app's requests ([`wire::outbound`]), the answers to Codex's
//!   requests ([`wire::answers`]) and the results of the app's requests ([`wire::results`]).
//! - [`mapper`]: the frames as the chat's events, plus the hints the engine acts on (responses,
//!   turns, user messages taken, asks, names, token counts).
//! - [`stats`]: session statistics and the context meter.
//!
//! The transport is the shared one: frames are read with
//! [`FrameReader`](crate::pi_family::rpc::FrameReader) and written with
//! [`encode_frame`](crate::pi_family::rpc::encode_frame), the app's requests wait in
//! [`PendingRequests`](crate::pi_family::rpc::PendingRequests) under
//! [`RequestId`](wire::RequestId)'s text.

pub mod mapper;
pub mod stats;
pub mod wire;

pub use mapper::{Ask, AskKind, CodexMapper, Hint, Mapped, StartKind};
