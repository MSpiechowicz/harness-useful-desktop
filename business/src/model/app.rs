use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Static facts about the running application, shown in Settings/About.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppInfo {
    /// Product name.
    pub name: String,
    /// Application version (semver).
    pub version: String,
    /// Operating system (`linux`, `macos`, `windows`, ...).
    pub os: String,
    /// CPU architecture (`x86_64`, `aarch64`, ...).
    pub arch: String,
    /// True for debug builds.
    pub debug: bool,
}

impl AppInfo {
    /// Builds the info for the current process; `os`, `arch` and `debug` come from the build target.
    pub fn current(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            os: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
            debug: cfg!(debug_assertions),
        }
    }
}
