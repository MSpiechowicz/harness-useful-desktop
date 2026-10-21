//! `!command`s and compaction of an [`OpenCodeV2Session`]: quiet runs of the chat's session
//! ([`QuietKind`]), shown as on OpenCode 1.
//!
//! - **`!command`:** `POST /api/session/{id}/shell {command}` answers (`204`) once the command
//!   ended; the output comes with `session.shell.ended`, before or after that answer, and reaches
//!   the caller with its exit code, redacted, cut like the RPC engine's and with the server
//!   password masked. OpenCode also hands it to the model with the next prompt. The chat refuses
//!   while it works, as on OpenCode 1. A command whose output never came fails after
//!   [`IDLE_GRACE`].
//! - **Compaction:** `POST /api/session/{id}/compact {}` admits it, then it runs in the background
//!   between `compaction` `start` and `end` with the session's model. OpenCode compacts with its own
//!   prompt, so instructions are refused. It is never cut short once its run started.
//! - A run's events stay hidden until its call answered and its run ended; a call answered
//!   without a run (or a `!command` without its end) ends the run after [`IDLE_GRACE`].

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::oneshot;

use super::ops::working;
use super::{Chat, Live, OpenCodeV2Session, stream};
use crate::agent::BASH_TIMEOUT;
use crate::agent::session::check_command;
use crate::error::{BusinessError, Result};
use crate::model::{AgentReply, BashOutput};
use crate::opencode_server::{HttpError, Response};
use crate::opencode_server_v2::RouteV2;
use crate::opencode_server_v2::mapper::{QuietKind, ShellResult, compaction_started};
use crate::opencode_server_v2::wire::{CompactBody, ShellBody};
use crate::pi_family::rpc::shared::parse_bash;

/// How long a run whose call answered waits for its run (or a `!command` for its end).
const IDLE_GRACE: Duration = Duration::from_secs(5);

/// Why a `!command` is refused while the agent works.
const BUSY: &str = "OpenCode is working: run the command once it is idle";

/// Why a `!command` without its output fails.
const NO_OUTPUT: &str = "OpenCode ran the command but did not report its output";

/// Why compaction instructions are refused.
const NO_INSTRUCTIONS: &str =
    "OpenCode compacts with its own summary prompt and takes no instructions. Compact without them";

impl OpenCodeV2Session {
    /// Runs `command` in the session's shell (see the module docs). The caller must have the
    /// user's explicit confirmation.
    pub async fn bash(&self, command: &str) -> Result<BashOutput> {
        self.process.ensure_running()?;
        check_command(command)?;

        let chat = &self.chat;
        let (session, generation, ended) = {
            let mut live = chat.lock();

            if live.mapper.is_working() {
                return Err(BusinessError::Unsupported(BUSY.to_owned()));
            }

            let generation = live.mapper.quiet_begin(QuietKind::Shell);
            let (sender, ended) = oneshot::channel();
            live.shell = Some((generation, sender));

            (live.session.clone(), generation, ended)
        };

        let route = RouteV2::Shell {
            session: &session,
            directory: &chat.directory,
        };
        let body = json_body(&ShellBody {
            command: command.to_owned(),
        });
        let sent = chat.client.send_with_timeout(route, Some(&body), BASH_TIMEOUT).await;

        let (ran, error) = outcome(&sent);
        answered(chat, generation, ran, error);

        let ended = match sent.and_then(Response::error_for_status) {
            Ok(_) => tokio::time::timeout(IDLE_GRACE, ended).await,
            Err(error) => {
                forget_shell(chat, generation);
                return Err(error.into());
            }
        };

        match ended {
            Ok(Ok(result)) => Ok(bash_output(chat, result)),
            _ => {
                forget_shell(chat, generation);
                Err(BusinessError::Protocol(NO_OUTPUT.to_owned()))
            }
        }
    }

    /// Compacts the context in the background (see the module docs).
    pub(super) async fn compact(&self, instructions: Option<&str>) -> Result<AgentReply> {
        if instructions.is_some_and(|instructions| !instructions.trim().is_empty()) {
            return Err(BusinessError::Unsupported(NO_INSTRUCTIONS.to_owned()));
        }

        let chat = self.chat.clone();
        let (session, generation) = {
            let mut live = chat.lock();

            if live.mapper.is_working() {
                return Err(working("compact the chat"));
            }

            let generation = live.mapper.quiet_begin(QuietKind::Compaction);
            chat.shared.emit(compaction_started());

            (live.session.clone(), generation)
        };

        tokio::spawn(async move {
            let route = RouteV2::Compact {
                session: &session,
                directory: &chat.directory,
            };
            let sent = chat.client.send(route, Some(&json_body(&CompactBody {}))).await;
            let (ran, error) = outcome(&sent);

            answered(&chat, generation, ran, error);
        });

        Ok(AgentReply::Ack)
    }
}

/// Whether OpenCode ran the call (it answered, and not `409`), and its error.
fn outcome(sent: &std::result::Result<Response, HttpError>) -> (bool, Option<String>) {
    match sent {
        Ok(response) if response.is_success() => (true, None),
        Ok(response) => {
            let status = response.status();
            (status != 409, Some(HttpError::Status { status }.to_string()))
        }
        Err(error) => (false, Some(error.to_string())),
    }
}

/// The run `generation` got its answer; a run still waiting for its events ends after
/// [`IDLE_GRACE`] (a compaction whose run started never does).
fn answered(chat: &Arc<Chat>, generation: u64, ran: bool, error: Option<String>) {
    {
        let mut live = chat.lock();
        let mapped = live.mapper.quiet_answered(ran, error);
        stream::apply(chat, &mut live, mapped);

        if live.mapper.quiet_kind().is_none() {
            return;
        }
    }

    let chat = chat.clone();

    tokio::spawn(async move {
        tokio::time::sleep(IDLE_GRACE).await;

        let mut live = chat.lock();
        let mapped = live.mapper.quiet_expire(generation);
        stream::apply(&chat, &mut live, mapped);
    });
}

/// Hands a `!command`'s end to the `bash` call waiting for it (`stream::apply`).
pub(super) fn shell_done(live: &mut Live, result: ShellResult) {
    if let Some((_, sender)) = live.shell.take() {
        // The call may have given up waiting.
        sender.send(result).ok();
    }
}

/// Drops the output slot of `!command` `generation`, which no longer waits.
fn forget_shell(chat: &Chat, generation: u64) {
    let mut live = chat.lock();

    if live.shell.as_ref().is_some_and(|(waiting, _)| *waiting == generation) {
        live.shell = None;
    }
}

/// `result` as the `bash` answer: cut to the size limit, the server password masked.
fn bash_output(chat: &Chat, result: ShellResult) -> BashOutput {
    let output = parse_bash(Some(&json!({
        "output": result.output,
        "truncated": result.truncated,
        "exitCode": result.exit,
    })));

    BashOutput {
        output: chat.shared.mask(&output.output, output.truncated),
        ..output
    }
}

/// The JSON of a request body (plain structs: serializing cannot fail).
fn json_body(body: &impl Serialize) -> Value {
    serde_json::to_value(body).unwrap_or(Value::Null)
}
