//! Talking to a local `opencode serve` (OpenCode's HTTP server), the base of the native OpenCode
//! chat engine ([plan](../../../docs/plans/opencode-native-chat.md)).
//!
//! - [`http`]: a small HTTP/1.1 and SSE client written on tokio's `TcpStream` that only connects to
//!   loopback addresses, with Basic auth, size caps and deadlines.
//! - [`routes`]: the closed [`Route`] allowlist, the only requests the client can make, and the
//!   checked ids they take.
//! - [`wire`]: the server's JSON read field by field, and the prompt body.
//! - [`mapper`]: the server's events as chat events ([`Mapper`]), and the OpenCode message
//!   conversion the CLI transcript shares.
//! - [`modes`]: per-chat approval modes as permission rules, and [`mode_config`]: those rules as
//!   the configuration of the chat's own server.
//! - [`stats`]: session statistics, context usage and the last reply from the messages.
//! - [`export_html`]: a chat transcript as a self-contained HTML page.

pub mod export_html;
pub mod http;
pub mod mapper;
pub mod mode_config;
pub mod modes;
pub mod routes;
pub mod stats;
pub mod wire;

pub use http::{ClientOptions, EventStream, HttpError, Method, OpenCodeClient, Response, ServerPassword, SseFault};
pub use mapper::{Mapped, Mapper};
pub use routes::{MessageId, PermissionId, QuestionId, Route, RouteError, SessionId};
