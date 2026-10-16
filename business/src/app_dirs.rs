//! The app's own folders outside Tauri, for code that must agree with it: the per-machine data
//! folder, where builds up to 0.16 kept a stable copy of the bundled `harness-memory` server
//! ([`legacy_memory_copy`], retired by `ecosystem::memory_migrate`). On Windows that folder is
//! `%LOCALAPPDATA%`, not the roaming `%APPDATA%` Tauri's `app_data_dir` uses.

use std::path::PathBuf;

use crate::ecosystem::catalog::MEMORY_BINARY;
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

/// The stable copy of the bundled `harness-memory` server that builds up to 0.16 installed on
/// macOS and Windows: `<local data>/bin/harness-memory[.exe]`. `None` on Linux, where that copy
/// was `~/.local/bin/harness-memory`, the path the Harness Memory installer uses now, and on
/// Windows when `LOCALAPPDATA` is unset.
pub fn legacy_memory_copy(env: &Env) -> Option<PathBuf> {
    let name = match env.os {
        OsKind::Linux => return None,
        OsKind::MacOs => MEMORY_BINARY.to_owned(),
        OsKind::Windows => format!("{MEMORY_BINARY}.exe"),
    };

    local_data_dir(env).map(|dir| dir.join("bin").join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_prefers_xdg_data_home_and_has_no_legacy_copy() {
        let env = Env::for_tests("/home/u").with_os(OsKind::Linux);
        assert_eq!(
            local_data_dir(&env),
            Some(PathBuf::from("/home/u/.local/share").join(APP_IDENTIFIER))
        );

        let env = env.with_var("XDG_DATA_HOME", "/data");
        assert_eq!(local_data_dir(&env), Some(PathBuf::from("/data").join(APP_IDENTIFIER)));
        assert_eq!(legacy_memory_copy(&env), None);
    }

    #[test]
    fn the_legacy_copy_is_in_the_local_data_folder_on_macos() {
        let env = Env::for_tests("/Users/u").with_os(OsKind::MacOs);
        let local = PathBuf::from("/Users/u/Library/Application Support").join(APP_IDENTIFIER);

        assert_eq!(legacy_memory_copy(&env), Some(local.join("bin").join("harness-memory")));
    }

    #[test]
    fn windows_uses_local_app_data() {
        let env = Env::for_tests("C:/Users/u").with_os(OsKind::Windows);
        assert_eq!(local_data_dir(&env), None);
        assert_eq!(legacy_memory_copy(&env), None);

        let env = env.with_var("LOCALAPPDATA", "C:/Users/u/AppData/Local");
        let local = PathBuf::from("C:/Users/u/AppData/Local").join(APP_IDENTIFIER);
        assert_eq!(
            legacy_memory_copy(&env),
            Some(local.join("bin").join("harness-memory.exe"))
        );
    }

    #[test]
    fn identifier_matches_tauri_conf() {
        let conf = include_str!("../../src-tauri/tauri.conf.json");
        assert!(conf.contains(&format!("\"identifier\": \"{APP_IDENTIFIER}\"")));
    }
}
