//! The `{data}` response envelope of the OpenCode 2 server, its list cursors, the few responses
//! that are not enveloped (`/api/info`, `interrupt`) and the line `opencode serve` prints once it
//! listens ([`listening_addr`]).
//!
//! Every type reads leniently: unknown fields are ignored, missing optional ones are `None`.

use std::net::SocketAddr;

use serde::Deserialize;
use serde::de::DeserializeOwned;

/// `{data, cursor?, location?}`: what most routes answer. Lists add `cursor` (sessions, messages)
/// or `location` (agents, commands, models, providers, forms).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(bound = "T: DeserializeOwned")]
pub struct Envelope<T> {
    /// The payload.
    pub data: T,
    /// The neighbouring pages of a list.
    #[serde(default)]
    pub cursor: Option<Cursor>,
    /// The location the answer is about.
    #[serde(default)]
    pub location: Option<Location>,
}

impl<T: DeserializeOwned> Envelope<T> {
    /// Reads an enveloped response body.
    pub fn from_slice(body: &[u8]) -> serde_json::Result<Self> {
        serde_json::from_slice(body)
    }

    /// The next page's cursor, if there is one.
    pub fn next_cursor(&self) -> Option<&str> {
        self.cursor
            .as_ref()?
            .next
            .as_deref()
            .filter(|cursor| !cursor.is_empty())
    }
}

/// What `opencode serve` 2.x prints on stdout once it listens: `server listening on <url>`.
const LISTENING_PREFIX: &str = "server listening on http://";

/// The address in the server's listening line (`server listening on http://127.0.0.1:<port>`),
/// any IP; `None` for any other line.
pub fn listening_addr(line: &str) -> Option<SocketAddr> {
    let rest = line.trim().strip_prefix(LISTENING_PREFIX)?;

    rest.trim_end_matches('/').parse().ok()
}

/// The opaque cursors of the neighbouring list pages.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Cursor {
    /// The page before this one.
    #[serde(default)]
    pub previous: Option<String>,
    /// The page after this one.
    #[serde(default)]
    pub next: Option<String>,
}

/// A location (`{directory}`): an absolute project directory.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Location {
    /// The absolute directory.
    #[serde(default)]
    pub directory: String,
}

/// `GET /api/info` (not enveloped): `{version, pid, urls, paths, capabilities}`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct ServerInfo {
    /// The server version (`2.1.2`).
    #[serde(default)]
    pub version: String,
    /// The server's process id.
    #[serde(default)]
    pub pid: Option<u32>,
    /// The URLs the server listens on.
    #[serde(default)]
    pub urls: Vec<String>,
}

/// `POST …/interrupt` (not enveloped): `{interrupted}`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct Interrupted {
    /// True when a running execution was stopped, false when the session was idle.
    #[serde(default)]
    pub interrupted: bool,
}
