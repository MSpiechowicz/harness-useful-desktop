//! Server-managed preferences.
//!
//! The webview edits only display and workflow choices (theme, motion, onboarding flags, ...).
//! The active project, the recent projects and the binary overrides decide which directories the
//! app reads and which executables it runs, so only Rust changes them: through the native pickers
//! and the dedicated project/harness commands, never through `prefs_set`. Project memory
//! (`memory_enabled`) writes to the harnesses' MCP files when switched, and vector search (`vector_search`) starts or
//! stops the embedder, so each has its own command too, as have Project knowledge (`knowledge_enabled`)
//! and its vector search (`knowledge_vector_search`), and the code graph (`code_enabled`). The main branch picked per project
//! (`main_branches`) is written by `git_set_main_branch` only, after validating the name.

use std::path::Path;

use super::{PrefsOrigin, PrefsStore};
use crate::env::OsKind;
use crate::error::{BusinessError, Result};
use crate::git::validate_branch_name;
use crate::harness::override_name_matches;
use crate::model::{AppPrefs, BinaryOverride, HarnessId, MainBranchChoice, UpdatePolicy};

impl PrefsStore {
    /// Loads the stored prefs, applies `change`, then normalizes and saves them; returns what was
    /// stored. Callers serialize access to the store, so the load-modify-save cycle is atomic.
    pub fn update(&self, change: impl FnOnce(&mut AppPrefs)) -> Result<AppPrefs> {
        let mut prefs = self.load();
        change(&mut prefs);

        self.save(&prefs)
    }

    /// Turns every update setting to automatic once per install: the app updates itself
    /// (`updatePolicy`), checks the Harness family, updates extensions and the Harness family, and
    /// refreshes catalogs. Runs at launch. A file that already had it (`updatesTurnedOn`) keeps whatever
    /// the user chose since. Prefs that could not be read stay as they load (off, failing closed) and
    /// are not marked, so the switch happens once a readable file exists. True when it changed the file.
    pub fn turn_updates_on_once(&self) -> Result<bool> {
        let (mut prefs, origin) = self.load_checked();

        if origin != PrefsOrigin::Stored || prefs.updates_turned_on {
            return Ok(false);
        }

        prefs.update_policy = UpdatePolicy::AutoInstall;
        prefs.check_companion_updates = true;
        prefs.check_harness_updates = true;
        prefs.auto_update_extensions = true;
        prefs.auto_refresh_catalogs = true;
        prefs.updates_turned_on = true;

        self.save(&prefs)?;

        Ok(true)
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

    /// Makes `branch` the main branch of `project` (replacing any earlier pick), or clears the pick
    /// with `None`.
    ///
    /// `project` must be an absolute path and `branch` a valid branch name, otherwise
    /// `InvalidInput`.
    pub fn set_main_branch(&self, project: &Path, branch: Option<&str>) -> Result<AppPrefs> {
        let project = project
            .to_str()
            .filter(|_| project.is_absolute())
            .ok_or_else(|| BusinessError::InvalidInput("the project path must be absolute".to_owned()))?
            .to_owned();

        if let Some(branch) = branch {
            validate_branch_name(branch)?;
        }

        self.update(|prefs| {
            prefs.main_branches.retain(|choice| choice.project != project);

            if let Some(branch) = branch {
                prefs.main_branches.push(MainBranchChoice {
                    project,
                    branch: branch.to_owned(),
                });
            }
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
        check_harness_updates,
        auto_update_extensions,
        auto_refresh_catalogs,
        open_any_link,
        ai_chat_names,
        onboarding_completed,
        approval_notice_seen,
        chat_defaults,
        keep_running_in_tray,
        chat_alerts,
        alert_sound,
        memory_notes_ask_default,
        memory_scripts_ask_default,
        memory_copy_to_remote,
        memory_copy_to_local,
        background_fetch,
        limit_bars,
        limit_forecast,
        limit_gauge,
        // Server-managed: kept from `stored`.
        active_project: _,
        recent_projects: _,
        binary_overrides: _,
        memory_enabled: _,
        vector_search: _,
        knowledge_enabled: _,
        knowledge_vector_search: _,
        code_enabled: _,
        code_auto_install_done: _,
        knowledge_restart_notice_done: _,
        updates_turned_on: _,
        memory_opt_in_notice_done: _,
        main_branches: _,
    } = ui.clone();

    // The groups are listed in another order than in the destructuring above, which is why the two
    // exhaustive lists are not one repeated block.
    AppPrefs {
        // Display.
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
        check_harness_updates,
        auto_update_extensions,
        auto_refresh_catalogs,
        open_any_link,
        // Memory and limits.
        memory_notes_ask_default,
        memory_scripts_ask_default,
        memory_copy_to_remote,
        memory_copy_to_local,
        background_fetch,
        limit_bars,
        limit_forecast,
        limit_gauge,
        // Chat.
        ai_chat_names,
        onboarding_completed,
        approval_notice_seen,
        chat_defaults,
        keep_running_in_tray,
        chat_alerts,
        alert_sound,
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
