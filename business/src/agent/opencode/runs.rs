//! `!command`s and compaction of an [`OpenCodeSession`]: quiet runs of the chat's session
//! ([`quiet`](crate::opencode_server::mapper::QuietKind)), shown the way OMP shows them.
//!
//! - **`!command`:** `POST /session/{id}/shell {agent, command, model?}` answers once the command
//!   ran, with the message OpenCode stored (the output also enters the context). The output is
//!   redacted and cut like the RPC engine's; OpenCode reports no exit code. It refuses while the
//!   session works (`409`), so the chat refuses first while it works.
//! - **Compaction:** `POST /session/{id}/summarize {providerID, modelID}` with the model the user
//!   picked, else the last reply's, else the one the chat shows while OpenCode picks. It runs in the background between `compaction`
//!   `start` and `end`; OpenCode compacts with its own prompt, so instructions are refused.
//! - A run's events stay hidden until its call answered and its `idle` arrived; a call answered
//!   without one ends the run after [`IDLE_GRACE`].
//! - The `idle` and the call's answer race, so a `!command` or session action asked for right after
//!   a run's call answered waits up to [`ANSWERED_WAIT`] for that run's `idle` before it counts the
//!   chat as working.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::time::Instant;

use super::startup::get_json;
use super::{Chat, OpenCodeSession, stream};
use crate::agent::BASH_TIMEOUT;
use crate::agent::session::check_command;
use crate::error::{BusinessError, Result};
use crate::model::{AgentReply, BashOutput};
use crate::opencode_server::mapper::{QuietKind, compaction_started, part_type, tool_outcome};
use crate::opencode_server::wire::ModelRef;
use crate::opencode_server::{HttpError, Response, Route, SessionId};
use crate::pi_family::rpc::shared::parse_bash;

/// How long a run whose call answered waits for its `idle`.
const IDLE_GRACE: Duration = Duration::from_secs(5);

/// How long an action waits for the `idle` of a run whose call answered.
const ANSWERED_WAIT: Duration = Duration::from_secs(2);

/// Messages read to find the last reply's model.
const MODEL_PAGE: u32 = 50;

/// Why a `!command` is refused while the agent works (also OpenCode's `409`).
const BUSY: &str = "OpenCode is working: run the command once it is idle";

/// Why compaction instructions are refused.
const NO_INSTRUCTIONS: &str =
    "OpenCode compacts with its own summary prompt and takes no instructions. Compact without them";

impl OpenCodeSession {
    /// Runs `command` in the session's shell (see the module docs). The caller must have the
    /// user's explicit confirmation.
    pub async fn bash(&self, command: &str) -> Result<BashOutput> {
        self.process.ensure_running()?;
        check_command(command)?;

        let chat = &self.chat;
        chat.settle_answered_run().await;

        let (session, body, generation) = {
            let mut live = chat.lock();

            if live.mapper.is_working() {
                return Err(BusinessError::Unsupported(BUSY.to_owned()));
            }

            let mut body = json!({ "agent": live.agent, "command": command });

            if let Some(model) = live.model.as_ref().filter(|_| live.model_chosen) {
                body["model"] = json!({ "providerID": model.provider, "modelID": model.id });
            }

            let generation = live.mapper.quiet_begin(QuietKind::Shell);
            (live.session.clone(), body, generation)
        };

        let sent = chat
            .client
            .send_with_timeout(Route::Shell { session: &session }, Some(&body), BASH_TIMEOUT)
            .await;

        answered(chat, generation, outcome(&sent).0, None);
        let response = sent?;

        if response.status() == 409 {
            return Err(BusinessError::Unsupported(BUSY.to_owned()));
        }

        let response = response.error_for_status()?;

        let output = parse_bash(Some(&json!({ "output": shell_output(&response.json()?) })));

        Ok(BashOutput {
            output: chat.shared.mask(&output.output, output.truncated),
            ..output
        })
    }

    /// Compacts the context in the background (see the module docs).
    pub(super) async fn compact(&self, instructions: Option<&str>) -> Result<AgentReply> {
        if instructions.is_some_and(|instructions| !instructions.trim().is_empty()) {
            return Err(BusinessError::Unsupported(NO_INSTRUCTIONS.to_owned()));
        }

        let session = self.idle_session("compact the chat").await?;
        let model = self.compaction_model(&session).await?;
        let chat = self.chat.clone();

        let generation = {
            let mut live = chat.lock();

            if live.mapper.is_working() || live.session != session {
                return Err(BusinessError::Unsupported(
                    "the agent is working. Wait until it is done, then compact the chat".to_owned(),
                ));
            }

            let generation = live.mapper.quiet_begin(QuietKind::Compaction);
            chat.shared.emit(compaction_started());
            generation
        };

        tokio::spawn(async move {
            let body = json!({ "providerID": model.provider, "modelID": model.id });
            let sent = chat
                .client
                .send(Route::Summarize { session: &session }, Some(&body))
                .await;
            let (ran, error) = outcome(&sent);

            answered(&chat, generation, ran, error);
        });

        Ok(AgentReply::Ack)
    }

    /// The model the user picked, else the model of the session's last reply, else the one the
    /// chat shows while OpenCode picks.
    async fn compaction_model(&self, session: &SessionId) -> Result<ModelRef> {
        let shown = {
            let live = self.chat.lock();

            match (&live.model, live.model_chosen) {
                (Some(model), true) => return Ok(model.clone()),
                (model, _) => model.clone(),
            }
        };

        let route = Route::Messages {
            session,
            limit: Some(MODEL_PAGE),
            before: None,
        };
        let page = get_json(&self.chat.client, route).await?;
        let model = page.as_array().into_iter().flatten().rev().find_map(|record| {
            let info = record.get("info")?;

            Some(ModelRef {
                provider: info.get("providerID")?.as_str()?.to_owned(),
                id: info.get("modelID")?.as_str()?.to_owned(),
            })
        });

        model.or(shown).ok_or_else(|| {
            BusinessError::Unsupported("OpenCode compacts with the chat's model: pick a model first".to_owned())
        })
    }
}

impl Chat {
    /// Waits up to [`ANSWERED_WAIT`] while the quiet run's call answered and only its `idle` is
    /// still to come (see the module docs).
    pub(super) async fn settle_answered_run(&self) {
        let deadline = Instant::now() + ANSWERED_WAIT;

        loop {
            let taken = self.event_taken.notified();
            tokio::pin!(taken);
            // Registered before the check, so an event taken in right after it still wakes us.
            taken.as_mut().enable();

            if !self.lock().mapper.quiet_awaits_idle() {
                return;
            }

            if tokio::time::timeout_at(deadline, taken).await.is_err() {
                return;
            }
        }
    }
}

/// Whether OpenCode ran the call (it answered other than `409`), and its error.
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

/// The run `generation` got its answer; a run still waiting for its `idle` ends after
/// [`IDLE_GRACE`].
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

/// The output of the command in a `shell` answer (`{info, parts}`): its tool part's output or
/// error, redacted.
fn shell_output(message: &Value) -> String {
    let parts = message.get("parts").and_then(Value::as_array).into_iter().flatten();

    parts
        .filter(|part| part_type(part) == "tool")
        .find_map(|part| part.get("state").and_then(tool_outcome))
        .map(|(output, _)| output)
        .unwrap_or_default()
}
