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
    /// Whether the opening animation plays.
    pub intro_enabled: bool,
    /// Whether the opening animation plays its sound.
    pub sound_enabled: bool,
    /// Whether chats and transcripts show the agent's thought process.
    pub show_thinking: bool,
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
    /// Whether onboarding was completed.
    pub onboarding_completed: bool,
    /// Whether the one-time approval-mode notice was dismissed.
    pub approval_notice_seen: bool,
    /// Per-harness executable overrides (at most one per harness).
    pub binary_overrides: Vec<BinaryOverride>,
    /// The model and thinking level new chats start with (at most one entry per harness).
    pub chat_defaults: Vec<ChatDefaults>,
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
            intro_enabled: true,
            sound_enabled: false,
            show_thinking: true,
            default_harness: None,
            active_project: None,
            remember_recent_projects: true,
            recent_projects: Vec::new(),
            update_policy: UpdatePolicy::StartupAsk,
            check_companion_updates: true,
            onboarding_completed: false,
            approval_notice_seen: false,
            binary_overrides: Vec::new(),
            chat_defaults: Vec::new(),
        }
    }
}
