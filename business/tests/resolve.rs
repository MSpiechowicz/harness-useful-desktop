//! Binary discovery: override precedence, PATH, extra install dirs, batch-script detection.

use std::fs;
use std::path::{Path, PathBuf};

use harness_business::env::{Env, OsKind};
use harness_business::model::{BinaryOverride, HarnessId};
use harness_business::process::{find_override, is_batch_script, resolve_binary};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

/// A name no real system has on PATH or in /usr/local/bin.
const TOOL: &str = "harness-resolve-test-tool-zq";

fn executable(path: PathBuf) -> PathBuf {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "").unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    path
}

/// The executable file name for the current OS (`which` needs `.exe` on Windows).
fn tool_file() -> String {
    if cfg!(windows) {
        format!("{TOOL}.exe")
    } else {
        TOOL.to_owned()
    }
}

#[test]
fn explicit_override_wins_over_path_and_extra_dirs() {
    let home = TempDir::new().unwrap();
    let on_path = executable(home.path().join("bin").join(tool_file()));
    executable(home.path().join(".bun/bin").join(tool_file()));
    let chosen = executable(home.path().join("custom").join(tool_file()));
    let env = Env::for_tests(home.path()).with_var("PATH", on_path.parent().unwrap().to_string_lossy());

    let resolved = resolve_binary(&env, TOOL, Some(&chosen)).unwrap();

    assert_eq!(resolved.path, chosen);
    assert!(!resolved.is_batch_script);
}

#[test]
fn unusable_override_falls_back_to_discovery() {
    let home = TempDir::new().unwrap();
    let found = executable(home.path().join(".bun/bin").join(tool_file()));
    let env = Env::for_tests(home.path());

    let relative = resolve_binary(&env, TOOL, Some(Path::new("bin/tool"))).unwrap();
    let missing = resolve_binary(&env, TOOL, Some(&home.path().join("missing"))).unwrap();

    assert_eq!(relative.path, found);
    assert_eq!(missing.path, found);
}

#[test]
fn path_is_searched_before_extra_dirs() {
    let home = TempDir::new().unwrap();
    let on_path = executable(home.path().join("bin").join(tool_file()));
    executable(home.path().join(".local/bin").join(tool_file()));
    let env = Env::for_tests(home.path()).with_var("PATH", on_path.parent().unwrap().to_string_lossy());

    assert_eq!(resolve_binary(&env, TOOL, None).unwrap().path, on_path);
}

#[test]
fn relative_and_empty_path_entries_are_never_searched() {
    let home = TempDir::new().unwrap();
    executable(home.path().join("bin").join(tool_file()));
    executable(home.path().join(tool_file()));
    let fallback = executable(home.path().join(".bun/bin").join(tool_file()));

    let separator = if cfg!(windows) { ";" } else { ":" };
    let path_var = [".", "", "bin"].join(separator);
    let env = Env::for_tests(home.path()).with_var("PATH", path_var);

    // Only the absolute install-dir fallback may match, never `<home>/bin` or `<home>/.`.
    assert_eq!(resolve_binary(&env, TOOL, None).unwrap().path, fallback);
}

#[test]
fn extra_dirs_in_the_home_are_discovered_without_path() {
    let home = TempDir::new().unwrap();
    let env = Env::for_tests(home.path());

    assert_eq!(resolve_binary(&env, TOOL, None), None);

    let found = executable(home.path().join(".cargo/bin").join(tool_file()));
    let earlier = executable(home.path().join(".local/bin").join(tool_file()));

    assert_ne!(found, earlier);
    assert_eq!(resolve_binary(&env, TOOL, None).unwrap().path, earlier);
}

#[cfg(unix)]
#[test]
fn non_executable_files_are_skipped_on_unix() {
    let home = TempDir::new().unwrap();
    let path = home.path().join(".bun/bin").join(TOOL);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "").unwrap();

    assert_eq!(resolve_binary(&Env::for_tests(home.path()), TOOL, None), None);
}

#[test]
fn windows_npm_shims_are_found_and_flagged_as_batch_scripts() {
    let home = TempDir::new().unwrap();
    let app_data = home.path().join("AppData/Roaming");
    let shim = executable(app_data.join("npm").join(format!("{TOOL}.cmd")));
    let env = Env::for_tests(home.path())
        .with_os(OsKind::Windows)
        .with_var("APPDATA", app_data.to_string_lossy());

    let resolved = resolve_binary(&env, TOOL, None).unwrap();

    assert_eq!(resolved.path, shim);
    assert!(resolved.is_batch_script);
}

#[test]
fn batch_script_detection_is_by_extension() {
    for (path, expected) in [
        ("C:/npm/omp.cmd", true),
        ("C:/npm/omp.CMD", true),
        ("C:/npm/omp.bat", true),
        ("C:/npm/omp.exe", false),
        ("/usr/bin/omp", false),
        ("/usr/bin/omp.cmd.txt", false),
    ] {
        assert_eq!(is_batch_script(Path::new(path)), expected, "{path}");
    }
}

#[test]
fn the_last_override_for_a_harness_wins() {
    let overrides = [
        BinaryOverride {
            harness: HarnessId::Omp,
            path: "/a/omp".into(),
        },
        BinaryOverride {
            harness: HarnessId::Pi,
            path: "/a/pi".into(),
        },
        BinaryOverride {
            harness: HarnessId::Omp,
            path: "/b/omp".into(),
        },
    ];

    assert_eq!(find_override(&overrides, HarnessId::Omp), Some(Path::new("/b/omp")));
    assert_eq!(find_override(&overrides, HarnessId::Pi), Some(Path::new("/a/pi")));
    assert_eq!(find_override(&overrides[1..2], HarnessId::Omp), None);
}
