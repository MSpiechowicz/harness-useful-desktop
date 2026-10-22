//! Renaming, compaction and `!command`s of a [`CodexSession`], and what the end of a turn means for
//! the chat besides its events.
//!
//! - **Rename:** `thread/name/set` with the trimmed name (a blank one is refused); Codex echoes it
//!   (`thread/name/updated`), which the chat shows.
//! - **Compaction:** `thread/compact/start`, which Codex runs as its own turn with a
//!   `contextCompaction` item: the mapper's manual compaction ([`StartKind::Compact`]). Codex
//!   compacts with its own prompt, so instructions are refused.
//! - **`!command`** (plan decision 5): refused in Always ask ([`codex_bash_allowed`]), then
//!   `thread/shellCommand` ([`StartKind::Shell`]). Codex runs it in the thread's shell
//!   (unsandboxed, in the project) as its own turn and keeps the command and its output in the thread, where the model reads them
//!   with the next prompt. The answer waits for that turn's end, at most [`BASH_TIMEOUT`]: the
//!   output (redacted, capped and with the chat's secret masked, see [`parse_bash`]) and exit
//!   code. An abort interrupts the turn (`cancelled`); a turn that failed before the command ran
//!   fails with its error. The command shows no tool card of its own: the chat shows the answer.
//! - Both are refused while the chat is busy; one Codex does not take is taken back from the
//!   mapper.
//! - **A turn's end** marks the thread as saved (not a helper chat's), drops the history read
//!   before it, finishes a running `!command` and lets a helper chat send its prompt again
//!   (`naming.rs`).

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::oneshot;

use super::approvals::codex_bash_allowed;
use super::io::refused;
use super::ops::working;
use super::{Chat, CodexSession, Live};
use crate::agent::BASH_TIMEOUT;
use crate::agent::session::check_command;
use crate::codex_rpc::mapper::TurnEnd;
use crate::codex_rpc::wire::outbound::Request;
use crate::codex_rpc::{ShellRun, StartKind};
use crate::error::{BusinessError, Result};
use crate::model::{AgentAction, AgentReply, BashOutput, PromptStatus};
use crate::pi_family::rpc::pending::DEFAULT_TIMEOUT;
use crate::pi_family::rpc::shared::parse_bash;

/// Why compaction instructions are refused.
const NO_INSTRUCTIONS: &str =
    "Codex compacts with its own summary prompt and takes no instructions. Compact without them";

/// A running `!command`: its result once Codex reported it, and who waits for its turn's end.
pub(super) struct Shell {
    run: Option<ShellRun>,
    done: oneshot::Sender<ShellEnd>,
}

/// How a `!command`'s turn ended.
struct ShellEnd {
    run: Option<ShellRun>,
    status: PromptStatus,
    error: Option<String>,
}

impl CodexSession {
    /// Runs a compaction or a rename.
    pub(super) async fn thread_op(&self, action: AgentAction) -> Result<AgentReply> {
        match action {
            AgentAction::Compact { instructions } => self.compact(instructions.as_deref()).await,
            AgentAction::SetSessionName { name } => self.chat.rename(&name).await,
            _ => Err(BusinessError::Internal("not a thread action".to_owned())),
        }
    }

    /// Compacts the context (see the module docs).
    async fn compact(&self, instructions: Option<&str>) -> Result<AgentReply> {
        if instructions.is_some_and(|text| !text.trim().is_empty()) {
            return Err(BusinessError::Unsupported(NO_INSTRUCTIONS.to_owned()));
        }

        let compact = |thread_id| Request::ThreadCompactStart { thread_id };
        self.chat
            .quiet_turn(StartKind::Compact, "compact the chat", compact, None)
            .await?;

        Ok(AgentReply::Ack)
    }
}

