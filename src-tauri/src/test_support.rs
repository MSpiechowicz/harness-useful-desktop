//! Test helpers for the shell's unit tests.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use harness_business::agent::{AgentManager, SessionOptions};
use harness_business::ecosystem::mcp_target_paths;
use harness_business::env::Env;
use harness_business::model::{AgentStartRequest, Detection, HarnessId, HarnessSummary, VersionSupport};
use harness_business::process::{ExecSpec, ProcessSpawner, SpawnedProcess, TokioSpawner};
use harness_business::project::Projects;
use serde_json::Value;
use tauri::async_runtime::block_on;
use tauri::ipc::{Channel, InvokeResponseBody};

use crate::agents::Agents;
use crate::state::AppState;

/// A uniquely named directory under the OS temp dir, removed on drop. Its path is canonical, so
/// containment checks compare like with like (macOS `/var` → `/private/var`).
pub struct TempDir(PathBuf);

impl TempDir {
    /// Creates `harness-desktop-<label>-<pid>-<n>`.
    pub fn new(label: &str) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);

        let name = format!(
            "harness-desktop-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&path).expect("create temp dir");

        // `dunce`, as the business crate stores projects: std's canonical form on Windows is `\\?\C:\...`,
        // which would never match a stored project path.
        Self(dunce::canonicalize(&path).expect("canonical temp dir"))
    }

    /// The directory.
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// App state over a fresh `<dir>/home` with its prefs in `<dir>/config`; nothing is detected yet.
pub fn home_state(dir: &TempDir) -> AppState {
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).expect("create home");

    // Windows keeps the app's per-machine data (the old copy of the memory server) in %LOCALAPPDATA%.
    let local = home.join("AppData").join("Local");
    let env = Env::for_tests(&home).with_var("LOCALAPPDATA", local.to_string_lossy());

    AppState::new(dir.path().join("config"), env)
}

/// Places an empty executable `name` (`.exe` on Windows) in `<home>/.local/bin`, where companion
/// binaries are looked up, and returns its path.
pub fn place_companion(home: &Path, name: &str) -> PathBuf {
    place_executable(&home.join(".local").join("bin"), name)
}

/// Places an empty executable `name` (`.exe` on Windows) in `dir`; it prints nothing when run.
pub fn place_executable(dir: &Path, name: &str) -> PathBuf {
    let binary = executable_in(dir, name);
    std::fs::write(&binary, "").expect("write binary");
    set_mode(&binary, 0o755);

    binary
}

/// Places a `harness-memory` that reports its version in `<home>/.local/bin`, where its installer
/// puts it (see [`place_memory_server_in`]).
pub fn place_memory_server(home: &Path) -> Option<PathBuf> {
    place_memory_server_in(&home.join(".local").join("bin"))
}

/// Places a `harness-memory` (`.exe` on Windows) in `dir` that answers `--version` with
/// `harness-memory 0.0.0`: a copy of `fake_agent`, see its docs. `None` (the caller skips) when
/// `fake_agent` is not built.
pub fn place_memory_server_in(dir: &Path) -> Option<PathBuf> {
    let agent = fake_agent()?;
    let binary = executable_in(dir, "harness-memory");
    std::fs::copy(&agent, &binary).expect("copy fake_agent");
    set_mode(&binary, 0o755);

    // A process another test forks while the copy is open for writing keeps it open until its own
    // exec, and until then running the copy fails ("text file busy").
    wait_until("the copied harness-memory runs", || {
        std::process::Command::new(&binary).arg("--version").output().is_ok()
    });

    Some(binary)
}

/// `<dir>/<name>` (`.exe` on Windows), with `dir` created and writable by its owner only.
fn executable_in(dir: &Path, name: &str) -> PathBuf {
    std::fs::create_dir_all(dir).expect("create bin");
    set_mode(dir, 0o755);

    dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

/// Sets the Unix permission bits of `path` to `mode` (whatever the umask; the memory server is
/// only run from files and folders no other user can change). Does nothing on Windows.
#[cfg_attr(not(unix), allow(unused_variables))]
pub fn set_mode(path: &Path, mode: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("chmod");
    }
}

/// The user MCP file of OMP in `state`'s home.
pub fn omp_mcp_file(state: &AppState) -> PathBuf {
    let omp = state.registry.get(HarnessId::Omp).expect("omp");

    mcp_target_paths(&state.env, omp).expect("omp has a user MCP file").0
}

/// App state over `env` with its prefs in `<dir>/config` and `project` created and opened (the
/// active project).
pub fn state_with_project(dir: &TempDir, env: Env, project: &Path) -> AppState {
    std::fs::create_dir_all(project).expect("create project");

    let state = AppState::new(dir.path().join("config"), env);
    state.prefs.with(|store| {
        Projects::new(store, &state.env).open(project).expect("open project");
    });

    state
}

