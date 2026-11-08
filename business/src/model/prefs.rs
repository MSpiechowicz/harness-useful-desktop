use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::HarnessId;

/// Colour theme preference.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ThemePref {
    /// Follow the operating system.
    #[default]
    System,
    /// Always dark.
    Dark,
    /// Always light.
    Light,
}

/// Animation preference.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MotionPref {
    /// All animations (still honours the OS reduced-motion setting).
    #[default]
    Full,
    /// Short fades only.
    Reduced,
    /// No animation.
    Off,
}

/// Which way the plan limit bars fill.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LimitBarsPref {
    /// Show how much of a limit is used.
    #[default]
    Used,
    /// Show how much of a limit is left.
    Left,
}

/// How the plan limits are drawn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LimitGaugePref {
    /// Horizontal bars.
    #[default]
    Bars,
    /// Tachometer dials.
    Tach,
    /// Segmented bars.
    Segments,
    /// Level meters.
    Vu,
    /// Thermometers.
    Thermo,
    /// Waffle grids.
    Waffle,
    /// Battery cells.
    Battery,
}

/// What the app does when an update is available.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum UpdatePolicy {
    /// Check at startup and ask before installing.
    StartupAsk,
    /// Check at startup and install automatically.
    #[default]
    AutoInstall,
    /// Never check automatically.
    Off,
}

/// A user-chosen executable for a harness, overriding PATH discovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BinaryOverride {
    /// The harness the override applies to.
    pub harness: HarnessId,
    /// Absolute path of the executable.
    pub path: String,
}

/// The model and thinking level a harness's new chats start with: the last ones chosen in the app.
/// Kept by Harness Desktop only; the harness's own configuration is never changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChatDefaults {
    /// The harness these defaults apply to.
    pub harness: HarnessId,
    /// Provider of the model, as the agent reports it.
    pub provider: Option<String>,
    /// Model id, as the agent reports it.
    pub model_id: Option<String>,
    /// Thinking level, one of the agent's `getAvailableThinkingLevels`.
    pub thinking_level: Option<String>,
}

/// The main branch the user picked for one project (see `PrefsStore::set_main_branch`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MainBranchChoice {
    /// Absolute path of the project directory.
    pub project: String,
    /// Short name of the branch (`main`).
    pub branch: String,
}

/// UI language codes the app ships translations for.
pub const LANGUAGES: [&str; 6] = ["en", "de", "es", "fr", "pl", "pt"];

/// UI language used when none is stored or the stored one is not in [`LANGUAGES`].
pub const DEFAULT_LANGUAGE: &str = "en";

