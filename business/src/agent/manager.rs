//! All running agent sessions of the app, addressed by session key.
//!
//! The manager validates what the business layer can check itself: the harness matches, the
//! project is an existing directory, a resumed session id is safe, the per-session approval
//! override and one-time trust are honoured only where the harness supports them, and at most
//! [`MAX_SESSIONS`] run at once. The rule that `project` must be the active or a recent project
//! is enforced by the caller (the desktop shell), as is the explicit user confirmation required
//! before [`AgentManager::bash`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tokio::task::JoinSet;

use super::SessionOptions;
use super::session::{AgentSession, SessionLaunch};
use super::sink::EventSink;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::harness::{Harness, LaunchRequest};
use crate::model::{AgentAction, AgentReply, AgentSessionInfo, AgentStartRequest, BashOutput, UiResponse};
use crate::pi_family::sessions::is_safe_session_id;
use crate::process::{ProcessSpawner, ResolvedBinary, launcher_search_dirs};
use crate::security::validate_project_dir;

/// Sessions that may run at once.
pub const MAX_SESSIONS: usize = 8;

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

struct Entry {
    info: AgentSessionInfo,
    session: Arc<AgentSession>,
}

#[derive(Default)]
struct Slots {
    sessions: HashMap<String, Entry>,
    starting: usize,
}

/// See the module docs.
#[derive(Default)]
pub struct AgentManager {
    slots: Mutex<Slots>,
    options: SessionOptions,
}

/// Releases a reserved start slot.
struct Reservation<'a>(&'a Mutex<Slots>);

impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        let mut slots = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        slots.starting = slots.starting.saturating_sub(1);
    }
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
            options,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Slots> {
        self.slots.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn reserve(&self) -> Result<Reservation<'_>> {
        let mut slots = self.lock();
        slots.sessions.retain(|_, entry| entry.session.is_running());

        if slots.sessions.len() + slots.starting >= MAX_SESSIONS {
            return Err(BusinessError::Unsupported(format!(
                "at most {MAX_SESSIONS} agent sessions can run at once"
            )));
        }

        slots.starting += 1;
        Ok(Reservation(&self.slots))
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
            })?
            .search_dirs(launcher_search_dirs(deps.env, &deps.binary.path));

        let _reservation = self.reserve()?;
        let key = (self.options.key)();

        let session = AgentSession::start(SessionLaunch {
            key: key.clone(),
            spec,
            spawner: deps.spawner.as_ref(),
            dialect: harness.dialect(),
            project: project.clone(),
            sessions_dir: harness.sessions_dir(deps.env, &project),
            sink,
            options: &self.options,
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

        self.lock().sessions.insert(
            key,
            Entry {
                info: info.clone(),
                session: Arc::new(session),
            },
        );

        Ok(info)
    }

    fn session(&self, key: &str) -> Result<Arc<AgentSession>> {
        self.lock()
            .sessions
            .get(key)
            .map(|entry| entry.session.clone())
            .ok_or_else(|| BusinessError::NotFound(format!("agent session {key}")))
    }

    /// Runs `action` in session `key`.
    pub async fn send(&self, key: &str, action: AgentAction) -> Result<AgentReply> {
        self.session(key)?.send(action).await
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
        self.lock()
            .sessions
            .get(key)
            .map(|entry| entry.info.clone())
            .ok_or_else(|| BusinessError::NotFound(format!("agent session {key}")))
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

    /// Stops session `key` and forgets it; unknown keys are ignored.
    pub async fn stop(&self, key: &str) -> Result<()> {
        let entry = self.lock().sessions.remove(key);

        if let Some(entry) = entry {
            entry.session.stop().await;
        }

        Ok(())
    }

    /// Stops every session concurrently (app exit).
    pub async fn stop_all(&self) {
        let entries: Vec<Entry> = self.lock().sessions.drain().map(|(_, entry)| entry).collect();
        let mut stopping = JoinSet::new();

        for entry in entries {
            stopping.spawn(async move { entry.session.stop().await });
        }

        while stopping.join_next().await.is_some() {}
    }

    /// The running sessions (sessions whose agent exited are forgotten).
    pub fn list(&self) -> Vec<AgentSessionInfo> {
        let mut slots = self.lock();
        slots.sessions.retain(|_, entry| entry.session.is_running());

        let mut infos: Vec<AgentSessionInfo> = slots.sessions.values().map(|entry| entry.info.clone()).collect();
        infos.sort_by(|left, right| left.started_at.cmp(&right.started_at).then(left.key.cmp(&right.key)));
        infos
    }
}
