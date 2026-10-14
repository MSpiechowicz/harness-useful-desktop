//! Agent chat commands: start a harness in RPC mode with its events streamed over a channel,
//! drive it with typed actions, answer its dialogs, run confirmed shell commands, stop it.
//!
//! The business [`AgentManager`](harness_business::agent::AgentManager) validates the request
//! itself; this layer adds the rules only the shell can check: the project must be the active or a
//! recent one, the harness must be detected as installed (its resolved binary is used), and
//! `agent_bash` needs the user's confirmation.

use std::path::PathBuf;
use std::sync::Arc;

use harness_business::BusinessError;
use harness_business::agent::AgentDeps;
use harness_business::model::{
    AgentAction, AgentEnvelope, AgentReply, AgentSessionInfo, AgentStartRequest, BashOutput, UiResponse,
};
use harness_business::project::Projects;
use tauri::State;
use tauri::ipc::Channel;

use super::blocking;
use super::harness::{find, installed_binary};
use crate::agent_sink::{ChannelSink, OnClosed};
use crate::agents::Agents;
use crate::error::CommandResult;
use crate::state::AppState;

/// Starts a session of `request.harness` in `request.project`; its events go to `on_event` from
/// `ready` to `exited`. At most 8 sessions run at once (`unsupported` beyond).
#[tauri::command]
pub async fn agent_start(
    state: State<'_, AppState>,
    request: AgentStartRequest,
    on_event: Channel<AgentEnvelope>,
) -> CommandResult<AgentSessionInfo> {
    start(&state, request, on_event).await
}

/// Runs `action` in session `key`.
#[tauri::command]
pub async fn agent_send(state: State<'_, AppState>, key: String, action: AgentAction) -> CommandResult<AgentReply> {
    Ok(state.agents.manager().send(&key, action).await?)
}

/// Answers an open dialog of session `key`.
#[tauri::command]
pub async fn agent_ui_respond(state: State<'_, AppState>, key: String, response: UiResponse) -> CommandResult<()> {
    Ok(state.agents.manager().respond(&key, response).await?)
}

/// Runs `command` through the agent of session `key`. `confirmed` must be true: the UI asks the
/// user before sending it (`invalidInput` otherwise).
#[tauri::command]
pub async fn agent_bash(
    state: State<'_, AppState>,
    key: String,
    command: String,
    confirmed: bool,
) -> CommandResult<BashOutput> {
    bash(&state, &key, &command, confirmed).await
}

/// Moves chat `key`, conversation included, to `project` (the active project or a recent one) and
/// returns its updated info. Only harnesses that can move a session support it (OMP `/move`).
#[tauri::command]
pub async fn agent_move_project(
    state: State<'_, AppState>,
    key: String,
    project: String,
) -> CommandResult<AgentSessionInfo> {
    let manager = state.agents.manager();
    let harness = find(&state, manager.info(&key)?.harness)?;
    let project = PathBuf::from(authorized_project(&state, project).await?);
    let sessions_dir = harness.sessions_dir(&state.env, &project);

    Ok(manager.move_project(&key, project, sessions_dir).await?)
}

/// Stops session `key` (close stdin → 3 s → kill); unknown keys are ignored.
#[tauri::command]
pub async fn agent_stop(state: State<'_, AppState>, key: String) -> CommandResult<()> {
    Ok(state.agents.manager().stop(&key).await?)
}

/// The running sessions, oldest first.
#[tauri::command]
pub async fn agent_list(state: State<'_, AppState>) -> CommandResult<Vec<AgentSessionInfo>> {
    Ok(state.agents.manager().list())
}

