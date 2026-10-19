//! The client of one `opencode serve`: connect, write a request, read the response.
//!
//! See the [module docs](super) for the rules every call follows.

use std::fmt;
use std::net::SocketAddr;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::timeout;

use super::chunked::{BodyReader, Framing};
use super::head::{Head, read_head};
use super::sse::EventStream;
use super::{BLOCKING_TIMEOUT, DEFAULT_TIMEOUT, HttpError, MAX_BODY_BYTES, Method, Response};
use crate::opencode_server::routes::Route;
use crate::pi_family::rpc::MAX_FRAME_BYTES;

/// The Basic auth user name (`OPENCODE_SERVER_USERNAME` is never set, so OpenCode's default).
const USERNAME: &str = "opencode";

/// The password of one `opencode serve` (`OPENCODE_SERVER_PASSWORD`). `Debug` never shows it and
/// it has no `Display`.
#[derive(Clone, PartialEq, Eq)]
pub struct ServerPassword(String);

impl ServerPassword {
    /// Wraps a password.
    pub fn new(password: impl Into<String>) -> Self {
        Self(password.into())
    }

    /// A random password for one launch: 256 bits from the OS generator, as 64 hex digits.
    pub fn random() -> Self {
        let first = uuid::Uuid::new_v4().simple().to_string();
        let second = uuid::Uuid::new_v4().simple().to_string();

        Self(first + &second)
    }

    /// The password itself, for the server's environment only.
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ServerPassword {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ServerPassword(<redacted>)")
    }
}

/// Limits and deadlines of an [`OpenCodeClient`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientOptions {
    /// Deadline of one call: connect, request and the whole response (the event stream: up to its
    /// head).
    pub timeout: Duration,
    /// Deadline of the routes that block until their work is done (`command`, `shell`,
    /// `summarize`).
    pub blocking_timeout: Duration,
    /// Largest accepted response body.
    pub max_body_bytes: usize,
    /// Largest accepted SSE event (all its `data:` lines); a larger one is skipped and reported.
    pub max_event_bytes: usize,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            blocking_timeout: BLOCKING_TIMEOUT,
            max_body_bytes: MAX_BODY_BYTES,
            max_event_bytes: MAX_FRAME_BYTES,
        }
    }
}

/// Talks to one `opencode serve` on a loopback address (see the [module docs](super)).
#[derive(Debug, Clone)]
pub struct OpenCodeClient {
    addr: SocketAddr,
    password: ServerPassword,
    options: ClientOptions,
}

impl OpenCodeClient {
    /// A client with the default [`ClientOptions`]; refuses a non-loopback `addr`.
    pub fn new(addr: SocketAddr, password: ServerPassword) -> Result<Self, HttpError> {
        Self::with_options(addr, password, ClientOptions::default())
    }

    /// A client with custom limits; refuses a non-loopback `addr`.
    pub fn with_options(addr: SocketAddr, password: ServerPassword, options: ClientOptions) -> Result<Self, HttpError> {
        if !addr.ip().is_loopback() {
            return Err(HttpError::NotLoopback);
        }

        Ok(Self {
            addr,
            password,
            options,
        })
    }

    /// The server's address.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Calls `route` with an optional JSON body, within the route's default deadline.
    pub async fn send(&self, route: Route<'_>, body: Option<&Value>) -> Result<Response, HttpError> {
        let limit = if route.blocks() {
            self.options.blocking_timeout
        } else {
            self.options.timeout
        };

        self.send_with_timeout(route, body, limit).await
    }

    /// Calls `route` with an optional JSON body, within `limit`. Any status is returned as a
    /// [`Response`]; see [`Response::error_for_status`].
    pub async fn send_with_timeout(
        &self,
        route: Route<'_>,
        body: Option<&Value>,
        limit: Duration,
    ) -> Result<Response, HttpError> {
        if route == Route::GlobalEvent {
            return Err(HttpError::StreamRoute);
        }

        let request = self.encode(route, body, "application/json");

        timeout(limit, self.exchange(request))
            .await
            .map_err(|_| HttpError::Timeout(limit))?
    }

    /// Opens `GET /global/event` and streams its events (see [`EventStream`]). Fails on a
    /// non-`2xx` status or when the head does not arrive within [`ClientOptions::timeout`]; the
    /// stream itself has no deadline.
    pub async fn events(&self) -> Result<EventStream, HttpError> {
        let request = self.encode(Route::GlobalEvent, None, "text/event-stream");
        let limit = self.options.timeout;

        let (reader, head) = timeout(limit, self.open(request))
            .await
            .map_err(|_| HttpError::Timeout(limit))??;

        if !(200..300).contains(&head.status) {
            return Err(HttpError::Status { status: head.status });
        }

        let body = BodyReader::new(reader, head.framing);

        Ok(EventStream::spawn(body, self.options.max_event_bytes))
    }

    async fn exchange(&self, request: Vec<u8>) -> Result<Response, HttpError> {
        let (reader, head) = self.open(request).await?;
        let max = self.options.max_body_bytes;

        if let Framing::Length(length) = head.framing
            && length > max as u64
        {
            return Err(HttpError::BodyTooLarge { limit: max });
        }

        let mut body = BodyReader::new(reader, head.framing);
        let mut bytes = Vec::new();

        (&mut body).take(max as u64 + 1).read_to_end(&mut bytes).await?;

        if bytes.len() > max {
            return Err(HttpError::BodyTooLarge { limit: max });
        }

        if !body.is_complete() {
            return Err(HttpError::Malformed("the response body ended early"));
        }

        Ok(Response {
            status: head.status,
            headers: head.headers,
            body: bytes,
        })
    }

    /// Connects, writes `request` and reads the response head.
    async fn open(&self, request: Vec<u8>) -> Result<(BufReader<TcpStream>, Head), HttpError> {
        let mut stream = TcpStream::connect(self.addr).await?;
        stream.set_nodelay(true)?;
        stream.write_all(&request).await?;
        stream.flush().await?;

        let mut reader = BufReader::new(stream);
        let head = read_head(&mut reader).await?;

        Ok((reader, head))
    }

    fn encode(&self, route: Route<'_>, body: Option<&Value>, accept: &str) -> Vec<u8> {
        let method = route.method();
        let credentials = BASE64.encode(format!("{USERNAME}:{}", self.password.expose_secret()));

        let mut head = format!(
            "{} {} HTTP/1.1\r\nHost: {}\r\nAuthorization: Basic {credentials}\r\nAccept: {accept}\r\n\
             Connection: close\r\n",
            method.as_str(),
            route.target(),
            self.addr,
        );

        let body = body.map(|value| value.to_string().into_bytes());

        match &body {
            Some(bytes) => {
                head.push_str("Content-Type: application/json\r\n");
                head.push_str(&format!("Content-Length: {}\r\n", bytes.len()));
            }
            None if method != Method::Get => head.push_str("Content-Length: 0\r\n"),
            None => {}
        }

        head.push_str("\r\n");

        let mut request = head.into_bytes();
        request.extend(body.unwrap_or_default());

        request
    }
}
