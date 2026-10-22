//! Ecosystem detection over temporary homes: the dashboard and memory binaries and their versions,
//! the skills and the sidebar as OMP plugins, and the per-OS catalog.

use std::path::Path;
use std::sync::Mutex;

use harness_business::Result;
use harness_business::ecosystem::{ECOSYSTEM, catalog_item, ecosystem_status};
use harness_business::env::{Env, OsKind};
use harness_business::model::{EcosystemId, EcosystemItem, EcosystemStatus};
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ProcessRunner};
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

/// Answers each command through `answer` and keeps the argument lists it saw.
struct ScriptedRunner {
    answer: fn(&ExecSpec) -> ExecOutput,
    args: Mutex<Vec<Vec<String>>>,
}

impl ScriptedRunner {
    fn new(answer: fn(&ExecSpec) -> ExecOutput) -> Self {
        Self {
            answer,
            args: Mutex::new(Vec::new()),
        }
    }
}

impl ProcessRunner for ScriptedRunner {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        let args = spec.args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect();
        self.args.lock().unwrap().push(args);

        let output = (self.answer)(spec);
        Box::pin(async move { Ok(output) })
    }
}

/// `harness-memory --version` and `harness-dashboard version` output.
fn prints_version(spec: &ExecSpec) -> ExecOutput {
    let memory = spec.args.first().is_some_and(|arg| arg == "--version");

    ExecOutput {
        status: Some(0),
        stdout: if memory { "harness-memory 1.0.0\n" } else { "1.22.1\n" }.to_owned(),
        ..ExecOutput::default()
    }
}

