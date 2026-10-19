//! An installed `harness-memory` under a temporary home whose every run is `fake_agent` (see
//! `src/bin/fake_agent/memory.rs` and `memory_cli.rs`), for the tests of what the app reads from the
//! server (`mcp`, `notes`, `scripts list`). The binary in the home is a placeholder: the spawner
//! and the runner replace it, so nothing is copied while other tests fork.
//! Each test binary uses only some of these helpers.
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use harness_business::ecosystem::{McpOptions, MemoryScripts, PoolOptions};
use harness_business::env::Env;
use harness_business::model::{ScriptDraft, ScriptLanguage, ScriptScope};
use harness_business::process::{
    BoxFuture, ExecOutput, ExecSpec, ProcessRunner, ProcessSpawner, SpawnedProcess, TokioRunner, TokioSpawner,
};
use tempfile::TempDir;

pub type Vars = &'static [(&'static str, &'static str)];

/// The arguments that make `fake_agent` serve MCP as `harness-memory mcp`.
pub const CURRENT: &[&str] = &["--dialect", "memory"];
pub const LEGACY: Vars = &[("FAKE_MEMORY_TOOLS", "legacy")];
pub const AGENT_APPROVAL: Vars = &[("FAKE_MEMORY_TOOLS", "agent-approval")];
pub const NO_RUN: Vars = &[("FAKE_MEMORY_TOOLS", "no-run")];
pub const BARE_RUN: Vars = &[("FAKE_MEMORY_TOOLS", "bare-run")];
/// A server whose `notes` and `scripts list` take no pages (a usage error).
pub const UNPAGED: Vars = &[("FAKE_MEMORY_CLI", "unpaged")];
/// A server whose lists predate combined filters: `--scope` and `--review` are usage errors, and
/// lists answer without `scopes` and `review`.
pub const SINGLE: Vars = &[("FAKE_MEMORY_CLI", "single")];

/// Runs the stand-in (with `vars` set) for every spawn and run, and records the requested specs:
/// a spawn runs `fake_agent <args>` (the MCP server), a run `fake_agent <the spec's arguments>`
/// named `harness-memory`.
pub struct StandIn {
    args: &'static [&'static str],
    vars: Mutex<Vars>,
    requested: Mutex<Vec<ExecSpec>>,
}

impl StandIn {
    fn fake(&self, args: &[std::ffi::OsString]) -> ExecSpec {
        let vars = *self.vars.lock().unwrap();

        vars.iter().fold(
            ExecSpec::new(env!("CARGO_BIN_EXE_fake_agent"))
                .args(args)
                .env("FAKE_AGENT_NAME", "harness-memory"),
            |fake, (name, value)| fake.env(name, value),
        )
    }
}

impl ProcessSpawner for StandIn {
    fn spawn(&self, spec: &ExecSpec) -> harness_business::Result<SpawnedProcess> {
        self.requested.lock().unwrap().push(spec.clone());

        let args: Vec<std::ffi::OsString> = self.args.iter().map(Into::into).collect();
        TokioSpawner.spawn(&self.fake(&args))
    }
}

impl ProcessRunner for StandIn {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, harness_business::Result<ExecOutput>> {
        self.requested.lock().unwrap().push(spec.clone());

        let mut fake = self
            .fake(&spec.args)
            .timeout(spec.timeout)
            .max_output_bytes(spec.max_output_bytes);
        fake.cwd = spec.cwd.clone();
        fake.stdin = spec.stdin.clone();

        Box::pin(async move { TokioRunner.run(&fake).await })
    }
}

pub struct Setup {
    _home: TempDir,
    pub env: Env,
    pub binary: PathBuf,
    pub stand_in: Arc<StandIn>,
    pub scripts: MemoryScripts,
}

impl Setup {
    /// A home with `~/.local/bin/harness-memory` (never run: the stand-in replaces it).
    pub fn installed(args: &'static [&'static str], vars: Vars) -> Self {
        let setup = Self::without_server(args, vars);

        std::fs::create_dir_all(setup.binary.parent().unwrap()).unwrap();
        std::fs::write(&setup.binary, "placeholder").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&setup.binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        }

        setup
    }

    pub fn without_server(args: &'static [&'static str], vars: Vars) -> Self {
        let home = tempfile::tempdir().unwrap();
        let env = Env::for_tests(home.path());
        let binary = home
            .path()
            .join(".local/bin")
            .join(format!("harness-memory{}", std::env::consts::EXE_SUFFIX));

        let stand_in = Arc::new(StandIn {
            args,
            vars: Mutex::new(vars),
            requested: Mutex::new(Vec::new()),
        });
        let options = PoolOptions {
            idle_timeout: Duration::from_secs(30),
            client: McpOptions {
                request_timeout: Duration::from_secs(5),
                ..McpOptions::default()
            },
        };

        Self {
            scripts: MemoryScripts::new(stand_in.clone(), options),
            _home: home,
            env,
            binary,
            stand_in,
        }
    }

    /// The runner for the server's commands.
    pub fn runner(&self) -> &dyn ProcessRunner {
        self.stand_in.as_ref()
    }

    /// Replaces the installed binary in place: the stand-in runs with `vars` from now on, and the
    /// binary's modification time changes.
    pub fn upgrade(&self, vars: Vars) {
        *self.stand_in.vars.lock().unwrap() = vars;

        let modified = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        let file = std::fs::File::options().write(true).open(&self.binary).unwrap();
        file.set_modified(modified).unwrap();
    }

    /// Every spec requested so far, spawned or run.
    pub fn requested(&self) -> Vec<ExecSpec> {
        self.stand_in.requested.lock().unwrap().clone()
    }

    /// The specs run (the commands, not the MCP server).
    pub fn runs(&self) -> Vec<ExecSpec> {
        self.requested()
            .into_iter()
            .filter(|spec| spec.args != ["mcp"])
            .collect()
    }

    /// The arguments of command run `run` (counted as [`Self::runs`]), as text.
    pub fn run_args(&self, run: usize) -> Vec<String> {
        self.runs()[run]
            .args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    /// Asserts that the first command run was the installed binary with `args`, in the home
    /// directory, within 15 s.
    pub fn assert_first_run(&self, args: &[&str]) {
        let runs = self.runs();
        let given: Vec<String> = runs[0]
            .args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        assert_eq!(given, args);
        assert_eq!(runs[0].program, self.binary);
        assert_eq!(runs[0].cwd.as_deref(), Some(self.env.home.as_path()));
        assert_eq!(runs[0].timeout, Duration::from_secs(15));
    }

    /// Windows has no `/` root, so the project is a folder in the temp home.
    pub fn project(&self) -> PathBuf {
        self.env.home.join("projects/demo")
    }
}

/// A repository script as the app's editor saves it.
pub fn script(name: &str) -> ScriptDraft {
    ScriptDraft {
        name: name.to_owned(),
        description: "Written in the app".to_owned(),
        language: ScriptLanguage::Sh,
        body: "echo app\n".to_owned(),
        timeout_secs: 45,
        scope: ScriptScope::Repo,
    }
}
