//! Agent routine runs (see "Agent runs" in `docs/routines-format.md`): [`RoutineAgents`] is the
//! [`AgentRunner`] the app installs at launch.
//!
//! - The chat starts like one of the Chat view ([`start_with`]) in the folder the runner
//!   authorized, with the routine's approval mode and one-time trust, and its events go to a
//!   [`RelaySink`]. While the session manager is full it tries again every 30 seconds, for 10
//!   minutes at most ("Too many chats open").
//! - It switches to the routine's model when one is set, then sends the prompt through the chat's
//!   hooks, so they fire like in any chat.
//! - The run is done when the agent settled after it took the prompt (a settle from before does
//!   not count) and the hooks have nothing left to do ([`ChatHooks::is_idle`]). Its excerpt is the last assistant text, redacted, at most 4 KiB.
//! - `maxMinutes` (default 30) limits the run from the moment its chat started (a wait for a free
//!   session does not count); a question nobody answers is handled by [`super::attention`].
//! - A run that ended well leaves its chat to the window that shows it, until the window leaves or
//!   closes the chat. Any other end (a stop, a failure, a prompt that ended with an error, the time
//!   limit) stops the agent, shown or not. An agent that exits before the run is done fails the run, also when a window showed it.
//! - The run's relay is known before its chat starts, so a page reload during the start spares the
//!   chat once its first event arrived; a start that failed while the page reloaded is tried once
//!   more.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use harness_business::env::Env;
use harness_business::model::{
    AgentSessionInfo, AgentStartRequest, ApprovalMode, HarnessId, IpcErrorCode, RoutineAction, RoutineModel,
};
use harness_business::process::BoxFuture;
use harness_business::routines::session_tags::SessionTagIndex;
use harness_business::routines::{
    SessionTags, checked_workspace, forget_workspaces, routine_workspaces, workspace_in_use, workspace_owner,
};
use tauri::{AppHandle, Manager, Runtime};
use tokio::time::{Instant, sleep};

use super::RoutineRuntime;
use super::agent_chat::{Chat, Ended, Watch};
use super::attention::{UNANSWERED_MESSAGE, ui_wait};
use super::job::{AgentRunner, RunJob, RunOutcome, StopSignal, excerpt};
use super::progress::RunProgress;
use super::relay::RelaySink;
use crate::commands::agent::{AuthorizedFolder, start_with};
use crate::error::CommandResult;
use crate::state::AppState;

/// The message of a run whose session manager stayed full.
pub const FULL_MESSAGE: &str = "Too many chats open";

/// The message of a run whose chat could not start.
pub const START_FAILED: &str = "The chat could not start";

/// The message of a run whose harness is not installed.
pub const NOT_INSTALLED: &str = "The agent is not installed";

/// The message of a run whose harness does not trust the folder yet (Claude Code).
pub const UNTRUSTED_MESSAGE: &str = "The agent does not trust this folder yet. Run it there once from a terminal, or let the routine trust the project once";

/// The message of a run whose agent refused the prompt.
pub const PROMPT_FAILED: &str = "The agent did not take the prompt";

/// The message of a run whose last prompt ended with an error.
pub const AGENT_FAILED: &str = "The agent run failed";

/// The message of a run whose agent exited before it was done.
pub const AGENT_GONE: &str = "The agent stopped before it finished";

/// `maxMinutes` when the routine sets none.
const DEFAULT_MAX_MINUTES: u32 = 30;

/// Gives the runner the app's state.
pub trait StateHost: Send + Sync + 'static {
    /// The state.
    fn app_state(&self) -> &AppState;
}

impl<R: Runtime> StateHost for AppHandle<R> {
    fn app_state(&self) -> &AppState {
        Manager::state::<AppState>(self).inner()
    }
}

/// The times of an agent run (shorter in tests).
#[derive(Debug, Clone, Copy)]
pub struct AgentTiming {
    /// One of `maxMinutes`' minutes.
    pub minute: Duration,
    /// The pause before a start is tried again while the session manager is full.
    pub full_retry: Duration,
    /// How long a start is tried again.
    pub full_limit: Duration,
    /// How long a question waits while no window shows the run.
    pub ui_wait: Duration,
    /// How often a settled run asks the hooks whether they are done.
    pub idle_poll: Duration,
}

impl AgentTiming {
    /// The documented times (the question wait from `env` in a debug build, see
    /// [`super::attention::ui_wait`]).
    pub fn new(env: &Env) -> Self {
        Self {
            minute: Duration::from_secs(60),
            full_retry: Duration::from_secs(30),
            full_limit: Duration::from_secs(10 * 60),
            ui_wait: ui_wait(env),
            idle_poll: Duration::from_millis(100),
        }
    }
}

