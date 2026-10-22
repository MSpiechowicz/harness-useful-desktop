//! A temporary machine for the "Share setup" import tests: harness files, summaries, binaries, and
//! a fake harness CLI ([`FakeCli`]) that records every command line and performs what it asks for on
//! the files.
//! The fake `opencode debug config` (and OpenCode 2's `opencode api --standalone GET /api/config`)
//! answers what OpenCode merges (see [`Probe`]).
//!
//! Each helper is used by some tests only, hence the `dead_code` allowance.
#![allow(dead_code)]

#[path = "fake_cli.rs"]
mod fake_cli;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use harness_business::env::Env;
use harness_business::harness::HarnessRegistry;
use harness_business::model::{
    HarnessId, SyncImportItem, SyncImportPlan, SyncImportResult, SyncItemOutcome, SyncItemStatus, SyncOutcome,
    SyncProgress,
};
use harness_business::process::{ExecOutput, ResolvedBinary};
use harness_business::sync::codec::decode_input;
use harness_business::sync::{ApplyContext, BuiltPlan, StoredPlan, apply_plan, build_plan};
use harness_business::{BusinessError, Result};
use serde_json::{Value, json};
use tempfile::TempDir;

pub use crate::sync_support::{BOTH, summaries};
pub use fake_cli::FakeCli;

pub const OMP_MCP: &str = ".omp/agent/mcp.json";
pub const PI_MCP: &str = ".pi/agent/mcp.json";
pub const OMP_MARKETPLACES: &str = ".omp/marketplaces.json";
pub const OMP_PLUGINS: &str = ".omp/plugins/installed_plugins.json";
pub const PI_SETTINGS: &str = ".pi/agent/settings.json";
pub const OPENCODE_JSON: &str = ".config/opencode/opencode.json";
pub const OPENCODE_JSONC: &str = ".config/opencode/opencode.jsonc";
pub const CLAUDE_GLOBAL: &str = ".claude.json";
pub const CLAUDE_MARKETPLACES: &str = ".claude/plugins/known_marketplaces.json";
pub const CLAUDE_PLUGINS: &str = ".claude/plugins/installed_plugins.json";

/// What the fake `opencode debug config` answers.
#[derive(Debug, Clone)]
pub enum Probe {
    /// A config whose `mcp` holds the servers of the OpenCode config files here (those that parse
    /// as plain JSON) plus these names, from layers the app cannot read (some values masked).
    Merges(Vec<String>),
    /// This output instead.
    Prints(ExecOutput),
}

/// A home directory with harness files, and the harnesses detected as installed.
pub struct Machine {
    dir: TempDir,
    pub env: Env,
    pub installed: Vec<HarnessId>,
    probe: Arc<Mutex<Probe>>,
}

impl Machine {
    pub fn new(installed: &[HarnessId]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let env = Env::for_tests(dir.path());

        Self {
            dir,
            env,
            installed: installed.to_vec(),
            probe: Arc::new(Mutex::new(Probe::Merges(Vec::new()))),
        }
    }

    /// OpenCode merges servers named `names` from a layer the app cannot read.
    pub fn opencode_merges(&self, names: &[&str]) {
        let names = names.iter().map(|name| (*name).to_owned()).collect();
        *self.probe.lock().unwrap() = Probe::Merges(names);
    }

    /// `opencode debug config` prints `output`.
    pub fn opencode_prints(&self, output: ExecOutput) {
        *self.probe.lock().unwrap() = Probe::Prints(output);
    }

    pub fn home(&self) -> &Path {
        self.dir.path()
    }

    pub fn write(&self, relative: &str, value: &Value) {
        write_json(&self.home().join(relative), value);
    }

    /// The file's text, `None` when it does not exist.
    pub fn text(&self, relative: &str) -> Option<String> {
        std::fs::read_to_string(self.home().join(relative)).ok()
    }

    pub fn json(&self, relative: &str) -> Value {
        serde_json::from_str(&self.text(relative).expect("file exists")).unwrap()
    }

    /// The review of `bundle` (share file JSON) on this machine. It reads files only: there is
    /// no runner to run a CLI with.
    pub fn plan(&self, bundle: &Value) -> BuiltPlan {
        self.plan_in(&HarnessRegistry::with_defaults(), bundle)
    }

    /// [`Machine::plan`] with the harnesses of `registry` (an OpenCode 2 one, say).
    pub fn plan_in(&self, registry: &HarnessRegistry, bundle: &Value) -> BuiltPlan {
        let decoded = decode_input(&bundle.to_string()).expect("a valid bundle");

        build_plan(&self.env, registry, &summaries(&self.installed), &decoded)
    }

    /// The binaries of the installed harnesses (never started: the fake CLI answers instead).
    pub fn binaries(&self) -> BTreeMap<HarnessId, ResolvedBinary> {
        self.installed
            .iter()
            .map(|harness| {
                let binary = ResolvedBinary {
                    path: PathBuf::from(format!("/fake/bin/{}", harness.binary_name())),
                    is_batch_script: false,
                };
                (*harness, binary)
            })
            .collect()
    }
}