impl Chat {
    /// Renames the thread (see the module docs).
    async fn rename(&self, name: &str) -> Result<AgentReply> {
        let name = name.trim();

        if name.is_empty() {
            return Err(BusinessError::InvalidInput("a chat name must not be blank".to_owned()));
        }

        let thread_id = self.lock().thread_id.clone().ok_or_else(no_thread)?;
        let request = Request::ThreadNameSet {
            thread_id,
            name: name.to_owned(),
        };
        self.request::<Value>(&request, DEFAULT_TIMEOUT).await?;

        Ok(AgentReply::Ack)
    }

    /// Runs `command` (see the module docs).
    pub(super) async fn run_command(self: &Arc<Self>, command: &str) -> Result<BashOutput> {
        check_command(command)?;
        codex_bash_allowed(self.lock().mode)?;

        let (done, ended) = oneshot::channel();
        let shell = |thread_id| Request::ThreadShellCommand {
            thread_id,
            command: command.to_owned(),
        };
        self.quiet_turn(StartKind::Shell, "run the command", shell, Some(done))
            .await?;

        let end = match tokio::time::timeout(BASH_TIMEOUT, ended).await {
            Ok(Ok(end)) => end,
            Ok(Err(_)) => return Err(BusinessError::Process("the agent session has exited".to_owned())),
            Err(_) => {
                self.lock().shell = None;
                return Err(BusinessError::Timeout("the command did not finish in time".to_owned()));
            }
        };

        if end.status == PromptStatus::Aborted {
            return Ok(BashOutput {
                output: String::new(),
                exit_code: None,
                cancelled: true,
                truncated: false,
            });
        }

        let Some(run) = end.run else {
            let reason = end.error.unwrap_or_else(|| "Codex ran no command".to_owned());
            return Err(BusinessError::Protocol(reason));
        };

        let output = parse_bash(Some(&json!({ "output": run.output, "exitCode": run.exit_code })));

        Ok(BashOutput {
            output: self.shared.mask(&output.output, output.truncated),
            ..output
        })
    }

    /// Starts a quiet turn of `kind` with the request `request` makes for the chat's thread;
    /// refused with `action` while the chat is busy. `shell`: who waits for a `!command`'s end.
    async fn quiet_turn(
        self: &Arc<Self>,
        kind: StartKind,
        action: &str,
        request: impl FnOnce(String) -> Request,
        shell: Option<oneshot::Sender<ShellEnd>>,
    ) -> Result<()> {
        let request = {
            let mut live = self.lock();

            if live.is_busy() {
                return Err(working(action));
            }

            let thread_id = live.thread_id.clone().ok_or_else(no_thread)?;
            live.mapper.sent(kind);
            live.shell = shell.map(|done| Shell { run: None, done });

            request(thread_id)
        };

        let error = match self.call(&request, DEFAULT_TIMEOUT).await {
            Ok(Ok(_)) => return Ok(()),
            Ok(Err(error)) => refused(request.method(), &error),
            Err(error) => error,
        };

        let owed = {
            let mut live = self.lock();
            live.shell = None;

            let mapped = live.mapper.withdrawn();
            self.apply(&mut live, mapped)
        };

        for frame in &owed {
            self.write(frame).await.ok();
        }

        Err(error)
    }

    /// The command of the running `!command` finished.
    pub(super) fn shell_output(&self, live: &mut Live, run: ShellRun) {
        if let Some(shell) = live.shell.as_mut() {
            shell.run = Some(run);
        }
    }

    /// A turn ended (see the module docs).
    pub(super) fn turn_ended(self: &Arc<Self>, live: &mut Live, end: &TurnEnd, owed: &mut Vec<Value>) {
        live.saved |= !self.helper;
        live.transcript = None;

        if end.kind == StartKind::Shell
            && let Some(shell) = live.shell.take()
        {
            let ended = ShellEnd {
                run: shell.run,
                status: end.status,
                error: end.error.clone(),
            };
            shell.done.send(ended).ok();
        }

        self.retry_naming(live, end, owed);
    }
}

fn no_thread() -> BusinessError {
    BusinessError::Internal("a Codex chat without a thread".to_owned())
}