impl RoutineRuntime {
    /// `<root>/workspace`, which holds the folders global routines run and keep their sessions in,
    /// as a canonical path once it exists ("Global routines" in Sessions). `None` while it is
    /// missing, a symbolic link, or not a plain folder of the routines folder.
    pub fn workspace(&self) -> Option<PathBuf> {
        checked_workspace(self.store.as_ref()?.root(), None).ok()
    }

    /// The working folders of the global routines (`<root>/workspace/<id>` and
    /// `<root>/workspace/team-<id>`) whose routine is still there, checked.
    pub fn routine_workspaces(&self) -> Vec<PathBuf> {
        let Some(store) = self.store.as_ref() else {
            return Vec::new();
        };

        routine_workspaces(store.root())
            .into_iter()
            .filter(|folder| {
                let name = folder.file_name().and_then(|name| name.to_str());
                name.is_some_and(|name| workspace_in_use(store.root(), name))
            })
            .collect()
    }

    /// `project` when it names the routines workspace (exactly as [`RoutineRuntime::workspace`]
    /// gives it) or the folder in it of a routine that is still there, as a checked canonical
    /// path.
    pub fn workspace_folder(&self, project: &str) -> Option<PathBuf> {
        let parent = self.workspace()?;

        if parent.as_os_str() == project {
            return Some(parent);
        }

        let folder = Path::new(project);
        let name = folder.file_name()?.to_str()?;
        let root = self.store.as_ref()?.root();

        if folder.parent() != Some(parent.as_path()) || !workspace_in_use(root, name) {
            return None;
        }

        checked_workspace(root, Some(name)).ok()
    }

    /// Deletes the workspace folders of routines that are gone (deleted, or a team copy that left)
    /// unless a run of theirs still goes on or waits. Blocks: call it off the async runtime.
    pub fn forget_workspaces(&self) {
        let Some(store) = self.store.as_ref() else {
            return;
        };

        let busy = self.busy();
        let forgotten = forget_workspaces(store.root(), |name| {
            workspace_owner(name).is_some_and(|(id, _)| busy.contains(id))
        });

        if !forgotten.is_empty() {
            tracing::info!(
                folders = forgotten.len(),
                "workspace folders of routines that are gone deleted"
            );
        }
    }

    /// [`RoutineRuntime::forget_workspaces`] off the async runtime.
    pub async fn forget_workspaces_soon(self: &Arc<Self>) {
        let runtime = self.clone();

        let forgot = super::off_runtime(move || {
            runtime.forget_workspaces();
            Ok(())
        })
        .await;

        if forgot.is_err() {
            tracing::warn!("the workspace folders of routines that are gone were not looked at");
        }
    }

    /// The sessions routine runs started (none without a local data folder). Reads a file: call
    /// it off the async runtime.
    pub fn session_tags(&self) -> SessionTagIndex {
        self.local_data
            .as_deref()
            .map(|local_data| SessionTags::new(local_data).index())
            .unwrap_or_default()
    }
}

/// Runs agent routines in the chats of `app` from now on.
pub fn install<R: Runtime>(app: &AppHandle<R>) {
    let state = Manager::state::<AppState>(app);
    let agents = RoutineAgents::new(Arc::new(app.clone()), AgentTiming::new(&state.env));

    state.routines.set_agent_runner(Arc::new(agents));
}

/// See the module docs.
pub struct RoutineAgents {
    host: Arc<dyn StateHost>,
    timing: AgentTiming,
}

impl RoutineAgents {
    /// Runs agent routines in `host`'s state with `timing`.
    pub fn new(host: Arc<dyn StateHost>, timing: AgentTiming) -> Self {
        Self { host, timing }
    }
}

impl AgentRunner for RoutineAgents {
    fn run(&self, job: RunJob, progress: RunProgress, stop: StopSignal) -> BoxFuture<'static, RunOutcome> {
        let (host, timing) = (self.host.clone(), self.timing);

        Box::pin(async move { run(host, timing, job, progress, stop).await })
    }
}

/// The agent action of a routine.
pub(super) struct Order {
    pub(super) harness: HarnessId,
    pub(super) model: Option<RoutineModel>,
    pub(super) prompt: String,
    approval_mode: Option<ApprovalMode>,
    trust_once: bool,
    max_minutes: u32,
}

impl Order {
    fn of(action: &RoutineAction) -> Option<Self> {
        let RoutineAction::Agent {
            harness,
            model,
            prompt,
            approval_mode,
            trust_once,
            max_minutes,
        } = action
        else {
            return None;
        };

        Some(Self {
            harness: *harness,
            model: model.clone(),
            prompt: prompt.clone(),
            approval_mode: *approval_mode,
            trust_once: *trust_once,
            max_minutes: max_minutes.unwrap_or(DEFAULT_MAX_MINUTES),
        })
    }

