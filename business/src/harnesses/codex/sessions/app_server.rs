//! The short-lived `codex app-server` the Sessions view asks: one process per view load, never a
//! shell. It runs in the project with Codex's update check off (`-c
//! check_for_update_on_startup=false`), the Codex home the app resolves (`CODEX_HOME`), the
//! harness's spawn environment and the launcher search directories.
//!
//! [`AppServer::start`] spawns it and runs the handshake (`initialize`, then `initialized`),
//! [`AppServer::call`] sends one request and waits for its response, [`AppServer::finish`] closes
//! stdin and waits for the exit. Everything shares one deadline: past it the process is killed
//! and the call fails with `Timeout`. Lines longer than [`MAX_LINE_BYTES`] fail the call; lines
//! that are not frames and Codex's notifications are skipped, and a request of Codex is refused
//! (the Sessions view never answers questions).

use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::codex_rpc::wire::answers::{initialized, refusal};
use crate::codex_rpc::wire::outbound::{Initialize, Request, next_request_id};
use crate::codex_rpc::wire::{Inbound, RpcError, decode};
use crate::error::{BusinessError, Result};
use crate::harness::{CliSessionCtx, Harness, rpc_exec_spec};
use crate::harnesses::codex::{Codex, codex_home};
use crate::pi_family::normalize::plain_text;
use crate::process::{LineChild, ReadLine, launcher_search_dirs};
use crate::security::redact_text;

/// The longest stdout line read (a page of items with long command output).
pub(super) const MAX_LINE_BYTES: usize = 32 * 1024 * 1024;

/// How long the process gets to exit after stdin closed.
const EXIT_GRACE: Duration = Duration::from_secs(2);

/// How long a process that stopped gets to finish writing stderr.
const STDERR_GRACE: Duration = Duration::from_secs(1);

/// Longest Codex error text passed on, in characters.
const MAX_ERROR_CHARS: usize = 200;

/// The arguments: the app-server, without its update check.
const ARGS: [&str; 3] = ["app-server", "-c", "check_for_update_on_startup=false"];

/// A running `codex app-server` (see the module docs).
pub(super) struct AppServer {
    child: LineChild,
    deadline: Instant,
}

/// Why a call failed: Codex answered with an error, or the process did not answer.
#[derive(Debug)]
pub(super) enum CallError {
    Rpc(RpcError),
    Failed(BusinessError),
}

impl From<BusinessError> for CallError {
    fn from(error: BusinessError) -> Self {
        Self::Failed(error)
    }
}

impl AppServer {
    /// Spawns the app-server for `ctx` and runs the handshake; everything must be done within
    /// `timeout`.
    pub(super) fn start(ctx: &CliSessionCtx<'_>, timeout: Duration) -> Result<Self> {
        let args: Vec<String> = ARGS.iter().map(|arg| (*arg).to_owned()).collect();
        let spec = rpc_exec_spec(ctx.binary, ctx.project, &args)?
            .env("CODEX_HOME", codex_home(ctx.env))
            .env("NO_COLOR", "1")
            .envs(Codex.spawn_env().iter().copied())
            .search_dirs(launcher_search_dirs(ctx.env, &ctx.binary.path));

        let mut server = Self {
            child: LineChild::spawn(&spec, MAX_LINE_BYTES)?,
            deadline: Instant::now() + timeout,
        };

        let handshake = Request::Initialize(Initialize::desktop(env!("CARGO_PKG_VERSION")));
        server
            .call::<Value>(&handshake)
            .map_err(|error| failure(error, "initialize"))?;
        server.send(&initialized())?;

        Ok(server)
    }

    /// Sends `request` and returns its result as `T`.
    pub(super) fn call<T: DeserializeOwned>(&mut self, request: &Request) -> std::result::Result<T, CallError> {
        let id = next_request_id();
        self.send(&request.frame(&id))?;

        loop {
            let line = match self.child.read_line(self.deadline) {
                ReadLine::Line(line) => line,
                ReadLine::TooLong => return Err(too_large(request).into()),
                ReadLine::Closed => return Err(self.ended().into()),
                ReadLine::TimedOut => return Err(BusinessError::Timeout(format!("codex {}", request.method())).into()),
            };

            match decode(&line) {
                Ok(Inbound::Response(response)) if response.id == id => {
                    return response.result().map_err(CallError::Rpc);
                }
                Ok(Inbound::Request(asked)) => self.send(&refusal(&asked))?,
                // Notifications, stale responses and lines that are not frames.
                _ => {}
            }
        }
    }

    /// Closes stdin and waits a moment for the exit (the process is killed otherwise).
    pub(super) fn finish(self) {
        self.child.finish(Instant::now() + EXIT_GRACE);
    }

    /// Writes `frame`; a process that closed its input has stopped ([`Self::ended`]).
    fn send(&mut self, frame: &Value) -> Result<()> {
        match self.child.write_line(&frame.to_string()) {
            Ok(()) => Ok(()),
            Err(_) => Err(self.ended()),
        }
    }

    /// Why the process stopped answering: the last line of its stderr, redacted and cut.
    fn ended(&self) -> BusinessError {
        let stderr = plain_text(&self.child.stderr_tail(STDERR_GRACE));
        let last_line = stderr.lines().map(str::trim).rfind(|line| !line.is_empty());

        match last_line {
            Some(line) => BusinessError::Process(format!("codex app-server stopped: {}", short(line))),
            None => BusinessError::Process("codex app-server stopped before it answered".to_owned()),
        }
    }
}

/// A call's failure as an error naming `method`: Codex's own error text, redacted and cut.
pub(super) fn failure(error: CallError, method: &str) -> BusinessError {
    match error {
        CallError::Failed(error) => error,
        CallError::Rpc(error) => BusinessError::Process(format!("codex {method} failed: {}", short(&error.message))),
    }
}

/// The first line of `text`, redacted, at most [`MAX_ERROR_CHARS`] characters.
fn short(text: &str) -> String {
    let line = text.lines().next().unwrap_or_default().trim();

    redact_text(line).chars().take(MAX_ERROR_CHARS).collect()
}

fn too_large(request: &Request) -> BusinessError {
    BusinessError::Protocol(format!(
        "codex sent a {} reply larger than the app reads",
        request.method()
    ))
}
