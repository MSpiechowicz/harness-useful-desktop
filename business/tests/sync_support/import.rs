//! A temporary machine for the "Share setup" import tests: harness files, summaries, binaries, and
//! a fake harness CLI that records every command line and performs what it asks for on the files.
//! The fake `opencode debug config` answers what OpenCode merges (see [`Probe`]).
//!
//! Each helper is used by some tests only, hence the `dead_code` allowance.
#![allow(dead_code)]

#[path = "harnesses.rs"]
mod harnesses;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use harness_business::env::Env;
use harness_business::harness::HarnessRegistry;
use harness_business::model::{
    HarnessId, SyncImportItem, SyncImportPlan, SyncImportResult, SyncItemOutcome, SyncItemStatus, SyncProgress,
};
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ProcessRunner, ResolvedBinary};
use harness_business::sync::codec::decode_input;
use harness_business::sync::{ApplyContext, BuiltPlan, StoredPlan, apply_plan, build_plan};
use harness_business::{BusinessError, Result};
use serde_json::{Value, json};
use tempfile::TempDir;

pub use harnesses::{BOTH, summaries};

pub const OMP_MCP: &str = ".omp/agent/mcp.json";
pub const PI_MCP: &str = ".pi/agent/mcp.json";
pub const OMP_MARKETPLACES: &str = ".omp/marketplaces.json";
pub const OMP_PLUGINS: &str = ".omp/plugins/installed_plugins.json";
pub const PI_SETTINGS: &str = ".pi/agent/settings.json";
pub const OPENCODE_JSON: &str = ".config/opencode/opencode.json";
pub const OPENCODE_JSONC: &str = ".config/opencode/opencode.jsonc";

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
        let decoded = decode_input(&bundle.to_string()).expect("a valid bundle");

        build_plan(
            &self.env,
            &HarnessRegistry::with_defaults(),
            &summaries(&self.installed),
            &decoded,
        )
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

fn read_json(path: &Path) -> Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| json!({}))
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

/// The fake `omp`, `pi` and `opencode`: records each command line (program file name first) and,
/// unless told to fail it, does what it asks to the files of `home`: registers a marketplace (named
/// after `marketplace_names`), installs a user plugin, adds a PI package, adds an OpenCode plugin to
/// `opencode.json`.
///
/// `opencode debug config` answers the machine's [`Probe`]; it is recorded in
/// [`FakeCli::probes`], not in [`FakeCli::seen`].
pub struct FakeCli {
    home: PathBuf,
    marketplace_names: Vec<(String, String)>,
    failing: Vec<Vec<String>>,
    seen: Mutex<Vec<Vec<String>>>,
    probe: Arc<Mutex<Probe>>,
    probes: Mutex<Vec<ExecSpec>>,
}

impl FakeCli {
    pub fn new(machine: &Machine) -> Self {
        Self {
            home: machine.home().to_path_buf(),
            marketplace_names: Vec::new(),
            failing: Vec::new(),
            seen: Mutex::new(Vec::new()),
            probe: machine.probe.clone(),
            probes: Mutex::new(Vec::new()),
        }
    }

    /// The `opencode debug config` commands run.
    pub fn probes(&self) -> Vec<ExecSpec> {
        self.probes.lock().unwrap().clone()
    }

    /// The answer of `opencode debug config`.
    fn merged_config(&self) -> ExecOutput {
        let names = match &*self.probe.lock().unwrap() {
            Probe::Prints(output) => return output.clone(),
            Probe::Merges(names) => names.clone(),
        };

        let mut servers = serde_json::Map::new();
        for file in [OPENCODE_JSON, OPENCODE_JSONC, ".config/opencode/config.json"] {
            if let Some(found) = read_json(&self.home.join(file))["mcp"].as_object() {
                servers.extend(found.clone());
            }
        }
        for name in names {
            let org = json!({ "type": "remote", "url": "https://org.example.com/mcp",
                              "headers": { "Authorization": "***" } });
            servers.insert(name, org);
        }

        let config = json!({ "mcp": servers, "$schema": "https://opencode.ai/config.json", "plugin": [] });

        ExecOutput {
            status: Some(0),
            stdout: serde_json::to_string_pretty(&config).unwrap(),
            ..ExecOutput::default()
        }
    }

