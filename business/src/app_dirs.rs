//! The app's own folders outside Tauri, for code that must agree with it: the config folder
//! ([`config_dir`], the one Tauri's `app_config_dir` names, for the MCP server mode
//! `harness-desktop mcp`, which starts no Tauri runtime) and the per-machine data folder, where
//! builds up to 0.16 kept a stable copy of the bundled `harness-memory` server
//! ([`legacy_memory_copy`], retired by `ecosystem::memory_migrate`). On Windows that data folder is
//! `%LOCALAPPDATA%`, not the roaming `%APPDATA%` Tauri's `app_data_dir` uses.

use std::path::PathBuf;

use crate::ecosystem::catalog::MEMORY_BINARY;
use crate::env::{Env, OsKind};

/// The bundle identifier in `src-tauri/tauri.conf.json`; Tauri names the app's folders after it.
pub const APP_IDENTIFIER: &str = "com.mspiechowicz.harness-useful-desktop";

/// The app's config folder, as Tauri's `app_config_dir` gives it: `$XDG_CONFIG_HOME` when it is
/// absolute (else `~/.config`) on Linux, `~/Library/Application Support` on macOS, the roaming
/// `%APPDATA%` (else `~/AppData/Roaming`) on Windows, each followed by [`APP_IDENTIFIER`]. The
/// prefs, the hook approvals and the registration records live there.
pub fn config_dir(env: &Env) -> PathBuf {
    let absolute = |name: &str| env.var(name).map(PathBuf::from).filter(|dir| dir.has_root());

    let base = match env.os {
        OsKind::Linux => absolute("XDG_CONFIG_HOME").unwrap_or_else(|| env.home_join(".config")),
        OsKind::MacOs => env.home_join("Library/Application Support"),
        OsKind::Windows => absolute("APPDATA").unwrap_or_else(|| env.home_join("AppData/Roaming")),
    };

    base.join(APP_IDENTIFIER)
}

/// A Harness family data folder shared with another program: `override_var` when it is set to an
/// absolute path (a leading `~` expanded; a relative one is ignored), else `$XDG_DATA_HOME`
/// (absolute only, or `~/.local/share`) on Linux, `~/Library/Application Support` on macOS and
/// `%LOCALAPPDATA%` (absolute only) on Windows, followed by `folder`. `None` on Windows while
/// `LOCALAPPDATA` is unset.
pub fn shared_data_dir(env: &Env, override_var: &str, folder: &str) -> Option<PathBuf> {
    let rooted = |name: &str| env.var(name).map(PathBuf::from).filter(|dir| dir.has_root());

    if let Some(dir) = env
        .var(override_var)
        .map(|dir| env.expand_home(dir))
        .filter(|dir| dir.has_root())
    {
        return Some(dir);
    }

    let base = match env.os {
        OsKind::Linux => rooted("XDG_DATA_HOME").unwrap_or_else(|| env.home_join(".local/share")),
        OsKind::MacOs => env.home_join("Library/Application Support"),
        OsKind::Windows => rooted("LOCALAPPDATA")?,
    };

    Some(base.join(folder))
}

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

/// The folder the app writes the chat exports it makes itself into (OpenCode's HTML export):
/// `exports` in [`local_data_dir`]. The desktop lets the file manager reveal files in it.
pub fn exports_dir(env: &Env) -> Option<PathBuf> {
    local_data_dir(env).map(|dir| dir.join("exports"))
}

/// The folder where each running app records the chat servers and agents it started, so the next
/// launch can stop the ones a crash left behind (`process::sweep_orphans`): `child-processes` in
/// [`local_data_dir`] (process ids are per machine).
pub fn child_processes_dir(env: &Env) -> Option<PathBuf> {
    local_data_dir(env).map(|dir| dir.join("child-processes"))
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
    fn the_config_folder_is_the_one_tauri_names() {
        let linux = Env::for_tests("/home/u").with_os(OsKind::Linux);
        assert_eq!(
            config_dir(&linux),
            PathBuf::from("/home/u/.config").join(APP_IDENTIFIER)
        );
        assert_eq!(
            config_dir(&linux.clone().with_var("XDG_CONFIG_HOME", "/cfg")),
            PathBuf::from("/cfg").join(APP_IDENTIFIER)
        );
        assert_eq!(
            config_dir(&linux.with_var("XDG_CONFIG_HOME", "relative")),
            PathBuf::from("/home/u/.config").join(APP_IDENTIFIER),
            "a relative XDG_CONFIG_HOME is ignored, as by Tauri"
        );

        let mac = Env::for_tests("/Users/u").with_os(OsKind::MacOs);
        assert_eq!(
            config_dir(&mac),
            PathBuf::from("/Users/u/Library/Application Support").join(APP_IDENTIFIER)
        );

        let windows = Env::for_tests("C:/Users/u").with_os(OsKind::Windows);
        assert_eq!(
            config_dir(&windows),
            Env::for_tests("C:/Users/u")
                .home_join("AppData/Roaming")
                .join(APP_IDENTIFIER)
        );

        let roaming = PathBuf::from("C:/Users/u/AppData/Roaming");
        if roaming.has_root() {
            assert_eq!(
                config_dir(&windows.with_var("APPDATA", "C:/Users/u/AppData/Roaming")),
                roaming.join(APP_IDENTIFIER)
            );
        }
    }

    #[test]
    fn identifier_matches_tauri_conf() {
        let conf = include_str!("../../src-tauri/tauri.conf.json");
        assert!(conf.contains(&format!("\"identifier\": \"{APP_IDENTIFIER}\"")));
    }
}
