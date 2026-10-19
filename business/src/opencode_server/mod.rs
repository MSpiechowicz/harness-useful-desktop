//! Talking to a local `opencode serve` (OpenCode's HTTP server), the base of the native OpenCode
//! chat engine ([plan](../../../docs/plans/opencode-native-chat.md)).
//!
//! - [`http`]: a small HTTP/1.1 and SSE client written on tokio's `TcpStream` that only connects to
//!   loopback addresses, with Basic auth, size caps and deadlines.
//! - [`routes`]: the closed [`Route`] allowlist, the only requests the client can make, and the
//!   checked ids they take.

pub mod http;
pub mod routes;

pub use http::{ClientOptions, EventStream, HttpError, Method, OpenCodeClient, Response, ServerPassword, SseFault};
pub use routes::{MessageId, PermissionId, QuestionId, Route, RouteError, SessionId};
