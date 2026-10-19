//! App lifecycle hooks that end agent sessions: a page (re)load of the main webview stops every
//! session (their channels died with the old page) and forgets their hook state, except the chats
//! of routine runs going on (their windows are detached), and app exit stops them before the
//! process ends.
//!
//! A reload takes the sessions out of the manager at once and stops their agents in the
//! background. Their session ids stay held until each agent exited, so the reloaded page resuming
//! its chats waits for the old agents instead of running a second agent on the same session.
//!
//! Exit is held back (`prevent_exit`) while a separate thread waits for
//! [`Agents::stop_all`](crate::agents::Agents::stop_all) and then for the chats' `sessionEnd`
//! hook commands (at most [`SESSION_END_BUDGET`]), all within [`EXIT_BUDGET`]; the event loop
//! keeps running meanwhile. Then the app exits for real. Shutdown (and `RunEvent::Exit`) also ends
//! the memory heartbeat and removes its file ([`ScriptRequests::stop`](crate::memory_requests::ScriptRequests::stop)),
//! and stops the live routine runs ([`RoutineRuntime::shutdown`](crate::routines::RoutineRuntime::shutdown)).
//!
//! Closing the main window quits, unless "Keep running in the tray when the window is closed" is
//! on and the tray icon exists: the window is hidden instead and routines keep running
//! ([`crate::window::hide_instead_of_closing`]).

use std::sync::mpsc;
use std::time::{Duration, Instant};

use harness_business::agent::STOP_GRACE;
use tauri::webview::{PageLoadEvent, PageLoadPayload};
use tauri::{AppHandle, Manager, RunEvent, Runtime, Webview, WindowEvent};

use crate::hooks::SESSION_END_BUDGET;
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
        page_reloaded(&state);
    }
}

/// The page reloaded: every chat stops but those of routine runs going on, which no page owns
/// (only their windows leave).
pub fn page_reloaded(state: &AppState) {
    let routine_chats = state.routines.relays().running_keys();
    state.routines.relays().detach_all();

    state.agents.stop_all_in_background_except(&routine_chats);
    state.hooks.clear_except(&routine_chats);
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
            event: WindowEvent::CloseRequested { api, .. },
            ..
        } if label == MAIN_WINDOW => {
            if crate::window::hide_instead_of_closing(app) {
                api.prevent_close();
            }
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

/// Stops every session on a helper thread (bounded by [`EXIT_BUDGET`]), then exits with `code`.
/// Later calls do nothing while the first shutdown runs.
fn begin_shutdown<R: Runtime>(app: &AppHandle<R>, code: i32) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };

    if !state.agents.begin_shutdown() {
        return;
    }

    // No agent can ask to run a script any more: the server stops waiting for this app.
    state.script_requests.stop();

    let agents = state.agents.clone();
    let hooks = state.hooks.clone();
    let routines = state.routines.clone();
    let handle = app.clone();
    let deadline = Instant::now() + EXIT_BUDGET;

    let waiter = std::thread::Builder::new()
        .name("agent-shutdown".to_owned())
        .spawn(move || {
            // Live routine runs end as "stopped" ("Harness Desktop closed during the run").
            routines.shutdown();

            let (done, stopped) = mpsc::channel();
            let stopping = agents.clone();

            tauri::async_runtime::spawn(async move {
                stopping.stop_all().await;

                // The exits reach the chats' hook tasks, which run their sessionEnd commands.
                let left = deadline.saturating_duration_since(Instant::now());
                hooks.finish_chats(left.min(SESSION_END_BUDGET)).await;

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
