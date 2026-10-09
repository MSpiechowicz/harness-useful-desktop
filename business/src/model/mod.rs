//! IPC data-transfer objects shared with the webview.
//!
//! Every type here derives `serde` (camelCase on the wire) and `ts_rs::TS`; `#[ts(export)]`
//! writes one TypeScript file per type into `ui/src/lib/ipc/bindings` when
//! `cargo test -p harness_business export_bindings` runs. The generated files are never edited
//! by hand.

mod app;
mod config_value;
mod harness;
mod ipc_error;
mod prefs;

pub use app::AppInfo;
pub use config_value::{ConfigField, ConfigValue, SecretKind};
pub use harness::HarnessId;
pub use ipc_error::{IpcError, IpcErrorCode};
pub use prefs::{AppPrefs, BinaryOverride, MotionPref, ThemePref, UpdatePolicy};
