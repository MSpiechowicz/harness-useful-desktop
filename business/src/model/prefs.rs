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
    /// Whether onboarding was completed.
    pub onboarding_completed: bool,
    /// Whether the one-time approval-mode notice was dismissed.
    pub approval_notice_seen: bool,
    /// Per-harness executable overrides (at most one per harness).
    pub binary_overrides: Vec<BinaryOverride>,
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
            default_harness: None,
            active_project: None,
            remember_recent_projects: true,
            recent_projects: Vec::new(),
            update_policy: UpdatePolicy::StartupAsk,
            onboarding_completed: false,
            approval_notice_seen: false,
            binary_overrides: Vec::new(),
        }
    }
}