async fn start(
    state: &AppState,
    mut request: AgentStartRequest,
    channel: Channel<AgentEnvelope>,
) -> CommandResult<AgentSessionInfo> {
    let agents = &state.agents;

    if agents.is_shutting_down() {
        return Err(shutting_down().into());
    }

    let harness = find(state, request.harness)?;
    request.project = authorized_project(state, request.project).await?;
    let binary = installed_binary(state, request.harness).await?;

    let sink = Arc::new(ChannelSink::new(channel, request.harness, stop_when_closed(agents)));
    let deps = AgentDeps {
        harness,
        env: &state.env,
        spawner: agents.spawner(),
        binary,
    };

    let info = match agents.manager().start(request, deps, sink.clone()).await {
        Ok(info) => info,
        Err(error) => {
            tracing::warn!(harness = ?harness.id(), code = ?error.code(), "an agent session failed to start");
            return Err(error.into());
        }
    };

    // The channel closed (or shutdown began) while the session started: nobody would see it.
    if sink.is_closed() || agents.is_shutting_down() {
        agents.manager().stop(&info.key).await?;

        return Err(BusinessError::Process("the session ended before it could be shown".to_owned()).into());
    }

    tracing::info!(session = %info.key, harness = ?info.harness, "agent session started");

    Ok(info)
}

async fn bash(state: &AppState, key: &str, command: &str, confirmed: bool) -> CommandResult<BashOutput> {
    if !confirmed {
        return Err(BusinessError::InvalidInput("the user has not confirmed the command".to_owned()).into());
    }

    Ok(state.agents.manager().bash(key, command).await?)
}

fn shutting_down() -> BusinessError {
    BusinessError::Unsupported("the app is shutting down".to_owned())
}

/// The canonical directory of `project`, which must be the active project or a recent one.
async fn authorized_project(state: &AppState, project: String) -> CommandResult<String> {
    let prefs = state.prefs.clone();
    let env = state.env.clone();

    let path: Option<PathBuf> =
        blocking(move || Ok(prefs.with(|store| Projects::new(store, &env).authorize(Some(&project)))?)).await?;

    let path = path.ok_or_else(|| BusinessError::Internal("an authorized project has no path".to_owned()))?;

    Ok(path.to_string_lossy().into_owned())
}

