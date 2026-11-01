//! What the "Recommended setup" step and the Ecosystem tab say about Harness Knowledge: the binary
//! (missing, found without a version, installed) with its fixed installer per OS, its MCP entry per
//! harness beside the memory server's, and the catalog entry.

use crate::knowledge_stand_in;
use crate::team_support;

use harness_business::ecosystem::catalog::{KNOWLEDGE_INSTALL_PS1, KNOWLEDGE_INSTALL_SH};
use harness_business::ecosystem::{Companion, ECOSYSTEM, catalog_item, ecosystem_status, installer, setup_status};
use harness_business::env::{Env, OsKind};
use harness_business::harness::HarnessRegistry;
use harness_business::model::{EcosystemId, McpState};
use knowledge_stand_in::{ALL_FEATURES, cli, installed};
use pretty_assertions::assert_eq;
use team_support::{Cli, exits};

fn banner(version_line: &'static str) -> Cli {
    Cli::default().starting(&["--version"], exits(0, version_line, ""))
}

async fn status_in(env: &Env, runner: &Cli) -> harness_business::model::RecommendedSetup {
    setup_status(env, runner, &HarnessRegistry::with_defaults(), &[]).await
}

#[tokio::test]
async fn the_setup_step_offers_the_installer_until_the_server_reports_its_version() {
    let dir = tempfile::tempdir().unwrap();
    let missing = status_in(&Env::for_tests(dir.path()).with_os(OsKind::Linux), &cli(ALL_FEATURES)).await;
    assert!(!missing.knowledge.installed && missing.knowledge.version.is_none());
    assert_eq!(missing.knowledge.command, KNOWLEDGE_INSTALL_SH);
    assert!(
        missing
            .knowledge
            .side_effects
            .iter()
            .any(|line| line.contains("harness-knowledge"))
    );
    assert!(missing.knowledge.side_effects.iter().all(|line| !line.contains(';')));
    assert!(!missing.memory.installed, "the memory server is a separate row");

    let (_dir, env) = installed();
    let silent = status_in(&env, &Cli::default()).await;
    assert!(
        !silent.knowledge.installed,
        "a server without a version is not installed"
    );

    let wrong = status_in(&env, &banner("harness-memory 1.0.0\n")).await;
    assert!(!wrong.knowledge.installed);

    let ready = status_in(&env, &cli(ALL_FEATURES)).await;
    assert!(ready.knowledge.installed);
    assert_eq!(ready.knowledge.version.as_deref(), Some("1.0.0"));
    assert!(ready.knowledge.supported_on_this_os);
}

#[tokio::test]
async fn every_harness_reports_the_knowledge_entry_apart_from_the_memory_one() {
    let (_dir, env) = installed();
    let setup = status_in(&env, &cli(ALL_FEATURES)).await;

    assert_eq!(setup.harnesses.len(), HarnessRegistry::with_defaults().all().count());

    for harness in &setup.harnesses {
        assert!(
            matches!(harness.knowledge_mcp, McpState::Missing { .. }),
            "{:?}",
            harness.harness
        );
        assert!(
            matches!(harness.memory_mcp, McpState::Missing { .. }),
            "{:?}",
            harness.harness
        );
    }

    // A knowledge entry in OMP's file shows for knowledge only.
    let omp = env.home.join(".omp/agent/mcp.json");
    std::fs::create_dir_all(omp.parent().unwrap()).unwrap();
    std::fs::write(
        &omp,
        r#"{"mcpServers":{"harness-knowledge":{"command":"/x/harness-knowledge","args":["mcp"]}}}"#,
    )
    .unwrap();

    let setup = status_in(&env, &cli(ALL_FEATURES)).await;
    let omp = setup
        .harnesses
        .iter()
        .find(|harness| harness.harness == harness_business::model::HarnessId::Omp)
        .unwrap();
    assert_eq!(
        omp.knowledge_mcp,
        McpState::Configured {
            file: "~/.omp/agent/mcp.json".to_owned(),
            name: "harness-knowledge".to_owned()
        }
    );
    assert!(matches!(omp.memory_mcp, McpState::Missing { .. }));
}

#[test]
fn the_installer_is_fixed_per_os_and_held_to_https() {
    let home = std::path::Path::new("/home/u");

    for os in [OsKind::Linux, OsKind::MacOs] {
        let spec = installer(&Env::for_tests(home).with_os(os), Companion::Knowledge);
        assert_eq!(spec.command, KNOWLEDGE_INSTALL_SH);
        assert!(spec.args[1].starts_with("curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/MSpiechowicz/harness-useful-knowledge/main/install.sh"));
    }

    let windows = installer(&Env::for_tests(home).with_os(OsKind::Windows), Companion::Knowledge);
    assert_eq!(windows.command, KNOWLEDGE_INSTALL_PS1);
    assert!(
        windows
            .args
            .last()
            .unwrap()
            .contains("harness-useful-knowledge/main/install.ps1")
    );
    assert_eq!(Companion::Knowledge.name(), "harness-knowledge");
}

#[tokio::test]
async fn the_ecosystem_tab_lists_it_after_memory_with_its_version() {
    assert_eq!(
        &ECOSYSTEM[..3],
        [EcosystemId::Dashboard, EcosystemId::Memory, EcosystemId::Knowledge]
    );

    let item = catalog_item(EcosystemId::Knowledge, OsKind::Linux);
    assert_eq!(item.name, "Harness Knowledge");
    assert_eq!(
        item.repo_url,
        "https://github.com/MSpiechowicz/harness-useful-knowledge"
    );
    assert_eq!(
        item.docs_url,
        "https://github.com/MSpiechowicz/harness-useful-knowledge#readme"
    );
    assert!(
        item.notes
            .iter()
            .chain([&item.tagline, &item.description])
            .all(|text| !text.contains(';'))
    );

    let (_dir, env) = installed();
    // The install folder and its `via` text are the Linux and macOS ones.
    let env = env.with_os(OsKind::Linux);
    let status = ecosystem_status(&env, &cli(ALL_FEATURES)).await;
    let found = status
        .items
        .iter()
        .find(|item| item.id == EcosystemId::Knowledge)
        .unwrap();
    assert!(found.installed);
    assert_eq!(found.version.as_deref(), Some("1.0.0"));
    // Windows installs under %LOCALAPPDATA%\\Programs: the `via` text is the Linux and macOS one.
    if !cfg!(windows) {
        assert_eq!(found.via, ["binary ~/.local/bin/harness-knowledge"]);
    }

    let memory = status.items.iter().find(|item| item.id == EcosystemId::Memory).unwrap();
    assert!(!memory.installed);
}