pub fn write_json(path: &Path, value: &Value) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
}

/// A share file holding `harnesses`.
pub fn bundle(harnesses: Value) -> Value {
    json!({
        "format": "harness-useful-desktop/setup",
        "version": 1,
        "app": "0.15.0",
        "createdAt": "2026-10-15",
        "harnesses": harnesses,
    })
}

/// Every item id of `plan`, in order.
pub fn ids(plan: &SyncImportPlan) -> Vec<String> {
    plan.sections
        .iter()
        .flat_map(|section| section.items.iter().map(|item| item.id.clone()))
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Fixtures and plan helpers
// ---------------------------------------------------------------------------------------------

pub const MCP_SECRET: &str = "[REDACTED]";

/// OMP: a marketplace, a plugin from it, an npm plugin, a stdio server needing a token and an HTTP
/// server; PI: a pinned package and an HTTP server.
pub fn typical() -> Value {
    bundle(json!({
        "omp": {
            "marketplaces": [{ "name": "useful", "source": "acme/useful-skills" }],
            "plugins": [
                { "id": "harness-useful-skills@useful", "marketplace": "useful", "version": "0.2.36" },
                { "id": "npm-helper" },
            ],
            "mcpServers": [
                { "name": "github", "transport": "stdio", "command": "npx",
                  "args": ["-y", "@modelcontextprotocol/server-github"], "envKeys": ["GITHUB_TOKEN"] },
                { "name": "remote", "transport": "http", "url": "https://mcp.example.com/mcp" },
            ],
        },
        "pi": {
            "packages": [{ "source": "npm:@demo/pi-review-kit@1.0.0" }],
            "mcpServers": [{ "name": "docs", "transport": "http", "url": "https://docs.example.com/mcp" }],
        },
    }))
}

pub fn fake(machine: &Machine) -> FakeCli {
    FakeCli::new(machine).naming("acme/useful-skills", "useful")
}

pub fn argv(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

pub fn item<'p>(plan: &'p SyncImportPlan, id: &str) -> &'p SyncImportItem {
    plan.sections
        .iter()
        .flat_map(|section| &section.items)
        .find(|item| item.id == id)
        .unwrap_or_else(|| panic!("no item {id}"))
}

pub fn statuses(plan: &SyncImportPlan) -> Vec<(String, SyncItemStatus)> {
    plan.sections
        .iter()
        .flat_map(|section| section.items.iter().map(|item| (item.id.clone(), item.status.clone())))
        .collect()
}

pub fn selectable(plan: &SyncImportPlan) -> Vec<String> {
    plan.sections
        .iter()
        .flat_map(|section| {
            section
                .items
                .iter()
                .filter(|item| item.selectable)
                .map(|item| item.id.clone())
        })
        .collect()
}

/// Applies `selected` of `stored` with the fake CLI, returning the progress events too.
pub async fn apply(
    machine: &Machine,
    runner: &FakeCli,
    stored: &StoredPlan,
    selected: &[String],
    confirmed: bool,
) -> (Result<SyncImportResult>, Vec<SyncProgress>) {
    apply_in(
        machine,
        runner,
        &HarnessRegistry::with_defaults(),
        stored,
        selected,
        confirmed,
    )
    .await
}

/// [`apply`] with the harnesses of `registry`.
pub async fn apply_in(
    machine: &Machine,
    runner: &FakeCli,
    registry: &HarnessRegistry,
    stored: &StoredPlan,
    selected: &[String],
    confirmed: bool,
) -> (Result<SyncImportResult>, Vec<SyncProgress>) {
    let binaries = machine.binaries();
    let context = ApplyContext {
        env: &machine.env,
        registry,
        runner,
        binaries: &binaries,
    };

    let mut events = Vec::new();
    let result = apply_plan(context, stored, selected, confirmed, |event| events.push(event)).await;

    (result, events)
}

/// Applies `selected` (confirmed), checks that every item is done, and returns the result and the
/// progress events.
pub async fn apply_done(
    machine: &Machine,
    runner: &FakeCli,
    stored: &StoredPlan,
    selected: &[String],
) -> (SyncImportResult, Vec<SyncProgress>) {
    let (result, events) = apply(machine, runner, stored, selected, true).await;
    let result = result.expect("applied");

    assert!(result.ok, "{result:?}");
    assert!(result.items.iter().all(|item| item.outcome == SyncOutcome::Done));

    (result, events)
}

pub fn outcome_of<'r>(result: &'r SyncImportResult, id: &str) -> &'r SyncItemOutcome {
    result
        .items
        .iter()
        .find(|item| item.id == id)
        .unwrap_or_else(|| panic!("no outcome for {id}"))
}

pub fn assert_refused(result: Result<SyncImportResult>) {
    assert!(
        matches!(result, Err(BusinessError::InvalidInput(_))),
        "expected invalidInput, got {result:?}"
    );
}
