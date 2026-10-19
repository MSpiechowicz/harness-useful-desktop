//! Agent chat sessions: a harness CLI running in `--mode rpc`, driven through its
//! [`RpcDialect`](crate::pi_family::rpc::RpcDialect).
//!
//! - [`AgentManager`] — every running session by key (at most [`MAX_SESSIONS`]); a
//!   [`SessionClaim`] keeps agents off a session id while its files are deleted.
//! - [`AgentSession`] — one process: writer, reader, stderr and waiter tasks, request
//!   correlation, dialogs, startup and shutdown.
//! - [`EventSink`] — where the normalized [`AgentEnvelope`](crate::model::AgentEnvelope)s go
//!   (the desktop shell adapts it to a Tauri channel).
//! - [`DeltaCoalescer`] — merges streaming deltas within [`COALESCE_WINDOW`].
//!
//! The webview never sees raw frames: every event is normalized, redacted where it carries agent
//! diagnostics, and numbered per session.

mod coalesce;
mod io;
mod manager;
mod session;
mod sink;
mod slots;
mod startup;

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub use coalesce::{COALESCE_WINDOW, DeltaCoalescer};
pub use io::{MAX_STDERR_LINE, MAX_STDERR_PER_SECOND};
pub use manager::{AgentDeps, AgentManager, MAX_SESSIONS, Stopping};
pub use session::{AgentSession, BASH_TIMEOUT, SessionLaunch};
pub use sink::{CollectingSink, EventSink, PromptTaken};
pub use slots::SessionClaim;
pub use startup::StartAbort;

/// Time limit for the handshake and the first `get_state`.
pub const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);

/// Time limit for the whole startup, which open dialogs do not extend (nobody can answer them
/// before startup completes).
pub const STARTUP_DEADLINE: Duration = Duration::from_secs(30);

/// How long `stop` waits after closing stdin before killing the process group.
pub const STOP_GRACE: Duration = Duration::from_secs(3);

/// Produces a value on demand (session keys, timestamps); replaceable in tests.
pub type Generator = Arc<dyn Fn() -> String + Send + Sync>;

/// Timing and identity knobs of new sessions.
#[derive(Clone)]
pub struct SessionOptions {
    /// Delta batching window; zero disables batching.
    pub coalesce_window: Duration,
    /// See [`STOP_GRACE`].
    pub stop_grace: Duration,
    /// See [`STARTUP_TIMEOUT`].
    pub startup_timeout: Duration,
    /// See [`STARTUP_DEADLINE`].
    pub startup_deadline: Duration,
    /// Session keys (uuid v4 by default).
    pub key: Generator,
    /// `startedAt` timestamps (RFC 3339 UTC by default).
    pub clock: Generator,
}

impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            coalesce_window: COALESCE_WINDOW,
            stop_grace: STOP_GRACE,
            startup_timeout: STARTUP_TIMEOUT,
            startup_deadline: STARTUP_DEADLINE,
            key: Arc::new(|| uuid::Uuid::new_v4().to_string()),
            clock: Arc::new(|| rfc3339(SystemTime::now())),
        }
    }
}

impl std::fmt::Debug for SessionOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionOptions")
            .field("coalesce_window", &self.coalesce_window)
            .field("stop_grace", &self.stop_grace)
            .field("startup_timeout", &self.startup_timeout)
            .field("startup_deadline", &self.startup_deadline)
            .finish_non_exhaustive()
    }
}

/// `time` as `YYYY-MM-DDTHH:MM:SSZ` (UTC, whole seconds; times before 1970 clamp to the epoch).
pub fn rfc3339(time: SystemTime) -> String {
    let seconds = time.duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs());
    let days = i64::try_from(seconds / 86_400).unwrap_or(0);
    let rest = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);

    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// Days since 1970-01-01 to a proleptic Gregorian date (Howard Hinnant's algorithm).
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);

    (year, month as u32, day as u32)
}
