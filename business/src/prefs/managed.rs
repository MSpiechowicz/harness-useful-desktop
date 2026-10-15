//! Server-managed preferences.
//!
//! The webview edits only display and workflow choices (theme, motion, onboarding flags, ...).
//! The active project, the recent projects and the binary overrides decide which directories the
//! app reads and which executables it runs, so only Rust changes them: through the native pickers
//! and the dedicated project/harness commands, never through `prefs_set`.

use std::path::Path;

use super::PrefsStore;
use crate::env::OsKind;
use crate::error::{BusinessError, Result};
use crate::harness::override_name_matches;
use crate::model::{AppPrefs, BinaryOverride, HarnessId};

impl PrefsStore {
    /// Loads the stored prefs, applies `change`, then normalizes and saves them; returns what was
    /// stored. Callers serialize access to the store, so the load-modify-save cycle is atomic.
    pub fn update(&self, change: impl FnOnce(&mut AppPrefs)) -> Result<AppPrefs> {
        let mut prefs = self.load();
        change(&mut prefs);

        self.save(&prefs)
    }

    /// Stores the UI-managed fields of `ui` and keeps every server-managed field as stored
    /// (see [`with_ui_fields`]). Turning `rememberRecentProjects` off clears the recent projects.
    pub fn save_ui(&self, ui: &AppPrefs) -> Result<AppPrefs> {
        self.update(|stored| *stored = with_ui_fields(stored.clone(), ui))
    }

    /// Makes `path` the binary of `harness` (replacing any earlier override).
    ///
    /// The path must be absolute, name an existing file and have the harness binary's file name
    /// (`binary_name`; on Windows also with `.exe`/`.cmd`/`.bat`, case-insensitive), otherwise
    /// `InvalidInput`.
    pub fn set_binary_override(
        &self,
        os: OsKind,
        harness: HarnessId,
        binary_name: &str,
        path: &Path,
    ) -> Result<AppPrefs> {
        let path = validate_override(os, binary_name, path)?;

        self.update(|prefs| {
            prefs.binary_overrides.retain(|entry| entry.harness != harness);
            prefs.binary_overrides.push(BinaryOverride { harness, path });
        })
    }

    /// Removes the binary override of `harness`, if any.
    pub fn clear_binary_override(&self, harness: HarnessId) -> Result<AppPrefs> {
        self.update(|prefs| prefs.binary_overrides.retain(|entry| entry.harness != harness))
    }
}

/// `stored` with every UI-managed field taken from `ui`.
///
/// The destructuring is exhaustive on purpose: a new [`AppPrefs`] field does not compile until it
/// is classified here as UI-managed or server-managed.
pub fn with_ui_fields(stored: AppPrefs, ui: &AppPrefs) -> AppPrefs {
    let AppPrefs {
        version: _,
        theme,
        motion,
        language,
        intro_enabled,
        sound_enabled,
        show_thinking,
        show_message_usage,
        default_harness,
        remember_recent_projects,
        update_policy,
        check_companion_updates,
        auto_update_extensions,
        open_any_link,
        ai_chat_names,
        onboarding_completed,
        approval_notice_seen,
        chat_defaults,
        // Server-managed: kept from `stored`.
        active_project: _,
        recent_projects: _,
        binary_overrides: _,
    } = ui.clone();

    AppPrefs {
        theme,
        motion,
        language,
        intro_enabled,
        sound_enabled,
        show_thinking,
        show_message_usage,
        default_harness,
        remember_recent_projects,
        update_policy,
        check_companion_updates,
        auto_update_extensions,
        open_any_link,
        ai_chat_names,
        onboarding_completed,
        approval_notice_seen,
        chat_defaults,
        ..stored
    }
}

fn validate_override(os: OsKind, binary_name: &str, path: &Path) -> Result<String> {
    if !override_name_matches(os, binary_name, path) {
        return Err(BusinessError::InvalidInput(format!(
            "the selected file must be named `{binary_name}`"
        )));
    }

    if !path.is_absolute() || !path.is_file() {
        return Err(BusinessError::InvalidInput(
            "the selected binary is not an existing file".to_owned(),
        ));
    }

    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| BusinessError::InvalidInput("the binary path is not valid UTF-8".to_owned()))
}
