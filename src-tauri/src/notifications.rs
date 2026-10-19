//! Tells the user about a new script run request while the main window is not in front: an OS
//! notification per request (`tauri-plugin-notification`, driven from Rust only) and a request
//! for window attention (taskbar flash, dock bounce, urgency hint). A click on a notification
//! brings the main window to the front, where the request dialog waits.
//!
//! A notification carries only the script name, never its code. Failures are logged at debug:
//! the in-app dialog still shows the request. Routines show theirs through the same helpers
//! ([`main_window_in_front`], [`show`], see `crate::routines::notify`).

use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Manager, Runtime, UserAttentionType};
use tauri_plugin_notification::NotificationExt;

use crate::native_text::{NativeText, native_text};
use crate::window::{MAIN_WINDOW, focus_main_window};

/// The plugin. Its JS commands are not granted to the webview (see `capabilities/main.json`).
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri_plugin_notification::init()
}

/// Brings the main window to the front when the user clicks a notification, and for a "Routine
/// needs you" one opens the chat of the run it names while that run still waits (see
/// `crate::routines::notify`). Call once, after the plugin is set up; notifications shown before
/// this report no clicks.
pub fn focus_on_click<R: Runtime>(app: &AppHandle<R>) {
    let handle = app.clone();

    let registered = app.notification().on_action(move |performed| {
        if performed.action_id() == "tap" {
            focus_main_window(&handle);

            let extra = performed
                .notification()
                .map(|notification| notification.extra().clone())
                .unwrap_or_default();

            if let Some(state) = handle.try_state::<crate::state::AppState>() {
                state.routines.on_notification_click(&extra);
            }
        }
    });

    if let Err(error) = registered {
        tracing::debug!(%error, "notification clicks are not reported");
    }
}

/// How the main window looks to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WindowView {
    visible: bool,
    focused: bool,
    minimized: bool,
}

/// Whether `new_requests` requests that just arrived need a notification: some did, and the main
/// window (`None` when it does not exist) is not shown, focused and unminimized.
fn should_alert(new_requests: usize, window: Option<WindowView>) -> bool {
    if new_requests == 0 {
        return false;
    }

    let in_front = window.is_some_and(|view| view.visible && view.focused && !view.minimized);

    !in_front
}

/// Notifies about the requests that just arrived (their script names) unless the main window is
/// in front. Requests sent again to a new listener are not passed here.
pub fn alert<R: Runtime>(app: &AppHandle<R>, script_names: &[String]) {
    if !should_alert(script_names.len(), window_view(app)) {
        return;
    }

    request_attention(app);

    let text = native_text(&crate::tray::saved_language(app));

    for name in script_names {
        show(app, text.script_request_title, &body(name, text));
    }
}

/// True while the main window is shown, focused and not minimized: the in-app UI tells the user
/// instead of an OS notification.
pub fn main_window_in_front<R: Runtime>(app: &AppHandle<R>) -> bool {
    !should_alert(1, window_view(app))
}

fn window_view<R: Runtime>(app: &AppHandle<R>) -> Option<WindowView> {
    let window = app.get_webview_window(MAIN_WINDOW)?;

    Some(WindowView {
        visible: window.is_visible().unwrap_or(false),
        focused: window.is_focused().unwrap_or(false),
        minimized: window.is_minimized().unwrap_or(false),
    })
}

/// Asks for attention to the main window (taskbar flash, dock bounce, urgency hint).
pub fn request_attention<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW)
        && let Err(error) = window.request_user_attention(Some(UserAttentionType::Informational))
    {
        tracing::debug!(%error, "cannot request attention for the main window");
    }
}

/// Shows one OS notification; a failure is logged at debug.
pub fn show<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) {
    show_with_extra(app, title, body, &[]);
}

/// Shows one OS notification that carries `extra` data, which a click reports back (see
/// [`focus_on_click`]); a failure is logged at debug.
pub fn show_with_extra<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str, extra: &[(&str, String)]) {
    let mut builder = app.notification().builder().title(title).body(body);

    for (key, value) in extra {
        builder = builder.extra(*key, value);
    }

    if let Err(error) = builder.show() {
        tracing::debug!(%error, "cannot show a notification");
    }
}

/// "<script name> · Review it in Harness Desktop". A name outside the server's documented form
/// (`^[a-z0-9][a-z0-9_-]{0,63}$`) is left out, so the notification never shows markup or
/// control characters taken from a request file.
fn body(script_name: &str, text: &NativeText) -> String {
    if is_script_name(script_name) {
        format!("{script_name} · {}", text.script_request_review)
    } else {
        text.script_request_review.to_owned()
    }
}

fn is_script_name(name: &str) -> bool {
    let mut chars = name.chars();

    let Some(first) = chars.next() else {
        return false;
    };

    let plain = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit();

    name.len() <= 64 && plain(first) && chars.all(|c| plain(c) || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    const IN_FRONT: WindowView = WindowView {
        visible: true,
        focused: true,
        minimized: false,
    };

    #[test]
    fn new_requests_alert_only_while_the_window_is_not_in_front() {
        assert!(!should_alert(1, Some(IN_FRONT)));

        let unfocused = WindowView {
            focused: false,
            ..IN_FRONT
        };
        let minimized = WindowView {
            minimized: true,
            ..IN_FRONT
        };
        let hidden = WindowView {
            visible: false,
            ..IN_FRONT
        };

        for view in [Some(unfocused), Some(minimized), Some(hidden), None] {
            assert!(should_alert(1, view), "{view:?}");
            assert!(should_alert(3, view), "{view:?}");
        }
    }

    #[test]
    fn no_new_request_never_alerts() {
        for view in [Some(IN_FRONT), None] {
            assert!(!should_alert(0, view), "{view:?}");
        }
    }

    #[test]
    fn the_body_names_the_script_in_its_documented_form_only() {
        let text = native_text("en");

        assert_eq!(body("deploy_db-2", text), "deploy_db-2 · Review it in Harness Desktop");

        for name in [
            "",
            "Deploy",
            "-x",
            "a b",
            "<a href=\"x\">y</a>",
            "x\ny",
            &"a".repeat(65),
        ] {
            assert_eq!(body(name, text), "Review it in Harness Desktop", "{name:?}");
        }
    }
}
