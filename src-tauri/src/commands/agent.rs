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
use harness_business::agent::{AgentDeps, EventSink};
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
use crate::hooks::HookRuntime;
use crate::hooks::chat::{self as chat_hooks, HookSink};
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

/// Runs `action` in session `key`; a chat's prompts get the context of its hooks (see
/// `crate::hooks`).
#[tauri::command]
pub async fn agent_send(state: State<'_, AppState>, key: String, action: AgentAction) -> CommandResult<AgentReply> {
    Ok(chat_hooks::send(&state, &key, action).await?)
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
/// returns its updated info. Only harnesses that can move a session support it (OMP `/move`). The
/// chat's hooks follow it to the new project.
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

    let info = manager.move_project(&key, project.clone(), sessions_dir).await?;
    chat_hooks::moved(&state, &key, project).await;

    Ok(info)
}

/// Stops session `key` (close stdin → 3 s → kill); unknown keys are ignored.
#[tauri::command]
pub async fn agent_stop(state: State<'_, AppState>, key: String) -> CommandResult<()> {
    stop(&state, &key).await
}

/// The running sessions, oldest first.
#[tauri::command]
pub async fn agent_list(state: State<'_, AppState>) -> CommandResult<Vec<AgentSessionInfo>> {
    Ok(state.agents.manager().list())
}

pub(crate) async fn start(
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
    let fresh = request.resume.is_none();

    let sink = Arc::new(ChannelSink::new(
        channel,
        request.harness,
        stop_when_closed(agents, &state.hooks),
    ));

    // Utility agents (titles, reviews) never run hooks.
    let (events, hook_events): (Arc<dyn EventSink>, _) = if request.utility {
        (sink.clone(), None)
    } else {
        let (hook_sink, hook_events) = HookSink::new(sink.clone());
        (Arc::new(hook_sink), Some(hook_events))
    };

    let deps = AgentDeps {
        harness,
        env: &state.env,
        spawner: agents.spawner(),
        binary,
    };

    let info = match agents.manager().start(request, deps, events).await {
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

    if let Some(hook_events) = hook_events {
        chat_hooks::track(state, &info, fresh, hook_events).await;
    }

    Ok(info)
}

/// Stops session `key` and forgets its hooks (after the session left the manager, so a chat
/// tracked meanwhile cannot outlive it; `sessionEnd` still fires from its exit).
pub(crate) async fn stop(state: &AppState, key: &str) -> CommandResult<()> {
    let stopping = state.agents.manager().begin_stop(key);
    state.hooks.forget(key);

    if let Some(stopping) = stopping {
        stopping.finish().await;
    }

    Ok(())
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

/// Stops the session in the background once its channel closes, and forgets its hooks. Holds the
/// agents and the hooks weakly: the sink lives inside a session the agents own.
fn stop_when_closed(agents: &Arc<Agents>, hooks: &Arc<HookRuntime>) -> OnClosed {
    let agents = Arc::downgrade(agents);
    let hooks = Arc::downgrade(hooks);

    // The session leaves the manager before its hooks are forgotten (see `hooks::chat::track`).
    Box::new(move |key| {
        if let Some(agents) = agents.upgrade() {
            agents.stop_in_background(key.to_owned());
        }

        if let Some(hooks) = hooks.upgrade() {
            hooks.forget(key);
        }
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use harness_business::model::{HarnessId, IpcErrorCode};
    use serde_json::Value;
    use tauri::async_runtime::block_on;

    use super::*;
    use crate::test_support::{
        agent_setup as setup, cache_detection, collecting_channel, fake_agent, start_request as request, wait_until,
    };

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
            images: Vec::new(),
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
            images: Vec::new(),
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
        assert_eq!(setup.spawner.spawned(), 1);
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

        assert_eq!(setup.spawner.spawned(), 0);
    }

    #[test]
    fn no_session_starts_once_shutdown_began() {
        let setup = setup(Path::new("/nonexistent/omp"), "omp_basic.jsonl");
        assert!(setup.state.agents.begin_shutdown());
        assert!(!setup.state.agents.begin_shutdown());

        let (channel, _) = collecting_channel();
        let error = block_on(start(&setup.state, request(&setup.project), channel)).expect_err("shutting down");

        assert_eq!(error.code(), IpcErrorCode::Unsupported);
        assert_eq!(setup.spawner.spawned(), 0);
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
