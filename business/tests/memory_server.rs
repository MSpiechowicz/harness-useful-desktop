//! Finding the installed `harness-memory` server and its version: the installer's folder first
//! (`~/.local/bin`, and on Windows `%LOCALAPPDATA%\Programs\harness-memory`), then `PATH`, then the
//! usual install directories; only files no other user can change (Unix) and only `.exe` files
//! (Windows); and only a `harness-memory <version>` answer to `--version` counts.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use harness_business::Result;
use harness_business::ecosystem::{installed_memory, memory_version, parse_memory_version, resolve_memory};
use harness_business::env::{Env, OsKind};
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ProcessRunner, ResolvedBinary};
use pretty_assertions::assert_eq;

/// An executable file at `path`, its folder created; on Unix both are writable by their owner only.
fn executable(path: &Path) -> PathBuf {
    let dir = path.parent().unwrap();
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(path, "").unwrap();

    set_mode(dir, 0o755);
    set_mode(path, 0o755);

    path.to_path_buf()
}

#[cfg_attr(not(unix), allow(unused_variables))]
fn set_mode(path: &Path, mode: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    }
}

fn binary_name() -> String {
    format!("harness-memory{}", std::env::consts::EXE_SUFFIX)
}

fn resolved(env: &Env) -> Option<PathBuf> {
    resolve_memory(env).map(|found| found.path)
}

#[test]
// `~/.local/bin` is the installer's folder only on Linux and macOS (Windows has its own, tested apart).
#[cfg(unix)]
fn the_installers_folder_comes_before_path_and_path_before_the_install_directories() {
    let home = tempfile::tempdir().unwrap();
    let env = Env::for_tests(home.path()).with_var("PATH", home.path().join("tools").to_string_lossy());

    let bun = executable(&home.path().join(".bun/bin").join(binary_name()));
    assert_eq!(resolved(&env), Some(bun));

    let on_path = executable(&home.path().join("tools").join(binary_name()));
    assert_eq!(resolved(&env), Some(on_path));

    let installed = executable(&home.path().join(".local/bin").join(binary_name()));
    assert_eq!(resolved(&env), Some(installed));
}

#[test]
fn nothing_is_found_in_an_empty_home() {
    let home = tempfile::tempdir().unwrap();

    assert_eq!(resolve_memory(&Env::for_tests(home.path())), None);
}

#[test]
fn on_windows_the_install_ps1_folder_comes_first_and_only_exe_files_count() {
    let home = tempfile::tempdir().unwrap();
    let local = home.path().join("AppData").join("Local");
    let env = Env::for_tests(home.path())
        .with_os(OsKind::Windows)
        .with_var("LOCALAPPDATA", local.to_string_lossy());

    for wrapper in ["cmd", "bat", "com"] {
        executable(&home.path().join(format!(".local/bin/harness-memory.{wrapper}")));
    }
    assert_eq!(resolved(&env), None, "no batch file or .com is run");

    let local_bin = executable(&home.path().join(".local/bin/harness-memory.exe"));
    assert_eq!(resolved(&env), Some(local_bin));

    let programs = executable(&local.join("Programs").join("harness-memory").join("harness-memory.exe"));
    assert_eq!(resolved(&env), Some(programs));
}

#[cfg(unix)]
#[test]
fn a_file_or_folder_others_may_change_is_skipped() {
    let home = tempfile::tempdir().unwrap();
    let env = Env::for_tests(home.path());
    let installed = executable(&home.path().join(".local/bin/harness-memory"));
    let fallback = executable(&home.path().join(".cargo/bin/harness-memory"));

    for mode in [0o775, 0o757] {
        set_mode(&installed, mode);
        assert_eq!(resolved(&env), Some(fallback.clone()), "file mode {mode:o}");
    }
    set_mode(&installed, 0o755);

    set_mode(installed.parent().unwrap(), 0o775);
    assert_eq!(
        resolved(&env),
        Some(installed.clone()),
        "a group-writable folder is fine (umask 002 makes ~/.local/bin 0775)"
    );

    set_mode(installed.parent().unwrap(), 0o777);
    assert_eq!(resolved(&env), Some(fallback.clone()), "a world-writable folder");

    set_mode(installed.parent().unwrap(), 0o1777);
    assert_eq!(resolved(&env), Some(installed.clone()), "a sticky folder is fine");

    set_mode(installed.parent().unwrap(), 0o755);
    set_mode(fallback.parent().unwrap(), 0o777);
    std::fs::remove_file(&installed).unwrap();
    assert_eq!(resolved(&env), None);
}

