//! A small HTTP/1.1 client for one local `opencode serve`, written on tokio's `TcpStream` (no
//! HTTP crate).
//!
//! - Loopback only: [`OpenCodeClient::new`] refuses every other address.
//! - One TCP connection per request, `Connection: close`, Basic auth `opencode:<password>`. The
//!   [`ServerPassword`] never shows in `Debug` output or in an error.
//! - Never sends `Accept-Encoding`, so OpenCode's compression middleware leaves bodies plain.
//! - A response head may take [`MAX_HEAD_BYTES`]; a body (`Content-Length`, chunked, or until the
//!   server closes) [`ClientOptions::max_body_bytes`], past which the call fails.
//! - Route-agnostic: every request names an [`ApiRoute`], a typed value from a closed allowlist
//!   (OpenCode 1's [`Route`], OpenCode 2's own route type), never a free string. Only this crate
//!   can add route types.
//! - Every call has a deadline: [`ClientOptions::timeout`], or [`ClientOptions::blocking_timeout`]
//!   for the routes that answer only when their work is done ([`ApiRoute::blocks`]).
//! - An event stream ([`OpenCodeClient::stream`], [`OpenCodeClient::events`]) yields each SSE
//!   event's `data:` payload and tells when the server last sent a line, comments included
//!   ([`sse`]).
//! - Errors name what failed, never request headers or response bodies.
//!
//! [`Route`]: super::routes::Route

mod chunked;
mod client;
mod head;
mod sse;

use std::io;
use std::time::Duration;

use serde::de::DeserializeOwned;

pub use client::{ClientOptions, OpenCodeClient, ServerPassword};
pub use sse::{EventStream, SseFault};

use crate::error::BusinessError;

/// Largest accepted response head: status line and headers (64 KiB).
pub const MAX_HEAD_BYTES: usize = 64 * 1024;

/// Default largest accepted response body (16 MiB).
pub const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

/// Default deadline of one call.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Default deadline of the calls that block until their work is done ([`ApiRoute::blocks`]).
pub const BLOCKING_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// A request method used by a route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// `GET`.
    Get,
    /// `POST`.
    Post,
    /// `PATCH`.
    Patch,
    /// `DELETE`.
    Delete,
}

impl Method {
    /// The method as sent on the request line.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}

/// One request the client can make: a typed value from a closed allowlist, never a free string.
///
/// Sealed: only route types of this crate implement it, so the client never builds a request from
/// text it was handed.
pub trait ApiRoute: sealed::Sealed {
    /// The request method.
    fn method(&self) -> Method;

    /// The request target: path and query, with every value checked or percent-encoded.
    fn target(&self) -> String;

    /// True for the calls that answer only when their work is done; they get
    /// [`ClientOptions::blocking_timeout`].
    fn blocks(&self) -> bool;

    /// True for the event stream, which is read with [`OpenCodeClient::stream`] and refused by
    /// [`OpenCodeClient::send`].
    fn is_stream(&self) -> bool;
}

/// Keeps [`ApiRoute`] closed to route types outside this crate.
pub(crate) mod sealed {
    /// Implemented by every [`ApiRoute`](super::ApiRoute) type of this crate.
    pub trait Sealed {}
}

/// A complete response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Response {
    /// The status code.
    pub fn status(&self) -> u16 {
        self.status
    }

    /// True for a `2xx` status.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// The first value of header `name` (any case), trimmed.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// The cursor of the next message page (`x-next-cursor`), for `Route::Messages { before }`.
    pub fn next_cursor(&self) -> Option<&str> {
        self.header("x-next-cursor").filter(|cursor| !cursor.is_empty())
    }

    /// The body bytes.
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// The body parsed as JSON.
    pub fn json<T: DeserializeOwned>(&self) -> Result<T, HttpError> {
        serde_json::from_slice(&self.body).map_err(|error| HttpError::InvalidJson {
            line: error.line(),
            column: error.column(),
        })
    }

    /// The response itself for a `2xx` status, otherwise [`HttpError::Status`].
    pub fn error_for_status(self) -> Result<Self, HttpError> {
        if self.is_success() {
            return Ok(self);
        }

        Err(HttpError::Status { status: self.status })
    }
}

/// Why a call failed. Never carries the password, request headers or a response body.
#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    /// The server address is not a loopback address.
    #[error("refused to talk to the OpenCode server: its address is not a loopback address")]
    NotLoopback,

    /// The event stream was asked for through a plain request.
    #[error("the OpenCode event stream is read with `events`, not as a single response")]
    StreamRoute,

    /// A plain request was asked for as an event stream.
    #[error("only the OpenCode event stream is read with `events`")]
    NotStreamRoute,

    /// Connecting, writing or reading failed (also a malformed chunked body).
    #[error("OpenCode server connection failed: {0}")]
    Io(#[source] io::Error),

    /// The call did not finish within its deadline.
    #[error("the OpenCode server did not answer within {} s", .0.as_secs())]
    Timeout(Duration),

    /// The response head is larger than [`MAX_HEAD_BYTES`].
    #[error("the OpenCode server's response head is larger than {MAX_HEAD_BYTES} bytes")]
    HeadTooLarge,

    /// The response body is larger than the client's limit.
    #[error("the OpenCode server's response body is larger than {limit} bytes")]
    BodyTooLarge {
        /// The limit in bytes.
        limit: usize,
    },

    /// The response is not valid HTTP/1.1.
    #[error("malformed OpenCode server response: {0}")]
    Malformed(&'static str),

    /// The server answered with a non-`2xx` status.
    #[error("the OpenCode server answered HTTP {status}")]
    Status {
        /// The status code.
        status: u16,
    },

    /// The body is not the expected JSON.
    #[error("the OpenCode server's response is not the expected JSON (line {line}, column {column})")]
    InvalidJson {
        /// Line of the first problem.
        line: usize,
        /// Column of the first problem.
        column: usize,
    },
}

impl From<io::Error> for HttpError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<HttpError> for BusinessError {
    fn from(error: HttpError) -> Self {
        let message = error.to_string();

        match error {
            HttpError::NotLoopback => Self::Forbidden(message),
            HttpError::Timeout(_) => Self::Timeout(message),
            HttpError::StreamRoute | HttpError::NotStreamRoute => Self::Internal(message),
            _ => Self::Protocol(message),
        }
    }
}
