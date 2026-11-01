//! An installed `harness-memory` under a temporary home, and a runner that stands in for it, for
//! the tests of the commands the app runs on the server (`check`, `trust`).
//! Each test binary uses only some of these helpers.
#![allow(dead_code)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use harness_business::Result;
use harness_business::env::Env;
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ProcessRunner};

use crate::common::set_mode;

/// A home with an installed `harness-memory` (never run: the stand-in answers).
pub fn installed() -> (tempfile::TempDir, Env) {
    installed_binary("harness-memory")
}

/// A home with an installed Harness family server `binary` (never run).
pub fn installed_binary(binary: &str) -> (tempfile::TempDir, Env) {
    let dir = tempfile::tempdir().unwrap();
    let (env, path) = installed_binary_at(dir.path(), binary);
    install(&path, None);

    (dir, env)
}

/// Where the installer puts `harness-memory` under `home`, and the env that finds it and its
/// config file there (Windows keeps the config under `APPDATA`).
pub fn installed_at(home: &Path) -> (Env, PathBuf) {
    installed_binary_at(home, "harness-memory")
}

/// [`installed_at`] for the Harness family server `binary`.
pub fn installed_binary_at(home: &Path, binary: &str) -> (Env, PathBuf) {
    let env = Env::for_tests(home);

    if cfg!(windows) {
        let local = home.join("AppData").join("Local");
        let roaming = home.join("AppData").join("Roaming");
        let path = local.join("Programs").join(binary).join(format!("{binary}.exe"));
        let env = env
            .with_var("LOCALAPPDATA", local.to_string_lossy())
            .with_var("APPDATA", roaming.to_string_lossy());

        return (env, path);
    }

    (env, home.join(".local").join("bin").join(binary))
}

/// Places `source` as the installed server (an empty executable without one).
pub fn install(path: &Path, source: Option<&Path>) {
    let dir = path.parent().unwrap();
    std::fs::create_dir_all(dir).unwrap();
    set_mode(dir, 0o755);

    match source {
        Some(source) => {
            std::fs::copy(source, path).unwrap();
        }
        None => std::fs::write(path, "").unwrap(),
    }

    set_mode(path, 0o755);
}

/// Answers `--version` as Harness Memory, and anything else with `answer`; keeps those specs.
pub struct Stub {
    pub answer: ExecOutput,
    pub runs: Mutex<Vec<ExecSpec>>,
}

impl ProcessRunner for Stub {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        let output = if spec.args == [OsString::from("--version")] {
            ExecOutput {
                status: Some(0),
                stdout: "harness-memory 1.2.0\n".to_owned(),
                ..ExecOutput::default()
            }
        } else {
            self.runs.lock().unwrap().push(spec.clone());
            self.answer.clone()
        };

        Box::pin(async move { Ok(output) })
    }
}

/// A [`Stub`] that answers `answer`.
pub fn stub(answer: ExecOutput) -> Stub {
    Stub {
        answer,
        runs: Mutex::new(Vec::new()),
    }
}
