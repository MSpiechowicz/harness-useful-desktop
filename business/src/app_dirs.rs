//! The app's own folders outside Tauri, for code that runs without it (the `harness-memory`
//! server) or must agree with it (the stable copy of that server).
//!
//! Memory lives in the per-machine data folder: on Windows that is `%LOCALAPPDATA%`, not the
//! roaming `%APPDATA%` Tauri's `app_data_dir` uses, because a SQLite database does not belong in a
//! roaming profile.

use std::path::PathBuf;

use crate::env::{Env, OsKind};

/// The bundle identifier in `src-tauri/tauri.conf.json`; Tauri names the app's folders after it.
pub const APP_IDENTIFIER: &str = "com.mspiechowicz.harness-useful-desktop";

/// The per-machine data folder of the app: `$XDG_DATA_HOME` (or `~/.local/share`) on Linux,
/// `~/Library/Application Support` on macOS, `%LOCALAPPDATA%` on Windows, each followed by
/// [`APP_IDENTIFIER`]. `None` on Windows when `LOCALAPPDATA` is unset.
pub fn local_data_dir(env: &Env) -> Option<PathBuf> {
    let base = match env.os {
        OsKind::Linux => match env.var("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
            Some(value) => PathBuf::from(value),
            None => env.home_join(".local/share"),
        },
        OsKind::MacOs => env.home_join("Library/Application Support"),
        OsKind::Windows => PathBuf::from(env.var("LOCALAPPDATA").filter(|value| !value.is_empty())?),
    };

    Some(base.join(APP_IDENTIFIER))
}

/// The folder holding the memory database (`<local data>/memory`).
pub fn memory_dir(env: &Env) -> Option<PathBuf> {
    local_data_dir(env).map(|dir| dir.join("memory"))
}

/// Where the app keeps its stable copy of the `harness-memory` server, whose absolute path the
/// harnesses' `mcp.json` entries name: `~/.local/bin` on Linux, `<local data>/bin` elsewhere.
pub fn memory_bin_dir(env: &Env) -> Option<PathBuf> {
    match env.os {
        OsKind::Linux => Some(env.home_join(".local/bin")),
        OsKind::MacOs | OsKind::Windows => local_data_dir(env).map(|dir| dir.join("bin")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_prefers_xdg_data_home() {
        let env = Env::for_tests("/home/u").with_os(OsKind::Linux);
        assert_eq!(
            memory_dir(&env),
            Some(
                PathBuf::from("/home/u/.local/share")
                    .join(APP_IDENTIFIER)
                    .join("memory")
            )
        );

        let env = env.with_var("XDG_DATA_HOME", "/data");
        assert_eq!(local_data_dir(&env), Some(PathBuf::from("/data").join(APP_IDENTIFIER)));
        assert_eq!(memory_bin_dir(&env), Some(PathBuf::from("/home/u/.local/bin")));
    }

    #[test]
    fn windows_uses_local_app_data() {
        let env = Env::for_tests("C:/Users/u").with_os(OsKind::Windows);
        assert_eq!(local_data_dir(&env), None);

        let env = env.with_var("LOCALAPPDATA", "C:/Users/u/AppData/Local");
        let local = PathBuf::from("C:/Users/u/AppData/Local").join(APP_IDENTIFIER);
        assert_eq!(memory_bin_dir(&env), Some(local.join("bin")));
    }

    #[test]
    fn identifier_matches_tauri_conf() {
        let conf = include_str!("../../src-tauri/tauri.conf.json");
        assert!(conf.contains(&format!("\"identifier\": \"{APP_IDENTIFIER}\"")));
    }
}
