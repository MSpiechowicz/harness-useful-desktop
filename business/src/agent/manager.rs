//! All running agent sessions of the app, addressed by session key.
//!
//! The manager validates what the business layer can check itself: the harness matches, the
//! project is an existing directory, a resumed session id is safe, the per-session approval
//! override and one-time trust are honoured only where the harness supports them, and at most
//! [`MAX_SESSIONS`] run at once. The rule that `project` must be the active or a recent project
//! is enforced by the caller (the desktop shell), as is the explicit user confirmation required
//! before [`AgentManager::bash`].
//!
//! At most one live agent runs per session id, whatever its harness (one folder can serve OMP
//! and PI): two agents writing one session file make the harness move the second one's writes to
//! a copy. Each agent is bound to the id it resumed, then to the id of every `state` it reports.
//! A start that resumes, or a switch to, an id held by a running or starting agent is refused;
//! a start that resumes one held by an agent that is still stopping (a page reload stops every
//! agent in the background) waits for that agent to exit. A [`SessionClaim`] holds an id without
//! an agent (while the session's files are deleted).
//!
//! A start still in flight when every session is stopped gives its startup up at once (or, when
//! startup already completed, stops its agent instead of registering it), so no agent outlives
//! [`AgentManager::begin_stop_all`]; its id counts as stopping until the agent exited. A startup
//! that never completes fails at its deadline, which releases the id as well.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio::time::Instant;

use super::SessionOptions;
use super::session::{AgentSession, SessionLaunch};
use super::sink::EventSink;
use super::slots::{
    BindingSink, Bound, Claimant, Entry, Holder, Reservation, SessionClaim, Slots, bind, open_elsewhere, still_stopping,
};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::harness::{Harness, LaunchRequest};
use crate::model::{AgentAction, AgentReply, AgentSessionInfo, AgentStartRequest, BashOutput, UiResponse};
use crate::pi_family::sessions::is_safe_session_id;
use crate::process::{ProcessSpawner, ResolvedBinary, launcher_search_dirs};
use crate::security::validate_project_dir;

/// Sessions that may run at once.
pub const MAX_SESSIONS: usize = 8;

/// How much longer than its stop grace a resume waits for the stopping agent of its session: the
/// session's kill wait (5 s) plus a second for the exit to be noticed.
const STOPPING_WAIT: Duration = Duration::from_secs(6);

/// Pause between two checks of a stopping agent.
const STOPPING_POLL: Duration = Duration::from_millis(50);

/// What `start` needs besides the request.
pub struct AgentDeps<'a> {
    /// The harness named by the request.
    pub harness: &'a dyn Harness,
    /// The environment (config lookups, session folders, `~` expansion).
    pub env: &'a Env,
    /// Starts the agent process.
    pub spawner: Arc<dyn ProcessSpawner>,
    /// The detected harness binary (with the user's override applied).
    pub binary: ResolvedBinary,
}

/// Agents taken out of the manager by [`AgentManager::begin_stop`] or
/// [`AgentManager::begin_stop_all`]. Their session ids stay held until they exit.
#[must_use = "the agents stop only when `finish` runs; dropping this kills them"]
pub struct Stopping {
    sessions: Vec<Arc<AgentSession>>,
}

impl Stopping {
    /// Stops the agents concurrently and waits until each delivered `exited`. The future owns
    /// the agents, so it can be spawned.
    pub async fn finish(self) {
        let mut stops = JoinSet::new();

        for session in self.sessions {
            stops.spawn(async move { session.stop().await });
        }

        while stops.join_next().await.is_some() {}
    }
}

/// See the module docs.
#[derive(Default)]
pub struct AgentManager {
    slots: Mutex<Slots>,
    /// [`Slots::generation`], watched by starts in flight to give up when every session stops.
    stopped_all: watch::Sender<u64>,
    options: SessionOptions,
}

impl AgentManager {
    /// A manager with production timing.
    pub fn new() -> Self {
        Self::default()
    }