#[cfg(unix)]
#[test]
fn a_symlink_into_a_folder_others_may_change_is_skipped() {
    let home = tempfile::tempdir().unwrap();
    let env = Env::for_tests(home.path());
    let shared = executable(&home.path().join("shared/harness-memory"));
    set_mode(shared.parent().unwrap(), 0o777);

    let bin = home.path().join(".local/bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::os::unix::fs::symlink(&shared, bin.join("harness-memory")).unwrap();

    assert_eq!(resolved(&env), None);
}

/// Answers `--version` per program: `harness-memory 1.0.0` for the ones in `good`, nothing for the
/// others.
struct Versions {
    good: Vec<PathBuf>,
    asked: Mutex<Vec<PathBuf>>,
}

impl ProcessRunner for Versions {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        self.asked.lock().unwrap().push(spec.program.clone());
        let stdout = if self.good.contains(&spec.program) {
            "harness-memory 1.0.0\n"
        } else {
            ""
        };

        Box::pin(async move {
            Ok(ExecOutput {
                status: Some(0),
                stdout: stdout.to_owned(),
                ..ExecOutput::default()
            })
        })
    }
}

#[tokio::test]
// `~/.local/bin` is the installer's folder only on Linux and macOS (Windows has its own, tested apart).
#[cfg(unix)]
async fn the_installers_server_is_used_over_a_silent_one_on_path() {
    let home = tempfile::tempdir().unwrap();
    let bun = home.path().join(".bun/bin");
    let env = Env::for_tests(home.path()).with_var("PATH", bun.to_string_lossy());
    executable(&bun.join(binary_name()));
    let installed = executable(&home.path().join(".local/bin").join(binary_name()));
    let runner = Versions {
        good: vec![installed.clone()],
        asked: Mutex::new(Vec::new()),
    };

    let found = installed_memory(&env, &runner).await.map(|binary| binary.path);

    assert_eq!(found, Some(installed.clone()));
    assert_eq!(runner.asked.into_inner().unwrap(), [installed], "asked once");
}

#[tokio::test]
// `~/.local/bin` is the installer's folder only on Linux and macOS (Windows has its own, tested apart).
#[cfg(unix)]
async fn a_server_that_reports_no_version_is_not_installed() {
    let home = tempfile::tempdir().unwrap();
    let env = Env::for_tests(home.path());
    let silent = executable(&home.path().join(".local/bin").join(binary_name()));
    executable(&home.path().join(".cargo/bin").join(binary_name()));
    let runner = Versions {
        good: Vec::new(),
        asked: Mutex::new(Vec::new()),
    };

    assert_eq!(installed_memory(&env, &runner).await, None);
    assert_eq!(
        runner.asked.into_inner().unwrap(),
        [silent],
        "no later candidate is tried"
    );
}

#[test]
fn only_a_harness_memory_version_line_counts() {
    assert_eq!(parse_memory_version("harness-memory 1.0.0\n"), Some("1.0.0".to_owned()));
    assert_eq!(
        parse_memory_version("\n  harness-memory 1.2.3-rc.1  \r\n"),
        Some("1.2.3-rc.1".to_owned())
    );

    for output in [
        "",
        "1.0.0\n",
        "harness-dashboard 1.0.0\n",
        "harness-memory\n",
        "harness-memory version 1.0.0\n",
        "harness-memory 1.0.0 (built from source)\n",
        "harness-memory1.0.0\n",
        "usage: harness-memory [mcp | --version]\nharness-memory 1.0.0\n",
    ] {
        assert_eq!(parse_memory_version(output), None, "{output:?}");
    }
}

/// Answers every command with `stdout` and `status`, keeping the arguments.
struct Answers {
    status: i32,
    stdout: &'static str,
    args: Mutex<Vec<Vec<String>>>,
}

impl ProcessRunner for Answers {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        let args = spec.args.iter().map(|arg| arg.to_string_lossy().into_owned());
        self.args.lock().unwrap().push(args.collect());

        Box::pin(async move {
            Ok(ExecOutput {
                status: Some(self.status),
                stdout: self.stdout.to_owned(),
                ..ExecOutput::default()
            })
        })
    }
}

async fn probe(status: i32, stdout: &'static str) -> (Option<String>, Vec<Vec<String>>) {
    let env = Env::for_tests("/home/u");
    let binary = ResolvedBinary {
        path: PathBuf::from("/home/u/.local/bin/harness-memory"),
        is_batch_script: false,
    };
    let runner = Answers {
        status,
        stdout,
        args: Mutex::new(Vec::new()),
    };

    let version = memory_version(&env, &runner, &binary).await;

    (version, runner.args.into_inner().unwrap())
}

#[tokio::test]
async fn the_version_comes_from_dash_dash_version() {
    let (version, args) = probe(0, "harness-memory 1.0.0\n").await;

    assert_eq!(version.as_deref(), Some("1.0.0"));
    assert_eq!(args, [vec!["--version".to_owned()]]);
}

#[tokio::test]
async fn a_failing_probe_has_no_version() {
    assert_eq!(probe(1, "harness-memory 1.0.0\n").await.0, None);
}
