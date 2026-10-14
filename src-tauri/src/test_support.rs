//! Test helpers for the shell's unit tests.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use harness_business::env::Env;
use harness_business::model::{Detection, HarnessId, HarnessSummary, VersionSupport};
use harness_business::process::{ExecSpec, ProcessSpawner, SpawnedProcess, TokioSpawner};
use harness_business::project::Projects;
use serde_json::Value;
use tauri::async_runtime::block_on;
use tauri::ipc::{Channel, InvokeResponseBody};

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

/// A channel that keeps every message it receives, parsed as JSON.
pub fn collecting_channel<T>() -> (Channel<T>, Arc<Mutex<Vec<Value>>>) {
    let received = Arc::new(Mutex::new(Vec::new()));
    let messages = received.clone();

    let channel = Channel::new(move |body| {
        if let InvokeResponseBody::Json(json) = body {
            let message = serde_json::from_str(&json).expect("channel messages are JSON");
            messages
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(message);
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

/// Runs `fake_agent --dialect omp --scenario <scenario>` in the requested directory.
pub struct ScenarioSpawner {
    binary: PathBuf,
    scenario: PathBuf,
    spawned: AtomicUsize,
}

impl ScenarioSpawner {
    /// Plays `scenario`, a stream under `business/tests/fixtures/streams`, with the fake agent
    /// at `binary`.
    pub fn new(binary: &Path, scenario: &str) -> Arc<Self> {
        Arc::new(Self {
            binary: binary.to_path_buf(),
            scenario: Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../business/tests/fixtures/streams")
                .join(scenario),
            spawned: AtomicUsize::new(0),
        })
    }

    /// How many agents were spawned.
    pub fn spawned(&self) -> usize {
        self.spawned.load(Ordering::Relaxed)
    }
}

impl ProcessSpawner for ScenarioSpawner {
    fn spawn(&self, spec: &ExecSpec) -> harness_business::Result<SpawnedProcess> {
        self.spawned.fetch_add(1, Ordering::Relaxed);

        let scenario = self.scenario.to_string_lossy().into_owned();
        let mut fake = ExecSpec::new(&self.binary).args(["--dialect", "omp", "--scenario", scenario.as_str()]);
        fake.cwd = spec.cwd.clone();

        TokioSpawner.spawn(&fake)
    }
}