fn fails(_spec: &ExecSpec) -> ExecOutput {
    ExecOutput {
        status: Some(1),
        ..ExecOutput::default()
    }
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn install_dashboard(home: &Path) {
    install_binary(home, "harness-dashboard");
}

/// Places an executable `name` in `~/.local/bin`.
fn install_binary(home: &Path, name: &str) {
    let binary = home.join(".local/bin").join(name);
    write(&binary, "");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

/// Registers marketplace installs `(id, version, enabled)` in `~/.omp/plugins`.
fn install_plugins(home: &Path, plugins: &[(&str, &str, bool)]) {
    let mut registry = serde_json::Map::new();
    let mut lock = serde_json::Map::new();

    for (id, version, enabled) in plugins {
        let name = id.split('@').next().unwrap();
        let dir = format!("~/.omp/plugins/cache/plugins/{name}");
        write(
            &home.join(format!(".omp/plugins/cache/plugins/{name}/package.json")),
            &json!({ "name": name, "version": version }).to_string(),
        );

        registry.insert(
            (*id).to_owned(),
            json!([{ "scope": "user", "installPath": dir, "version": version }]),
        );
        lock.insert(name.to_owned(), json!({ "version": version, "enabled": enabled }));
    }

    write(
        &home.join(".omp/plugins/installed_plugins.json"),
        &json!({ "version": 2, "plugins": registry }).to_string(),
    );
    write(
        &home.join(".omp/plugins/omp-plugins.lock.json"),
        &json!({ "plugins": lock }).to_string(),
    );
}

async fn status(home: &TempDir, os: OsKind, runner: &ScriptedRunner) -> EcosystemStatus {
    ecosystem_status(&Env::for_tests(home.path()).with_os(os), runner).await
}

fn item(status: &EcosystemStatus, id: EcosystemId) -> &EcosystemItem {
    status.items.iter().find(|item| item.id == id).expect("item")
}

#[tokio::test]
async fn installed_tools_report_their_version_and_where_they_were_found() {
    let home = tempfile::tempdir().unwrap();
    install_dashboard(home.path());
    install_plugins(
        home.path(),
        &[("harness-useful-skills@harness-useful-skills", "0.2.36", true)],
    );
    let runner = ScriptedRunner::new(prints_version);

    let status = status(&home, OsKind::Linux, &runner).await;

    let ids: Vec<EcosystemId> = status.items.iter().map(|item| item.id).collect();
    assert_eq!(
        ids,
        [
            EcosystemId::Dashboard,
            EcosystemId::Memory,
            EcosystemId::Skills,
            EcosystemId::Sidebar
        ]
    );

    let dashboard = item(&status, EcosystemId::Dashboard);
    assert!(dashboard.installed);
    assert_eq!(dashboard.version.as_deref(), Some("1.22.1"));
    assert_eq!(dashboard.via, ["binary ~/.local/bin/harness-dashboard"]);
    assert_eq!(*runner.args.lock().unwrap(), [vec!["version".to_owned()]]);

    let memory = item(&status, EcosystemId::Memory);
    assert!(!memory.installed);
    assert!(memory.via.is_empty());

    let skills = item(&status, EcosystemId::Skills);
    assert!(skills.installed);
    assert_eq!(skills.version.as_deref(), Some("0.2.36"));
    assert_eq!(
        skills.via,
        ["OMP plugin harness-useful-skills@harness-useful-skills v0.2.36"]
    );

    let sidebar = item(&status, EcosystemId::Sidebar);
    assert!(!sidebar.installed);
    assert_eq!(sidebar.version, None);
    assert!(sidebar.via.is_empty());
    assert!(sidebar.supported_on_this_os);
    assert_eq!(sidebar.install_steps.len(), 3);
}

#[tokio::test]
async fn an_empty_home_has_nothing_installed_and_runs_nothing() {
    let home = tempfile::tempdir().unwrap();
    let runner = ScriptedRunner::new(prints_version);

    let status = status(&home, OsKind::Linux, &runner).await;

    for item in &status.items {
        assert!(!item.installed, "{:?}", item.id);
        assert!(!item.install_steps.is_empty(), "{:?}", item.id);
    }
    assert!(runner.args.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_disabled_plugin_still_counts_and_says_so() {
    let home = tempfile::tempdir().unwrap();
    install_plugins(
        home.path(),
        &[("harness-useful-sidebar@harness-useful-sidebar", "1.0.71", false)],
    );

    let status = status(&home, OsKind::Linux, &ScriptedRunner::new(prints_version)).await;

    let sidebar = item(&status, EcosystemId::Sidebar);
    assert!(sidebar.installed);
    assert_eq!(
        sidebar.via,
        ["OMP plugin harness-useful-sidebar@harness-useful-sidebar v1.0.71 (disabled)"]
    );
}

#[tokio::test]
async fn the_memory_server_reports_the_version_of_its_dash_dash_version() {
    let home = tempfile::tempdir().unwrap();
    install_binary(home.path(), "harness-memory");
    let runner = ScriptedRunner::new(prints_version);

    let status = status(&home, OsKind::Linux, &runner).await;

    let memory = item(&status, EcosystemId::Memory);
    assert!(memory.installed);
    assert_eq!(memory.version.as_deref(), Some("1.0.0"));
    assert_eq!(memory.via, ["binary ~/.local/bin/harness-memory"]);
    assert_eq!(*runner.args.lock().unwrap(), [vec!["--version".to_owned()]]);
    assert!(!item(&status, EcosystemId::Dashboard).installed);
}

#[tokio::test]
async fn a_memory_server_without_a_version_is_installed_with_a_note() {
    let home = tempfile::tempdir().unwrap();
    install_binary(home.path(), "harness-memory");

    let status = status(&home, OsKind::Linux, &ScriptedRunner::new(fails)).await;

    let memory = item(&status, EcosystemId::Memory);
    assert!(memory.installed);
    assert_eq!(memory.version, None);
    assert!(
        memory
            .notes
            .last()
            .unwrap()
            .contains("--version` did not report a version")
    );
}

#[tokio::test]
async fn a_dashboard_without_a_version_is_installed_with_a_note() {
    let home = tempfile::tempdir().unwrap();
    install_dashboard(home.path());

    let status = status(&home, OsKind::Linux, &ScriptedRunner::new(fails)).await;

    let dashboard = item(&status, EcosystemId::Dashboard);
    assert!(dashboard.installed);
    assert_eq!(dashboard.version, None);
    assert!(dashboard.notes.last().unwrap().contains("did not report a version"));
}

#[test]
fn the_sidebar_is_unsupported_on_windows_and_the_binaries_install_with_powershell() {
    let sidebar = catalog_item(EcosystemId::Sidebar, OsKind::Windows);
    assert!(!sidebar.supported_on_this_os);
    assert!(sidebar.install_steps.is_empty());

    let dashboard = catalog_item(EcosystemId::Dashboard, OsKind::Windows);
    assert!(dashboard.install_steps[0].command.contains("install.ps1"));
    assert!(
        catalog_item(EcosystemId::Dashboard, OsKind::MacOs).install_steps[0]
            .command
            .contains("install.sh")
    );

    let memory = catalog_item(EcosystemId::Memory, OsKind::Windows);
    assert!(memory.supported_on_this_os);
    assert!(
        memory.install_steps[0]
            .command
            .contains("harness-useful-memory/main/install.ps1")
    );
    assert!(
        catalog_item(EcosystemId::Memory, OsKind::Linux).install_steps[0]
            .command
            .contains("harness-useful-memory/main/install.sh")
    );

    for id in ECOSYSTEM {
        let item = catalog_item(id, OsKind::Linux);
        assert!(
            item.repo_url
                .starts_with("https://github.com/MSpiechowicz/harness-useful-")
        );
        assert_eq!(item.docs_url, format!("{}#readme", item.repo_url));
    }
}