/// A temporary home with `home/demo` open as the active project and `home/stranger`, a folder
/// that is neither open nor recent, for the commands that take a project.
pub struct ProjectHome {
    pub home: PathBuf,
    pub project: PathBuf,
    pub stranger: PathBuf,
    pub state: AppState,
    _dir: TempDir,
}

impl ProjectHome {
    pub fn new(name: &str) -> Self {
        let dir = TempDir::new(name);
        let home = dir.path().join("home");
        let project = home.join("demo");
        let state = state_with_project(&dir, Env::for_tests(&home), &project);
        let stranger = home.join("stranger");
        std::fs::create_dir_all(&stranger).expect("create stranger");

        Self {
            home,
            project,
            stranger,
            state,
            _dir: dir,
        }
    }
}

/// `path` as the `project` argument of a command.
pub fn path_arg(path: &Path) -> Option<String> {
    Some(path.to_string_lossy().into_owned())
}

/// A channel that keeps every message it receives, parsed as JSON.
pub fn collecting_channel<T>() -> (Channel<T>, Arc<Mutex<Vec<Value>>>) {
    let received = Arc::new(Mutex::new(Vec::new()));
    let messages = received.clone();

    let channel = Channel::new(move |body| {
        if let InvokeResponseBody::Json(json) = body {
            let message = serde_json::from_str(&json).expect("channel messages are JSON");
            messages.lock().unwrap_or_else(PoisonError::into_inner).push(message);
        }
        Ok(())
    });

    (channel, received)
}

/// Caches a detection of `harness`: installed at `binary`, or not installed.
pub fn cache_detection(state: &AppState, harness: HarnessId, binary: Option<&Path>) {
    let found = state.registry.get(harness).expect("the harness is registered");
    let summary = HarnessSummary {
        info: found.info(),
        capabilities: found.capabilities(),
        detection: Detection {
            installed: binary.is_some(),
            binary_path: binary.map(|path| path.to_string_lossy().into_owned()),
            is_batch_script: false,
            version: None,
            version_support: VersionSupport::Unknown,
            config_dir: None,
            config_dir_exists: false,
            install_hint: None,
            error: None,
        },
    };

    block_on(state.detections.write()).insert(harness, summary);
}

/// Set to `1` (CI) to make the tests that need `fake_agent` fail instead of skipping.
const REQUIRE_FAKE_AGENT: &str = "HARNESS_REQUIRE_FAKE_AGENT";

/// `fake_agent` from the business crate, built next to this test binary by
/// `cargo test --workspace` (Cargo exposes `CARGO_BIN_EXE_*` only to the owning package).
/// `None` (the caller skips) when it is missing, unless [`REQUIRE_FAKE_AGENT`] is `1`.
pub fn fake_agent() -> Option<PathBuf> {
    let found = built_fake_agent();

    if found.is_none() {
        assert_ne!(
            std::env::var(REQUIRE_FAKE_AGENT).as_deref(),
            Ok("1"),
            "fake_agent is not built but {REQUIRE_FAKE_AGENT}=1 \
             (run `cargo build -p harness_business --bin fake_agent` first)"
        );
        eprintln!("skipped: fake_agent is not built (run `cargo test --workspace`)");
    }

    found
}

fn built_fake_agent() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let target_dir = exe.parent()?.parent()?;
    let path = target_dir.join(format!("fake_agent{}", std::env::consts::EXE_SUFFIX));

    path.is_file().then_some(path)
}

/// Runs `fake_agent --dialect <omp|mcp> --scenario <scenario>` in the requested directory, keeping
/// the pids it started.
pub struct ScenarioSpawner {
    binary: PathBuf,
    dialect: &'static str,
    scenario: PathBuf,
    spawned: AtomicUsize,
    pids: Mutex<Vec<u32>>,
}

impl ScenarioSpawner {
    /// Plays `scenario`, a stream under `business/tests/fixtures/streams`, as an OMP agent with the
    /// fake agent at `binary`.
    pub fn new(binary: &Path, scenario: &str) -> Arc<Self> {
        Self::with_dialect(binary, "omp", scenario)
    }

    /// Plays `scenario` as an MCP server (a scripted `harness-memory`).
    pub fn mcp(binary: &Path, scenario: &str) -> Arc<Self> {
        Self::with_dialect(binary, "mcp", scenario)
    }

    fn with_dialect(binary: &Path, dialect: &'static str, scenario: &str) -> Arc<Self> {
        Arc::new(Self {
            binary: binary.to_path_buf(),
            dialect,
            scenario: Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../business/tests/fixtures/streams")
                .join(scenario),
            spawned: AtomicUsize::new(0),
            pids: Mutex::new(Vec::new()),
        })
    }