/// Persisted user preferences. Holds no secrets and no chat content.
///
/// `#[serde(default)]` lets older or partial files load; unknown fields are ignored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct AppPrefs {
    /// Schema version of the file (currently 1).
    pub version: u32,
    /// Colour theme.
    pub theme: ThemePref,
    /// Animation level.
    pub motion: MotionPref,
    /// UI language code, one of `LANGUAGES` (`en`, `de`, `es`, `fr`, `pl`, `pt` for Brazilian
    /// Portuguese); default `en`.
    /// Any other stored value (an unknown code, or not a string at all) loads as `en` instead of
    /// making the file unusable.
    #[serde(with = "lenient_language")]
    #[ts(as = "String")]
    pub language: String,
    /// Whether the opening animation plays.
    pub intro_enabled: bool,
    /// Whether the opening animation plays its sound.
    pub sound_enabled: bool,
    /// Whether chats and transcripts show the agent's thought process.
    pub show_thinking: bool,
    /// Whether each reply ends with its model, token count and cost.
    pub show_message_usage: bool,
    /// The harness opened by default ("daily driver").
    pub default_harness: Option<HarnessId>,
    /// Absolute path of the active project directory.
    pub active_project: Option<String>,
    /// Whether recently opened projects are remembered.
    pub remember_recent_projects: bool,
    /// Absolute project paths, most recent first (max 12).
    pub recent_projects: Vec<String>,
    /// Update behaviour.
    pub update_policy: UpdatePolicy,
    /// Whether the Ecosystem view looks up the latest GitHub release of the companion tools.
    /// Independent of `update_policy`; when off, `ecosystem_releases` makes no request.
    pub check_companion_updates: bool,
    /// Whether the app looks up the latest npm version of the harness CLIs and offers their updates.
    /// Independent of `check_companion_updates`; when off, `harness_releases` makes no request. A
    /// stored file without this field takes the value of `check_companion_updates`, which also
    /// covered this lookup before (see `PrefsStore::load_checked`).
    pub check_harness_updates: bool,
    /// Whether the app keeps the active harness's plugins, marketplace catalogs and packages up to date by
    /// itself, announcing each batch in a toast. Independent of `update_policy`, which is about the app.
    pub auto_update_extensions: bool,
    /// Refresh the extension catalogs (marketplace lists) in the background. Installs are governed by
    /// `auto_update_extensions`. A stored file without this field takes the value of
    /// `auto_update_extensions` (see `PrefsStore::load_checked`).
    pub auto_refresh_catalogs: bool,
    /// Whether any `https` link (a pull request the agent opened, docs it cites) opens in the browser.
    /// Off: only project and harness sites do.
    pub open_any_link: bool,
    /// Whether a new chat asks its own agent for a short name and description after the first message
    /// (one small request per chat, on the chat's model). Off: the name comes from the first message.
    pub ai_chat_names: bool,
    /// Whether onboarding was completed.
    pub onboarding_completed: bool,
    /// Whether the one-time approval-mode notice was dismissed.
    pub approval_notice_seen: bool,
    /// Per-harness executable overrides (at most one per harness).
    pub binary_overrides: Vec<BinaryOverride>,
    /// The model and thinking level new chats start with (at most one entry per harness).
    pub chat_defaults: Vec<ChatDefaults>,
    /// Whether the app registers the installed `harness-memory` MCP server (Harness Memory) in every
    /// detected harness's user MCP file at launch and after its install (default on). Server-managed:
    /// only `memory_set_enabled` changes it, since turning it off also removes the app's entries.
    pub memory_enabled: bool,
    /// Whether Harness Memory searches by meaning as well as by keywords, through the local embedder
    /// the app keeps running (default on, also for a file without this key). Server-managed: only
    /// `memory_set_vector_search` changes it, since it also starts or stops the embedder.
    pub vector_search: bool,
    /// Whether the app registers the installed `harness-knowledge` MCP server (Harness Knowledge) in
    /// every detected harness's user MCP file at launch and after its install (default on).
    /// Server-managed: only `knowledge_set_enabled` changes it, since turning it off also removes
    /// the app's entries.
    pub knowledge_enabled: bool,
    /// Whether Harness Knowledge searches by meaning as well as by keywords, through its own local
    /// embedder the app keeps running (default on, also for a file without this key).
    /// Server-managed: only `knowledge_set_vector_search` changes it, since it also starts or stops
    /// the embedder.
    pub knowledge_vector_search: bool,
    /// Whether the app registers the installed `harness-code` MCP server (Harness Code) in every
    /// detected harness's user MCP file at launch and after its install (default on). Server-managed:
    /// only `code_set_enabled` changes it, since turning it off also removes the app's entries.
    pub code_enabled: bool,
    /// Whether the background updater already installed Harness Code once for a user who has Harness
    /// Memory without it (default off). Set before that install starts, so a failed or later removed
    /// Harness Code is never installed again on its own, and by any install of Harness Code from the
    /// app. Server-managed: only `code_auto_install_claim` and `setup_install_code` change it.
    pub code_auto_install_done: bool,
    /// Whether the one-time notice to restart running harnesses was raised, when this computer first
    /// had a Harness Knowledge with drafts installed (default off). A harness that started before
    /// holds the older server, whether or not this computer upgraded the database. Server-managed:
    /// only the install of Harness Knowledge changes it.
    #[serde(default)]
    pub knowledge_restart_notice_done: bool,
    /// Whether the one-time switch of every update setting to automatic already ran (see
    /// [`PrefsStore::turn_updates_on_once`](crate::prefs::PrefsStore::turn_updates_on_once)). A file
    /// without this key reads `false`, so every existing install gets it once; fresh installs start with
    /// the automatic defaults and `true`. Server-managed: only that migration changes it.
    #[serde(default)]
    pub updates_turned_on: bool,
    /// Whether the one-time notice that agents save notes and scripts only where the user turned
    /// saving on was handled. A file without this key reads `false`, so every existing install sees
    /// the notice once; fresh installs start with `true` and never see it, since they begin with
    /// saving off, unless Harness Memory already has a database on the computer (someone who used it
    /// before this app, or lost the prefs file): then the first load starts with `false`. Prefs that
    /// could not be read load `false`, so the notice is not lost. Server-managed: only
    /// `memory_opt_in_notice_done` changes it.
    #[serde(default)]
    pub memory_opt_in_notice_done: bool,
    /// Whether closing the main window hides it to the tray instead of quitting, so routines keep
    /// running (default off). Without a tray icon closing always quits.
    pub keep_running_in_tray: bool,
    /// Whether a new chat starts with "Ask before saving" on for notes (default off).
    pub memory_notes_ask_default: bool,
    /// Whether a new chat starts with "Ask before saving" on for scripts (default off).
    pub memory_scripts_ask_default: bool,
    /// Whether "Remote + this computer" may copy its entries to the team database on request
    /// (the Sync view's `toRemote`, default off). The commands refuse (`forbidden`) while off.
    pub memory_copy_to_remote: bool,
    /// Whether "Remote + this computer" may copy the team's entries to this computer on request
    /// (the Sync view's `toLocal`, default off). The commands refuse (`forbidden`) while off.
    pub memory_copy_to_local: bool,
    /// Whether the app fetches the open project's remote in the background (every few minutes while
    /// the window is visible, default on). Off: git only reaches the network when a button is clicked.
    pub background_fetch: bool,
    /// Whether the plan limit bars show what is used or what is left (default used).
    /// Any other stored value loads as the default instead of making the file unusable.
    #[serde(with = "lenient")]
    #[ts(as = "LimitBarsPref")]
    pub limit_bars: LimitBarsPref,
    /// Whether the plan limits show the forecast of when a limit runs out (default on). Only a
    /// stored `false` hides it: any other value loads as on.
    #[serde(with = "lenient")]
    #[ts(as = "bool")]
    pub limit_forecast: bool,
    /// How the plan limits are drawn (default bars).
    /// Any other stored value loads as the default instead of making the file unusable.
    #[serde(with = "lenient")]
    #[ts(as = "LimitGaugePref")]
    pub limit_gauge: LimitGaugePref,
    /// The main branch picked per project (at most one entry per project). Server-managed: only
    /// `git_set_main_branch` changes it.
    pub main_branches: Vec<MainBranchChoice>,
}

