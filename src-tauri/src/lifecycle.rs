//! App lifecycle hooks that end agent sessions: a page (re)load of the main webview stops every
//! session (their channels died with the old page) and forgets their memory recalls (stopping
//! their warm servers), and app exit stops them and their warm servers before the process ends.
//!
//! A reload takes the sessions out of the manager at once and stops their agents in the
//! background. Their session ids stay held until each agent exited, so the reloaded page resuming
//! its chats waits for the old agents instead of running a second agent on the same session.
//!
//! Exit is held back (`prevent_exit`) while a separate thread waits for
//! [`Agents::stop_all`](crate::agents::Agents::stop_all), at most [`EXIT_BUDGET`]; the event loop
//! keeps running meanwhile. Then the app exits for real. Shutdown (and `RunEvent::Exit`) also ends
//! the memory heartbeat and removes its file ([`ScriptRequests::stop`](crate::memory_requests::ScriptRequests::stop)).

use std::sync::mpsc;
use std::time::Duration;

use harness_business::agent::STOP_GRACE;
use tauri::webview::{PageLoadEvent, PageLoadPayload};
use tauri::{AppHandle, Manager, RunEvent, Runtime, Webview, WindowEvent};

use crate::state::AppState;
use crate::window::MAIN_WINDOW;

/// Longest wait for sessions at exit. Sessions stop concurrently: stdin is closed, agents get
/// [`STOP_GRACE`] (3 s) to exit, then their process groups are killed; the extra second covers
/// the kill and the delivery of the last events.
pub const EXIT_BUDGET: Duration = STOP_GRACE.saturating_add(Duration::from_secs(1));

/// `Builder::on_page_load` hook.
pub fn on_page_load<R: Runtime>(webview: &Webview<R>, payload: &PageLoadPayload<'_>) {
    if webview.label() != MAIN_WINDOW || payload.event() != PageLoadEvent::Started {
        return;
    }

    if let Some(state) = webview.try_state::<AppState>() {
        state.agents.stop_all_in_background();
        state.recall.clear();
    }
}

/// `App::run` callback.
pub fn on_run_event<R: Runtime>(app: &AppHandle<R>, event: RunEvent) {
    match event {
        RunEvent::ExitRequested { code, api, .. } => {
            let Some(state) = app.try_state::<AppState>() else {
                return;
            };

            if state.agents.is_stopped() {
                return;
            }

            api.prevent_exit();
            begin_shutdown(app, code.unwrap_or(0));
        }
        RunEvent::WindowEvent {
            label,
            event: WindowEvent::Destroyed,
            ..
        } if label == MAIN_WINDOW => begin_shutdown(app, 0),
        RunEvent::Exit => {
            if let Some(state) = app.try_state::<AppState>() {
                state.script_requests.stop();
            }
        }
        _ => {}
    }
}

/// Stops the warm memory servers at once and every session on a helper thread (bounded by
/// [`EXIT_BUDGET`]), then exits with `code`. Later calls do nothing while the first shutdown runs.
fn begin_shutdown<R: Runtime>(app: &AppHandle<R>, code: i32) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };

    if !state.agents.begin_shutdown() {
        return;
    }

    state.recall.clear();
    // No agent can ask to run a script any more: the server stops waiting for this app.
    state.script_requests.stop();

    let agents = state.agents.clone();
    let handle = app.clone();

    let waiter = std::thread::Builder::new()
        .name("agent-shutdown".to_owned())
        .spawn(move || {
            let (done, stopped) = mpsc::channel();
            let stopping = agents.clone();

            tauri::async_runtime::spawn(async move {
                stopping.stop_all().await;
                done.send(()).ok();
            });

            if stopped.recv_timeout(EXIT_BUDGET).is_err() {
                tracing::warn!(
                    budget_ms = EXIT_BUDGET.as_millis(),
                    "agent sessions did not stop in time"
                );
            }

            agents.finish_shutdown();
            handle.exit(code);
        });

    if let Err(error) = waiter {
        tracing::error!(%error, "cannot start the shutdown thread; exiting without waiting for agents");
        state.agents.finish_shutdown();
        app.exit(code);
    }
}
