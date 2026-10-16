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

/// What the app does when an update is available.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum UpdatePolicy {
    /// Check at startup and ask before installing.
    #[default]
    StartupAsk,
    /// Check at startup and install automatically.
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

/// UI language codes the app ships translations for.
pub const LANGUAGES: [&str; 5] = ["en", "de", "es", "fr", "pl"];

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
    /// UI language code, one of `LANGUAGES` (`en`, `de`, `es`, `fr`, `pl`); default `en`.
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
    /// Whether the app keeps the active harness's plugins, marketplace catalogs and packages up to date by
    /// itself, announcing each batch in a toast. Independent of `update_policy`, which is about the app.
    pub auto_update_extensions: bool,
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
    /// Whether the app registers its bundled `harness-memory` MCP server in every detected harness's
    /// user MCP file at launch (default on). Server-managed: only `memory_set_enabled` changes it,
    /// since turning it off also removes the app's entries.
    pub memory_enabled: bool,
    /// Whether a chat message is sent with the project-memory notes found for it in front, as a
    /// `<harness-memory-recall>` block (default on; files written before it existed turn it on).
    /// Only applies while `memory_enabled` is on. UI-managed.
    pub memory_recall: bool,
}

impl AppPrefs {
    /// Current schema version written by this build.
    pub const VERSION: u32 = 1;
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
            update_policy: UpdatePolicy::StartupAsk,
            check_companion_updates: true,
            auto_update_extensions: true,
            open_any_link: false,
            ai_chat_names: false,
            onboarding_completed: false,
            approval_notice_seen: false,
            binary_overrides: Vec::new(),
            chat_defaults: Vec::new(),
            memory_enabled: true,
            memory_recall: true,
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
