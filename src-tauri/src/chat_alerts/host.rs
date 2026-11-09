//! What an alert needs from the app: the prefs, the main window, notifications and the chime. A
//! trait, so the delivery rules are tested without a window or a sound card.

use harness_business::model::AppPrefs;
use tauri::{AppHandle, Manager, Runtime};

use super::KEY_EXTRA;
use crate::commands::sound::play_alert_chime;
use crate::notifications::{main_window_in_front, request_attention, show_with_extra};
use crate::state::AppState;

pub trait Host: Send + Sync {
    /// The saved prefs. Blocks.
    fn prefs(&self) -> AppPrefs;

    /// Whether the main window is shown, focused and not minimized.
    fn in_front(&self) -> bool;

    /// Asks for attention to the main window (taskbar flash, dock bounce, urgency hint).
    fn attention(&self);

    /// Shows an OS notification whose click names chat `key`.
    fn notify(&self, title: &str, body: &str, key: &str);

    /// Plays the chime.
    fn chime(&self);
}

/// The window, prefs and notifications of the running app.
pub struct AppHost<R: Runtime>(AppHandle<R>);

impl<R: Runtime> AppHost<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self(app)
    }
}

impl<R: Runtime> Host for AppHost<R> {
    fn prefs(&self) -> AppPrefs {
        self.0
            .try_state::<AppState>()
            .map(|state| state.prefs.with(|store| store.load()))
            .unwrap_or_default()
    }

    fn in_front(&self) -> bool {
        main_window_in_front(&self.0)
    }

    fn attention(&self) {
        request_attention(&self.0);
    }

    fn notify(&self, title: &str, body: &str, key: &str) {
        show_with_extra(&self.0, title, body, &[(KEY_EXTRA, key.to_owned())]);
    }

    fn chime(&self) {
        play_alert_chime();
    }
}
