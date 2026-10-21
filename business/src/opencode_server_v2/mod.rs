//! Talking to a local OpenCode 2 `opencode serve` (its `/api` routes), the base of the OpenCode 2
//! chat engine ([plan](../../../docs/plans/opencode-v2.md)). Shares the loopback HTTP and SSE
//! client, the server password and the HTML export with [`crate::opencode_server`].
//!
//! - [`routes`]: the closed allowlist of OpenCode 2 requests ([`RouteV2`]) and the checked ids
//!   they take.
//! - [`wire`]: the server's JSON (the `{data}` envelope, sessions, catalogs) and the prompt body.
//! - [`events`]: the typed event stream ([`events::parse`] → [`events::Event`]).
//! - [`messages`]: OpenCode 2 session messages as transcript and history.
//! - [`tools`]: OpenCode 2 tool ids and the paths their calls touch.
//! - [`mapper`]: the server's events as chat events.
//! - [`modes`]: per-chat approval modes as OpenCode 2 permission rules.
//! - [`stats`]: session statistics and context usage.

pub mod events;
pub mod mapper;
pub mod messages;
pub mod modes;
pub mod routes;
pub mod stats;
pub mod tools;
pub mod wire;

pub use routes::{FormId, MessageId, MessageOrder, PermissionId, RouteV2, SessionId};