    /// "Stopped after N minutes".
    fn time_up(&self) -> RunOutcome {
        let unit = if self.max_minutes == 1 { "minute" } else { "minutes" };

        RunOutcome::failed(&format!("Stopped after {} {unit}", self.max_minutes))
    }
}

async fn run(
    host: Arc<dyn StateHost>,
    timing: AgentTiming,
    job: RunJob,
    progress: RunProgress,
    mut stop: StopSignal,
) -> RunOutcome {
    let state = host.app_state();

    let (Some(order), Some(folder)) = (Order::of(&job.routine.action), job.folder.clone()) else {
        return RunOutcome::failed(START_FAILED);
    };

    let (relay, mut signals) = RelaySink::new(&job.run.record.id);

    // Known before the start: a page reload spares the chat as soon as its key is.
    state.routines.relays().insert(relay.clone());

    let info = match start(state, timing, &order, &folder, &relay, &mut stop).await {
        Ok(info) => info,
        Err(outcome) => {
            state.routines.relays().remove(relay.run_id());
            return outcome;
        }
    };

    tracing::info!(run = job.run.record.id, session = %info.key, "routine chat started");

    // The time limit counts from here: waiting for a free session is not the run's time.
    let deadline = Instant::now() + timing.minute * order.max_minutes;

    let chat = Chat {
        state,
        hooks: state.hooks.chat(&info.key),
        key: info.key,
        relay,
    };

    let mut watch = Watch::new(progress, order.harness, timing);

    let ended = match chat.set_model(&order).await {
        Ok(()) => watch.until_end(&chat, &order, &mut signals, &mut stop, deadline).await,
        Err(outcome) => Ended::Refused(outcome),
    };

    watch.progress.needs_you(false);
    chat.relay.set_waiting(false);

    // Only a run that ended well leaves its chat to a window: a stop or a failed prompt ends the
    // agent too.
    let may_stay = matches!(ended, Ended::Done) && !watch.failed;

    let mut outcome = match ended {
        Ended::Done if watch.failed => RunOutcome::failed(AGENT_FAILED),
        Ended::Done => RunOutcome::ok(),
        Ended::Stopped(reason) => RunOutcome::stopped(reason),
        Ended::TimeUp => order.time_up(),
        Ended::Unanswered => {
            chat.cancel(watch.attention.open()).await;
            RunOutcome::needs_attention(UNANSWERED_MESSAGE)
        }
        Ended::Exited => RunOutcome::failed(AGENT_GONE),
        Ended::Refused(outcome) => outcome,
    };

    outcome.excerpt = watch.said.as_deref().and_then(excerpt);

    chat.end(&host, may_stay, signals);

    outcome
}

/// Starts the chat, trying again while the session manager is full (and once more when a page
/// reload got in the way). The failed outcome when it cannot start, or the run is stopped first.
async fn start(
    state: &AppState,
    timing: AgentTiming,
    order: &Order,
    folder: &Path,
    relay: &Arc<RelaySink>,
    stop: &mut StopSignal,
) -> Result<AgentSessionInfo, RunOutcome> {
    let give_up = Instant::now() + timing.full_limit;
    let mut retried_reload = false;

    loop {
        let reloads = state.routines.relays().reloads();
        let request = AgentStartRequest {
            harness: order.harness,
            project: String::new(),
            resume: None,
            approval_override: order.approval_mode,
            trust_once: order.trust_once,
            utility: false,
            memory_chat: None,
        };

        let started: CommandResult<AgentSessionInfo> =
            start_with(state, request, relay.clone(), AuthorizedFolder::of_routine(folder)).await;

        let error = match started {
            Ok(info) => return Ok(info),
            Err(error) => error,
        };

        let full = error.code() == IpcErrorCode::Unsupported && state.agents.manager().is_full();
        let reloaded = state.routines.relays().reloads() != reloads;

        if reloaded && !retried_reload && !state.agents.is_shutting_down() {
            tracing::info!("a page reload stopped a routine chat while it started: trying again");
            retried_reload = true;
            continue;
        }

        if !full {
            tracing::warn!(code = ?error.code(), "a routine chat failed to start");

            return Err(match error.code() {
                IpcErrorCode::NotFound => RunOutcome::failed(NOT_INSTALLED),
                IpcErrorCode::Untrusted | IpcErrorCode::UntrustedUnreadable => RunOutcome::failed(UNTRUSTED_MESSAGE),
                _ => RunOutcome::failed(START_FAILED),
            });
        }

        let now = Instant::now();

        if now >= give_up {
            return Err(RunOutcome::failed(FULL_MESSAGE));
        }

        tracing::info!("a routine chat waits for a free session");

        tokio::select! {
            reason = stop.stopped() => return Err(RunOutcome::stopped(reason)),
            () = sleep(timing.full_retry.min(give_up - now)) => {}
        }
    }
}