    /// How many processes were spawned.
    pub fn spawned(&self) -> usize {
        self.spawned.load(Ordering::Relaxed)
    }

    /// The pids of the spawned processes, oldest first.
    pub fn pids(&self) -> Vec<u32> {
        self.pids.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }
}

impl ProcessSpawner for ScenarioSpawner {
    fn spawn(&self, spec: &ExecSpec) -> harness_business::Result<SpawnedProcess> {
        self.spawned.fetch_add(1, Ordering::Relaxed);

        let scenario = self.scenario.to_string_lossy().into_owned();
        let mut fake = ExecSpec::new(&self.binary).args(["--dialect", self.dialect, "--scenario", scenario.as_str()]);
        fake.cwd = spec.cwd.clone();

        let process = TokioSpawner.spawn(&fake)?;

        if let Some(pid) = process.child.id() {
            self.pids.lock().unwrap_or_else(PoisonError::into_inner).push(pid);
        }

        Ok(process)
    }
}

/// How long a test waits for the fake agent.
pub const WAIT: Duration = Duration::from_secs(10);

/// Polls `condition` (sessions run on the async runtime's worker threads).
pub fn wait_until(what: &str, condition: impl Fn() -> bool) {
    let deadline = Instant::now() + WAIT;

    while !condition() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// App state with agent sessions that play a scenario stream.
pub struct AgentSetup {
    pub state: AppState,
    pub spawner: Arc<ScenarioSpawner>,
    pub project: PathBuf,
    _dir: TempDir,
}

/// An app state with an opened project, OMP detected at `binary` and sessions that play the
/// `scenario` stream.
pub fn agent_setup(binary: &Path, scenario: &str) -> AgentSetup {
    let dir = TempDir::new("agent");
    let home = dir.path().join("home");
    let project = home.join("demo");
    std::fs::create_dir_all(project.join("src")).expect("create project");

    let mut state = AppState::new(dir.path().join("config"), Env::for_tests(&home));
    state.prefs.with(|store| {
        Projects::new(store, &state.env).open(&project).expect("open project");
    });

    let spawner = ScenarioSpawner::new(binary, scenario);
    let options = SessionOptions {
        coalesce_window: Duration::ZERO,
        ..SessionOptions::default()
    };
    state.agents = Arc::new(Agents::with(AgentManager::with_options(options), spawner.clone()));

    cache_detection(&state, HarnessId::Omp, Some(binary));

    AgentSetup {
        state,
        spawner,
        project,
        _dir: dir,
    }
}

/// A request for a new OMP chat in `project`.
pub fn start_request(project: &Path) -> AgentStartRequest {
    AgentStartRequest {
        harness: HarnessId::Omp,
        project: project.to_string_lossy().into_owned(),
        resume: None,
        approval_override: None,
        trust_once: false,
        utility: false,
    }
}

/// The id of the run request [`write_script_request`] writes in tests.
pub const REQUEST_ID: &str = "0123456789abcdef0123456789abcdef";

/// A temp folder with a home and a Harness Memory data folder (`HARNESS_MEMORY_DIR`, not created).
pub fn memory_data_setup(label: &str) -> (TempDir, Env, PathBuf) {
    let dir = TempDir::new(label);
    let home = dir.path().join("home");
    let data = dir.path().join("memory-data");
    let env = Env::for_tests(&home).with_var("HARNESS_MEMORY_DIR", data.to_string_lossy());

    (dir, env, data)
}

/// Writes run request `id` for script `deploy` into `<data>/requests` (created `0700`), as the
/// server does.
pub fn write_script_request(data: &Path, id: &str) {
    let requests = data.join(harness_business::ecosystem::memory_requests::REQUESTS_DIR);
    std::fs::create_dir_all(&requests).expect("create requests");
    set_mode(&requests, 0o700);

    let request = serde_json::json!({
        "version": 1, "requestId": id, "createdAt": "2026-10-17T10:00:00Z", "client": "codex",
        "script": {
            "id": 3, "name": "deploy", "description": "", "language": "sh", "scope": "global",
            "repo": null, "sha256": "00", "timeoutSecs": 60, "place": "team",
            "author": { "source": "codex", "root": null }, "createdAt": "x", "updatedAt": "x",
            "runCount": 0, "lastRunAt": null, "lastExitCode": null, "trusted": false, "approval": "needed"
        },
        "body": "echo hi\n", "language": "sh", "args": [], "previous": null
    });
    std::fs::write(requests.join(format!("{id}.json")), request.to_string()).expect("write request");
}