    /// `marketplace add <source>` registers the marketplace as `name`.
    pub fn naming(mut self, source: &str, name: &str) -> Self {
        self.marketplace_names.push((source.to_owned(), name.to_owned()));
        self
    }

    /// `argv` exits with 1 and an OMP-style failure line, changing nothing.
    pub fn failing(mut self, argv: &[&str]) -> Self {
        self.failing.push(argv.iter().map(|arg| (*arg).to_owned()).collect());
        self
    }

    pub fn seen(&self) -> Vec<Vec<String>> {
        self.seen.lock().unwrap().clone()
    }

    fn perform(&self, argv: &[String]) {
        let args: Vec<&str> = argv.iter().map(String::as_str).collect();

        match args.as_slice() {
            ["omp", "plugin", "marketplace", "add", source] => {
                let name = self
                    .marketplace_names
                    .iter()
                    .find(|(known, _)| known == source)
                    .map_or(*source, |(_, name)| name.as_str());
                let path = self.home.join(OMP_MARKETPLACES);
                let mut registry = read_json(&path);
                let entry = json!({ "name": name, "sourceType": "github", "sourceUri": source });

                match registry["marketplaces"].as_array_mut() {
                    Some(entries) => entries.push(entry),
                    None => registry = json!({ "version": 1, "marketplaces": [entry] }),
                }
                write_json(&path, &registry);
            }
            ["omp", "plugin", "install", id, "--scope", "user"] => {
                let path = self.home.join(OMP_PLUGINS);
                let mut registry = read_json(&path);
                if registry.get("plugins").is_none() {
                    registry = json!({ "version": 2, "plugins": {} });
                }
                registry["plugins"][*id] = json!([{ "scope": "user" }]);
                write_json(&path, &registry);
            }
            ["pi", "install", source] => {
                let path = self.home.join(PI_SETTINGS);
                let mut settings = read_json(&path);
                match settings["packages"].as_array_mut() {
                    Some(packages) => packages.push(json!(source)),
                    None => settings["packages"] = json!([source]),
                }
                write_json(&path, &settings);
            }
            ["opencode", "plugin", spec, "-g"] => {
                let path = self.home.join(OPENCODE_JSON);
                let mut config = read_json(&path);
                match config["plugin"].as_array_mut() {
                    Some(plugins) => plugins.push(json!(spec)),
                    None => config["plugin"] = json!([spec]),
                }
                write_json(&path, &config);
            }
            _ => panic!("unexpected command {argv:?}"),
        }
    }
}

impl ProcessRunner for FakeCli {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        let program = spec.program.file_name().unwrap().to_string_lossy().into_owned();
        let mut argv = vec![program];
        argv.extend(spec.args.iter().map(|arg| arg.to_string_lossy().into_owned()));

        if argv == ["opencode", "debug", "config"] {
            self.probes.lock().unwrap().push(spec.clone());
            let output = self.merged_config();

            return Box::pin(async move { Ok(output) });
        }

        self.seen.lock().unwrap().push(argv.clone());

        let output = if self.failing.contains(&argv) {
            ExecOutput {
                status: Some(1),
                stderr: "✘ Failed to add marketplace: Error: not found".to_owned(),
                ..ExecOutput::default()
            }
        } else {
            self.perform(&argv);
            ExecOutput {
                status: Some(0),
                stdout: "Done.".to_owned(),
                ..ExecOutput::default()
            }
        };

        Box::pin(async move { Ok(output) })
    }
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
    let registry = HarnessRegistry::with_defaults();
    let binaries = machine.binaries();
    let context = ApplyContext {
        env: &machine.env,
        registry: &registry,
        runner,
        binaries: &binaries,
    };

    let mut events = Vec::new();
    let result = apply_plan(context, stored, selected, confirmed, |event| events.push(event)).await;

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