impl AppPrefs {
    /// Current schema version written by this build.
    pub const VERSION: u32 = 1;

    /// The main branch picked for `project`, if any.
    pub fn picked_main_branch(&self, project: &str) -> Option<&str> {
        self.main_branches
            .iter()
            .find(|choice| choice.project == project)
            .map(|choice| choice.branch.as_str())
    }
}

impl Default for AppPrefs {
    fn default() -> Self {
        Self {
            version: Self::VERSION,
            theme: ThemePref::System,
            motion: MotionPref::Full,
            language: DEFAULT_LANGUAGE.to_owned(),
            intro_enabled: true,
            sound_enabled: false,
            show_thinking: true,
            show_message_usage: true,
            default_harness: None,
            active_project: None,
            remember_recent_projects: true,
            recent_projects: Vec::new(),
            update_policy: UpdatePolicy::AutoInstall,
            check_companion_updates: true,
            check_harness_updates: true,
            auto_update_extensions: true,
            auto_refresh_catalogs: true,
            open_any_link: false,
            ai_chat_names: false,
            onboarding_completed: false,
            approval_notice_seen: false,
            binary_overrides: Vec::new(),
            chat_defaults: Vec::new(),
            memory_enabled: true,
            vector_search: true,
            knowledge_enabled: true,
            knowledge_vector_search: true,
            code_enabled: true,
            code_auto_install_done: false,
            knowledge_restart_notice_done: false,
            updates_turned_on: true,
            memory_opt_in_notice_done: true,
            keep_running_in_tray: false,
            memory_notes_ask_default: false,
            memory_scripts_ask_default: false,
            memory_copy_to_remote: false,
            memory_copy_to_local: false,
            background_fetch: true,
            limit_bars: LimitBarsPref::Used,
            limit_forecast: true,
            limit_gauge: LimitGaugePref::Bars,
            main_branches: Vec::new(),
        }
    }
}

/// `language` as stored: written as the string it is; read from any JSON value, keeping a string as
/// is (the store whitelists it) and turning anything else into [`DEFAULT_LANGUAGE`], so one
/// hand-edited value cannot make the whole file unusable.
mod lenient_language {
    use serde::{Deserialize, Deserializer, Serializer};
    use serde_json::Value;

    use super::DEFAULT_LANGUAGE;

    pub fn serialize<S: Serializer>(language: &str, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(language)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
        match Value::deserialize(deserializer)? {
            Value::String(language) => Ok(language),
            _ => Ok(DEFAULT_LANGUAGE.to_owned()),
        }
    }
}

/// What a display choice reads as when its stored value does not parse.
trait Fallback {
    fn fallback() -> Self;
}

impl Fallback for LimitBarsPref {
    fn fallback() -> Self {
        Self::default()
    }
}

impl Fallback for LimitGaugePref {
    fn fallback() -> Self {
        Self::default()
    }
}

/// The forecast is on unless a stored `false` says otherwise.
impl Fallback for bool {
    fn fallback() -> Self {
        true
    }
}

/// Display choices as stored: written as they are; read from any JSON value, taking the
/// [`Fallback`] for one that does not parse, so one hand-edited value cannot make the whole file
/// unusable. (A missing key takes the struct default.)
mod lenient {
    use serde::de::DeserializeOwned;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use serde_json::Value;

    use super::Fallback;

    pub fn serialize<T: Serialize, S: Serializer>(choice: &T, serializer: S) -> Result<S::Ok, S::Error> {
        choice.serialize(serializer)
    }

    pub fn deserialize<'de, T: DeserializeOwned + Fallback, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<T, D::Error> {
        let value = Value::deserialize(deserializer)?;

        Ok(serde_json::from_value(value).unwrap_or_else(|_| T::fallback()))
    }
}
