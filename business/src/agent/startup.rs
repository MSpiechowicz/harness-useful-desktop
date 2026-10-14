//! Startup of an [`AgentSession`]: handshake (OMP `ready` within 10 s; PI none) → the dialect's
//! post-handshake commands → `get_state` (PI's probe, also within 10 s) → the command list →
//! `ready`, `state` and `commandsUpdated` are emitted, then everything the agent sent meanwhile.
//!
//! The whole startup must finish within [`STARTUP_DEADLINE`](super::STARTUP_DEADLINE), which
//! open dialogs do not extend: dialogs are held back until startup completes, so nobody can
//! answer one, and an agent waiting for that answer would otherwise hold its session id forever.
//! A start is also given up as soon as every session is stopped. A failed or abandoned startup
//! kills the process and emits nothing.

use std::time::Duration;

use serde_json::Value;
use tokio::sync::oneshot;

use super::SessionOptions;
use super::session::AgentSession;
use super::sink::EmitMsg;
use crate::error::{BusinessError, Result};
use crate::model::{AgentAction, AgentEvent, AgentReply};
use crate::pi_family::rpc::pending::DEFAULT_TIMEOUT;
use crate::pi_family::rpc::{Handshake, RequestKind};
use crate::process::BoxFuture;

/// Resolves when a start must be given up (every session was stopped meanwhile).
pub type StartAbort<'a> = BoxFuture<'a, ()>;

impl AgentSession {
    /// Runs the startup within the deadline of `options`, unless `abort` resolves first.
    pub(super) async fn startup(
        &mut self,
        ready: oneshot::Receiver<Option<u32>>,
        options: &SessionOptions,
        abort: StartAbort<'_>,
    ) -> Result<()> {
        let deadline = options.startup_deadline;
        let steps = tokio::time::timeout(deadline, self.startup_steps(ready, options.startup_timeout));

        tokio::select! {
            result = steps => result.unwrap_or_else(|_| {
                Err(BusinessError::Timeout(format!(
                    "the agent did not finish starting within {} s",
                    deadline.as_secs_f64()
                )))
            }),
            () = abort => Err(BusinessError::Process("the session was stopped while it started".to_owned())),
        }
    }

    async fn startup_steps(&mut self, ready: oneshot::Receiver<Option<u32>>, limit: Duration) -> Result<()> {
        let dialect = self.shared.dialect;

        if dialect.handshake() == Handshake::ReadyFrame {
            self.protocol_version = tokio::time::timeout(limit, ready)
                .await
                .map_err(|_| {
                    BusinessError::Timeout(format!("the agent sent no ready frame within {} s", limit.as_secs()))
                })?
                .map_err(|_| BusinessError::Process("the agent exited during startup".to_owned()))?;
        }

        for mut frame in dialect.after_handshake() {
            let id = self.next_id();
            frame["id"] = Value::from(id.as_str());
            self.request(&frame, &id, RequestKind::Other, DEFAULT_TIMEOUT).await?;
        }

        for mut frame in dialect.optional_after_handshake() {
            let id = self.next_id();
            frame["id"] = Value::from(id.as_str());
            if let Err(error) = self.request(&frame, &id, RequestKind::Other, DEFAULT_TIMEOUT).await {
                tracing::debug!(%error, "the agent refused an optional startup command");
            }
        }

        let state = match self.call(&AgentAction::GetState, limit).await? {
            AgentReply::State { state } => state,
            _ => return Err(BusinessError::Protocol("get_state returned no state".to_owned())),
        };

        let commands = match self.call(&AgentAction::RefreshCommands, DEFAULT_TIMEOUT).await {
            Ok(AgentReply::Commands { commands }) => commands,
            _ => dialect.catalog().merged(&[]),
        };

        let first = vec![
            AgentEvent::Ready {
                protocol_version: self.protocol_version,
            },
            AgentEvent::State { state },
            AgentEvent::CommandsUpdated { commands },
        ];
        self.shared.events.send(EmitMsg::Open(first)).ok();

        Ok(())
    }
}

/// Adds the agent's last stderr lines to a startup error.
pub(super) fn with_stderr(error: BusinessError, tail: &str) -> BusinessError {
    if tail.is_empty() {
        return error;
    }

    match error {
        BusinessError::Timeout(message) => BusinessError::Timeout(format!("{message} (stderr: {tail})")),
        BusinessError::Process(message) => BusinessError::Process(format!("{message} (stderr: {tail})")),
        BusinessError::Protocol(message) => BusinessError::Protocol(format!("{message} (stderr: {tail})")),
        other => other,
    }
}