    /// A manager with custom options (tests, fixture export).
    pub fn with_options(options: SessionOptions) -> Self {
        Self {
            slots: Mutex::default(),
            stopped_all: watch::Sender::default(),
            options,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Slots> {
        self.slots.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Reserves a start slot; `resume` is the resumed session id, which nothing may hold.
    fn reserve(&self, resume: Option<&str>) -> Result<Reservation<'_>> {
        let mut slots = self.lock();
        slots.prune();

        if let Some(id) = resume {
            match slots.holder(id, None) {
                Some(Holder::Open) => return Err(open_elsewhere()),
                Some(Holder::Stopping) => return Err(still_stopping()),
                None => {}
            }
        }

        if slots.sessions.len() + slots.starting >= MAX_SESSIONS {
            return Err(BusinessError::Unsupported(format!(
                "at most {MAX_SESSIONS} agent sessions can run at once"
            )));
        }

        let generation = slots.generation;
        let claim = resume.map(str::to_owned);

        if let Some(id) = &claim {
            slots.claimed.insert(id.clone(), Claimant::Start { generation });
        }

        slots.starting += 1;
        Ok(Reservation {
            slots: &self.slots,
            claim,
            generation,
        })
    }

    /// Waits until no stopping agent (or start that stops its own agent) holds session `id`, at
    /// most the stop grace plus [`STOPPING_WAIT`]. Fails at once when a running agent, another
    /// start or a claim holds it.
    async fn wait_until_released(&self, id: &str) -> Result<()> {
        let deadline = Instant::now() + self.options.stop_grace + STOPPING_WAIT;

        loop {
            let holder = self.lock().holder(id, None);

            match holder {
                None => return Ok(()),
                Some(Holder::Open) => return Err(open_elsewhere()),
                Some(Holder::Stopping) if Instant::now() >= deadline => return Err(still_stopping()),
                Some(Holder::Stopping) => tokio::time::sleep(STOPPING_POLL).await,
            }
        }
    }

    /// Starts a session; events go to `sink` from `ready` to `exited`.
    pub async fn start(
        &self,
        request: AgentStartRequest,
        deps: AgentDeps<'_>,
        sink: Arc<dyn EventSink>,
    ) -> Result<AgentSessionInfo> {
        let harness = deps.harness;

        if request.harness != harness.id() {
            return Err(BusinessError::InvalidInput(
                "the request names another harness".to_owned(),
            ));
        }

        let project = validate_project_dir(deps.env, Path::new(&request.project))?;
        let resume = match &request.resume {
            Some(session) if session.harness != harness.id() || !is_safe_session_id(&session.id) => {
                return Err(BusinessError::InvalidInput("invalid session to resume".to_owned()));
            }
            Some(session) => Some(session.id.as_str()),
            None => None,
        };

        if request.utility && request.resume.is_some() {
            return Err(BusinessError::InvalidInput(
                "a utility agent cannot resume a session".to_owned(),
            ));
        }

        let capabilities = harness.capabilities();
        let approval_override = request.approval_override.filter(|_| capabilities.approval_modes);
        let trust_once = request.trust_once && capabilities.project_trust;

        let spec = harness
            .launch_spec(&LaunchRequest {
                binary: &deps.binary,
                project: &project,
                resume,
                approval_override,
                trust_once,
                utility: request.utility,
            })?
            .search_dirs(launcher_search_dirs(deps.env, &deps.binary.path));

        if let Some(id) = resume {
            self.wait_until_released(id).await?;
        }

        let reservation = self.reserve(resume)?;
        let mut stopped_all = self.stopped_all.subscribe();
        let generation = reservation.generation;
        let abort = Box::pin(async move {
            stopped_all.wait_for(|current| *current != generation).await.ok();
        });
        let key = (self.options.key)();
        let bound: Bound = Arc::new(Mutex::new(resume.map(str::to_owned)));
        let sink = Arc::new(BindingSink {
            inner: sink,
            bound: bound.clone(),
        });

        let session = AgentSession::start(SessionLaunch {
            key: key.clone(),
            spec,
            spawner: deps.spawner.as_ref(),
            dialect: harness.dialect(),
            project: project.clone(),
            sessions_dir: harness.sessions_dir(deps.env, &project),
            sink,
            options: &self.options,
            abort,
        })
        .await?;

        let info = AgentSessionInfo {
            key: key.clone(),
            harness: harness.id(),
            project: project.to_string_lossy().into_owned(),
            started_at: (self.options.clock)(),
            approval_override,
            configured_approval_mode: harness.configured_approval_mode(deps.env, Some(&project)),
            trusted_once: trust_once,
            protocol_version: session.protocol_version(),
        };

        let entry = Entry {
            info: info.clone(),
            session: Arc::new(session),
            bound,
        };

        if let Some(stopping) = self.register(key, entry, reservation.generation) {
            stopping.finish().await;

            return Err(BusinessError::Process(
                "the session was stopped while it started".to_owned(),
            ));
        }

        Ok(info)
    }

    /// Adds a started agent. When every session was stopped since its start was reserved (at
    /// `generation`), the agent is parked instead (its id stays held until it exited) and
    /// returned to be stopped.
    fn register(&self, key: String, entry: Entry, generation: u64) -> Option<Stopping> {
        let mut slots = self.lock();

        if slots.generation != generation {
            return Some(Stopping {
                sessions: vec![slots.park(entry)],
            });
        }

        slots.sessions.insert(key, entry);
        None
    }

    /// True while an agent of any harness works on session `id` (running, stopping or starting)
    /// or a claim holds it.
    pub fn holds_session(&self, id: &str) -> bool {
        self.lock().holder(id, None).is_some()
    }

    /// Holds session `id` until the claim is dropped: starts that resume it and switches to it
    /// are refused meanwhile. `None` while an agent of any harness (running, starting or
    /// stopping) or another claim holds it.
    pub fn claim_session(&self, id: &str) -> Option<SessionClaim<'_>> {
        let claim = self.lock().claim(id, Claimant::Other)?;

        Some(SessionClaim {
            slots: &self.slots,
            claim,
        })
    }

    /// Holds the session id `action` switches session `key` to while the switch runs; refused when
    /// another chat, a start, a stopping agent or a claim holds it. The chat's own id is free.
    fn claim_target(&self, key: &str, action: &AgentAction) -> Result<Option<SessionClaim<'_>>> {
        let AgentAction::SwitchSession { session } = action else {
            return Ok(None);
        };

        let mut slots = self.lock();

        if slots.holder(&session.id, Some(key)).is_some() {
            return Err(open_elsewhere());
        }

        // Only the chat itself can still hold the id: it already works on it.
        let Some(claim) = slots.claim(&session.id, Claimant::Other) else {
            return Ok(None);
        };

        Ok(Some(SessionClaim {
            slots: &self.slots,
            claim,
        }))
    }

