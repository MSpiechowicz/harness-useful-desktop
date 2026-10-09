//! IPC data-transfer objects shared with the webview.
//!
//! Every type here derives `serde` (camelCase on the wire) and `ts_rs::TS`; `#[ts(export)]`
//! writes one TypeScript file per type into `ui/src/lib/ipc/bindings` when
//! `cargo test -p harness_business export_bindings` runs. The generated files are never edited
//! by hand.

mod app;
mod command;
mod config;
mod config_value;
mod extensions;
mod harness;
mod ipc_error;
mod prefs;
mod project;

pub use app::AppInfo;
pub use command::{AgentActionKind, CommandInvocation, CommandSource, CommandSpec, GuiTarget};
pub use config::{ConfigFormat, ConfigLayer, ConfigSection, ConfigSource, ConfigView, Notice, NoticeLevel};
pub use config_value::{ConfigField, ConfigValue, SecretKind};
pub use extensions::{
    ExtensionModuleInfo, ExtensionScope, ExtensionsView, MarketplaceEntry, MarketplaceInfo, McpServerInfo,
    McpTransport, PackageInfo, PluginFeature, PluginInfo, PromptTemplateInfo, SkillInfo, ThemeInfo,
};
pub use harness::{Capabilities, Detection, HarnessId, HarnessInfo, HarnessSummary, InstallHint, VersionSupport};
pub use ipc_error::{IpcError, IpcErrorCode};
pub use prefs::{AppPrefs, BinaryOverride, MotionPref, ThemePref, UpdatePolicy};
pub use project::ProjectInfo;