/// Stops the session in the background once its channel closes. Holds the agents weakly: the
/// sink lives inside a session the agents own.
fn stop_when_closed(agents: &Arc<Agents>) -> OnClosed {
    let agents = Arc::downgrade(agents);

    Box::new(move |key| {
        if let Some(agents) = agents.upgrade() {
            agents.stop_in_background(key.to_owned());
        }
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    use harness_business::agent::{AgentManager, SessionOptions};
    use harness_business::env::Env;
    use harness_business::model::{HarnessId, IpcErrorCode};
    use harness_business::process::{ExecSpec, ProcessSpawner, SpawnedProcess, TokioSpawner};
    use serde_json::Value;
    use tauri::async_runtime::block_on;

    use super::*;
    use crate::test_support::{TempDir, cache_detection, collecting_channel};

    /// How long a test waits for the fake agent.
    const WAIT: Duration = Duration::from_secs(10);

    /// Set to `1` (CI) to make the tests that need `fake_agent` fail instead of skipping.
    const REQUIRE_FAKE_AGENT: &str = "HARNESS_REQUIRE_FAKE_AGENT";

    /// `fake_agent` from the business crate, built next to this test binary by
    /// `cargo test --workspace` (Cargo exposes `CARGO_BIN_EXE_*` only to the owning package).
    /// `None` (the caller skips) when it is missing, unless [`REQUIRE_FAKE_AGENT`] is `1`.
    fn fake_agent() -> Option<PathBuf> {
        let found = built_fake_agent();

        if found.is_none() {
            assert_ne!(
                std::env::var(REQUIRE_FAKE_AGENT).as_deref(),
                Ok("1"),
                "fake_agent is not built but {REQUIRE_FAKE_AGENT}=1 \
                 (run `cargo build -p harness_business --bin fake_agent` first)"
            );
            eprintln!("skipped: fake_agent is not built (run `cargo test --workspace`)");
        }

        found
    }

    fn built_fake_agent() -> Option<PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let target_dir = exe.parent()?.parent()?;
        let path = target_dir.join(format!("fake_agent{}", std::env::consts::EXE_SUFFIX));

        path.is_file().then_some(path)
    }

    /// Runs `fake_agent --dialect omp --scenario <scenario>` in the requested directory.
    struct ScenarioSpawner {
        binary: PathBuf,
        scenario: PathBuf,
        spawned: AtomicUsize,
    }

    impl ProcessSpawner for ScenarioSpawner {
        fn spawn(&self, spec: &ExecSpec) -> harness_business::Result<SpawnedProcess> {
            self.spawned.fetch_add(1, Ordering::Relaxed);

            let scenario = self.scenario.to_string_lossy().into_owned();
            let mut fake = ExecSpec::new(&self.binary).args(["--dialect", "omp", "--scenario", scenario.as_str()]);
            fake.cwd = spec.cwd.clone();

            TokioSpawner.spawn(&fake)
        }
    }

    struct Setup {
        state: AppState,
        spawner: Arc<ScenarioSpawner>,
        project: PathBuf,
        _dir: TempDir,
    }

    /// An app state with an opened project, OMP detected at `binary` and sessions that play the
    /// `scenario` stream.
    fn setup(binary: &Path, scenario: &str) -> Setup {
        let dir = TempDir::new("agent");
        let home = dir.path().join("home");
        let project = home.join("demo");
        std::fs::create_dir_all(project.join("src")).expect("create project");

        let mut state = AppState::new(dir.path().join("config"), Env::for_tests(&home));
        state.prefs.with(|store| {
            Projects::new(store, &state.env).open(&project).expect("open project");
        });

        let spawner = Arc::new(ScenarioSpawner {
            binary: binary.to_path_buf(),
            scenario: Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../business/tests/fixtures/streams")
                .join(scenario),
            spawned: AtomicUsize::new(0),
        });
        let options = SessionOptions {
            coalesce_window: Duration::ZERO,
            ..SessionOptions::default()
        };
        state.agents = Arc::new(Agents::with(AgentManager::with_options(options), spawner.clone()));

        cache_detection(&state, HarnessId::Omp, Some(binary));

        Setup {
            state,
            spawner,
            project,
            _dir: dir,
        }
    }

    fn request(project: &Path) -> AgentStartRequest {
        AgentStartRequest {
            harness: HarnessId::Omp,
            project: project.to_string_lossy().into_owned(),
            resume: None,
            approval_override: None,
            trust_once: false,
            utility: false,
        }
    }

    /// A channel that delivers while `open` is set and fails like a destroyed webview after.
    fn closable_channel(open: bool) -> (Channel<AgentEnvelope>, Arc<AtomicBool>, Arc<AtomicUsize>) {
        let open = Arc::new(AtomicBool::new(open));
        let delivered = Arc::new(AtomicUsize::new(0));
        let (is_open, count) = (open.clone(), delivered.clone());

        let channel = Channel::new(move |_| {
            if !is_open.load(Ordering::Acquire) {
                return Err(tauri::Error::WebviewNotFound);
            }

            count.fetch_add(1, Ordering::AcqRel);
            Ok(())
        });

        (channel, open, delivered)
    }

    fn event_types(messages: &Mutex<Vec<Value>>) -> Vec<String> {
        let messages = messages.lock().unwrap();

        messages
            .iter()
            .map(|message| message["event"]["type"].as_str().unwrap_or_default().to_owned())
            .collect()
    }

    /// Polls `condition` (the session runs on the async runtime's worker threads).
    fn wait_until(what: &str, condition: impl Fn() -> bool) {
        let deadline = Instant::now() + WAIT;

        while !condition() {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn a_session_streams_its_events_over_the_channel_until_it_stops() {
        let Some(binary) = fake_agent() else {
            return;
        };
        let setup = setup(&binary, "omp_basic.jsonl");
        let agents = &setup.state.agents;
        let (channel, received) = collecting_channel();

        let info = block_on(start(&setup.state, request(&setup.project), channel)).expect("session starts");
        assert_eq!(info.harness, HarnessId::Omp);
        assert_eq!(Path::new(&info.project), setup.project);
        assert_eq!(agents.manager().list(), std::slice::from_ref(&info));

        let prompt = AgentAction::Prompt {
            message: "Make `parse` handle empty input.".to_owned(),
            streaming_behavior: None,
        };
        let reply = block_on(agents.manager().send(&info.key, prompt)).expect("prompt accepted");
        assert!(matches!(reply, AgentReply::Prompt { .. }), "{reply:?}");
        wait_until("the settled event", || {
            event_types(&received).contains(&"settled".to_owned())
        });

        block_on(agents.manager().stop(&info.key)).expect("stop");

        let types = event_types(&received);
        assert_eq!(types[..3], ["ready", "state", "commandsUpdated"]);
        assert_eq!(types.last().map(String::as_str), Some("exited"));
        assert!(types.contains(&"textDelta".to_owned()), "{types:?}");

        let messages = received.lock().unwrap();
        let seqs: Vec<u64> = messages
            .iter()
            .map(|message| message["seq"].as_u64().unwrap())
            .collect();
        assert!(seqs.windows(2).all(|pair| pair[0] < pair[1]), "{seqs:?}");
        assert!(
            messages
                .iter()
                .all(|message| message["sessionKey"] == info.key.as_str())
        );
        assert!(agents.manager().list().is_empty());
    }

    #[test]
    fn a_session_is_stopped_when_its_channel_closes() {
        let Some(binary) = fake_agent() else {
            return;
        };
        let setup = setup(&binary, "omp_basic.jsonl");
        let agents = setup.state.agents.clone();
        let (channel, open, delivered) = closable_channel(true);

        let info = block_on(start(&setup.state, request(&setup.project), channel)).expect("session starts");
        wait_until("the startup events", || delivered.load(Ordering::Acquire) >= 3);

        open.store(false, Ordering::Release);
        let prompt = AgentAction::Prompt {
            message: "hello".to_owned(),
            streaming_behavior: None,
        };
        block_on(agents.manager().send(&info.key, prompt)).expect("prompt accepted");

        wait_until("the session to stop", || agents.manager().list().is_empty());
        block_on(agents.stop_all());
    }

    #[test]
    fn a_channel_closed_during_startup_leaves_no_session() {
        let Some(binary) = fake_agent() else {
            return;
        };
        let setup = setup(&binary, "omp_basic.jsonl");
        let agents = setup.state.agents.clone();
        let (channel, _, _) = closable_channel(false);

        // Depending on timing the first send fails before or after `start` returns.
        let started = block_on(start(&setup.state, request(&setup.project), channel));
        if let Err(error) = &started {
            assert_eq!(error.code(), IpcErrorCode::Process);
        }

        wait_until("the session to stop", || agents.manager().list().is_empty());
        block_on(agents.stop_all());
        assert_eq!(setup.spawner.spawned.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn unknown_projects_and_missing_harnesses_are_refused_before_spawning() {
        let setup = setup(Path::new("/nonexistent/omp"), "omp_basic.jsonl");
        let stranger = setup.project.parent().unwrap().join("stranger");
        std::fs::create_dir_all(&stranger).unwrap();

        let (channel, _) = collecting_channel();
        let forbidden = block_on(start(&setup.state, request(&stranger), channel)).expect_err("unknown project");
        assert_eq!(forbidden.code(), IpcErrorCode::Forbidden);

        cache_detection(&setup.state, HarnessId::Omp, None);
        let (channel, _) = collecting_channel();
        let missing = block_on(start(&setup.state, request(&setup.project), channel)).expect_err("not installed");
        assert_eq!(missing.code(), IpcErrorCode::NotFound);

        assert_eq!(setup.spawner.spawned.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn no_session_starts_once_shutdown_began() {
        let setup = setup(Path::new("/nonexistent/omp"), "omp_basic.jsonl");
        assert!(setup.state.agents.begin_shutdown());
        assert!(!setup.state.agents.begin_shutdown());

        let (channel, _) = collecting_channel();
        let error = block_on(start(&setup.state, request(&setup.project), channel)).expect_err("shutting down");

        assert_eq!(error.code(), IpcErrorCode::Unsupported);
        assert_eq!(setup.spawner.spawned.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn bash_needs_the_users_confirmation() {
        let setup = setup(Path::new("/nonexistent/omp"), "omp_basic.jsonl");

        let unconfirmed = block_on(bash(&setup.state, "key-1", "ls", false)).expect_err("unconfirmed");
        let unknown = block_on(bash(&setup.state, "key-1", "ls", true)).expect_err("no such session");

        assert_eq!(unconfirmed.code(), IpcErrorCode::InvalidInput);
        assert_eq!(unknown.code(), IpcErrorCode::NotFound);
    }
}