    fn entry<T>(&self, key: &str, read: impl FnOnce(&Entry) -> T) -> Result<T> {
        self.lock()
            .sessions
            .get(key)
            .map(read)
            .ok_or_else(|| BusinessError::NotFound(format!("agent session {key}")))
    }

    fn session(&self, key: &str) -> Result<Arc<AgentSession>> {
        self.entry(key, |entry| entry.session.clone())
    }

    /// Runs `action` in session `key`. A switch to a session id held elsewhere is refused. After
    /// an action that changes the session (new, switch, fork, clone) the agent is bound to its new
    /// session id before this returns.
    pub async fn send(&self, key: &str, action: AgentAction) -> Result<AgentReply> {
        let (session, bound) = self.entry(key, |entry| (entry.session.clone(), entry.bound.clone()))?;
        let _target = self.claim_target(key, &action)?;
        let changes_session = matches!(
            action,
            AgentAction::NewSession | AgentAction::SwitchSession { .. } | AgentAction::Fork { .. } | AgentAction::Clone
        );
        let reply = session.send(action).await?;

        if let AgentReply::State { state } = &reply {
            bind(&bound, &state.session_id);
        } else if changes_session {
            // The `state` event the session emits arrives later; a failed read leaves the binding
            // to it.
            if let Ok(AgentReply::State { state }) = session.send(AgentAction::GetState).await {
                bind(&bound, &state.session_id);
            }
        }

        Ok(reply)
    }

    /// Answers an open dialog of session `key`.
    pub async fn respond(&self, key: &str, response: UiResponse) -> Result<()> {
        self.session(key)?.respond(response).await
    }

    /// Runs `command` through the agent of session `key` (RPC `bash`). The caller must have the
    /// user's explicit confirmation.
    pub async fn bash(&self, key: &str, command: &str) -> Result<BashOutput> {
        self.session(key)?.bash(command).await
    }

    /// The info of session `key`.
    pub fn info(&self, key: &str) -> Result<AgentSessionInfo> {
        self.entry(key, |entry| entry.info.clone())
    }

    /// Moves session `key`, conversation included, to `project` (a canonical, authorized project
    /// directory whose session folder is `sessions_dir`); returns the updated info. The chat keeps
    /// going in the new project; a refused or failed move leaves it where it was.
    pub async fn move_project(&self, key: &str, project: PathBuf, sessions_dir: PathBuf) -> Result<AgentSessionInfo> {
        self.session(key)?.move_project(&project, &sessions_dir).await?;

        let mut slots = self.lock();
        let entry = slots
            .sessions
            .get_mut(key)
            .ok_or_else(|| BusinessError::NotFound(format!("agent session {key}")))?;
        entry.info.project = project.to_string_lossy().into_owned();

        Ok(entry.info.clone())
    }

    /// Takes session `key` out of the manager; its agent stops when the result is finished.
    /// Unknown keys give `None`.
    pub fn begin_stop(&self, key: &str) -> Option<Stopping> {
        let mut slots = self.lock();
        let entry = slots.sessions.remove(key)?;

        Some(Stopping {
            sessions: vec![slots.park(entry)],
        })
    }

    /// Takes every session out of the manager; their agents stop when the result is finished.
    /// Starts in flight give up and stop their agents themselves.
    pub fn begin_stop_all(&self) -> Stopping {
        let mut slots = self.lock();
        slots.generation = slots.generation.wrapping_add(1);
        self.stopped_all.send_replace(slots.generation);

        let entries: Vec<Entry> = slots.sessions.drain().map(|(_, entry)| entry).collect();

        Stopping {
            sessions: entries.into_iter().map(|entry| slots.park(entry)).collect(),
        }
    }

    /// Stops session `key` and forgets it; unknown keys are ignored.
    pub async fn stop(&self, key: &str) -> Result<()> {
        if let Some(stopping) = self.begin_stop(key) {
            stopping.finish().await;
        }

        Ok(())
    }

    /// Stops every session concurrently (app exit).
    pub async fn stop_all(&self) {
        self.begin_stop_all().finish().await;
    }

    /// The running sessions (sessions whose agent exited are forgotten).
    pub fn list(&self) -> Vec<AgentSessionInfo> {
        let mut slots = self.lock();
        slots.prune();

        let mut infos: Vec<AgentSessionInfo> = slots.sessions.values().map(|entry| entry.info.clone()).collect();
        infos.sort_by(|left, right| left.started_at.cmp(&right.started_at).then(left.key.cmp(&right.key)));
        infos
    }
}
