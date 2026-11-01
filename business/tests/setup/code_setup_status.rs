//! What the "Recommended setup" step and the Ecosystem tab say about Harness Code: the binary
//! (missing, found without a version, installed) with its fixed installer per OS, its MCP entry per
//! harness beside the others, and the catalog entry.

use crate::memory_cli_support;
use crate::team_support;

use harness_business::ecosystem::catalog::{CODE_INSTALL_PS1, CODE_INSTALL_SH};
use harness_business::ecosystem::{Companion, ECOSYSTEM, catalog_item, ecosystem_status, installer, setup_status};
use harness_business::env::{Env, OsKind};
use harness_business::harness::HarnessRegistry;
use harness_business::model::{EcosystemId, HarnessId, McpState, RecommendedSetup};
use memory_cli_support::installed_binary;
use pretty_assertions::assert_eq;
use team_support::{Cli, exits};

fn reports(version_line: &'static str) -> Cli {
    Cli::default().starting(&["--version"], exits(0, version_line, ""))
}

async fn status_in(env: &Env, runner: &Cli) -> RecommendedSetup {
    setup_status(env, runner, &HarnessRegistry::with_defaults(), &[]).await
}

#[tokio::test]
async fn the_setup_step_offers_the_installer_until_the_server_reports_its_version() {
    let empty = tempfile::tempdir().unwrap();
    let missing = status_in(&Env::for_tests(empty.path()).with_os(OsKind::Linux), &Cli::default()).await;
    assert!(!missing.code.installed && missing.code.version.is_none());
    assert_eq!(missing.code.command, CODE_INSTALL_SH);
    assert!(
        missing
            .code
            .side_effects
            .iter()
            .any(|line| line.contains("harness-code"))
    );
    assert!(missing.code.side_effects.iter().all(|line| !line.contains(';')));

    let (_dir, env) = installed_binary("harness-code");
    assert!(
        !status_in(&env, &Cli::default()).await.code.installed,
        "no version, not installed"
    );
    assert!(
        !status_in(&env, &reports("harness-knowledge 1.0.0\n"))
            .await
            .code
            .installed
    );

    let ready = status_in(&env, &reports("harness-code 1.2.3\n")).await;
    assert!(ready.code.installed && ready.code.supported_on_this_os);
    assert_eq!(ready.code.version.as_deref(), Some("1.2.3"));
    assert!(
        !ready.knowledge.installed && !ready.memory.installed,
        "each server is its own row"
    );
}

#[tokio::test]
async fn every_harness_reports_the_code_entry_apart_from_the_others() {
    let (_dir, env) = installed_binary("harness-code");
    let runner = reports("harness-code 1.0.0\n");
    let setup = status_in(&env, &runner).await;

    assert_eq!(setup.harnesses.len(), HarnessRegistry::with_defaults().all().count());
    for harness in &setup.harnesses {
        assert!(
            matches!(harness.code_mcp, McpState::Missing { .. }),
            "{:?}",
            harness.harness
        );
    }

    let omp = env.home.join(".omp/agent/mcp.json");
    std::fs::create_dir_all(omp.parent().unwrap()).unwrap();
    std::fs::write(
        &omp,
        r#"{"mcpServers":{"harness-code":{"command":"/x/harness-code","args":["mcp"]}}}"#,
    )
    .unwrap();

    let setup = status_in(&env, &runner).await;
    let found = setup
        .harnesses
        .iter()
        .find(|harness| harness.harness == HarnessId::Omp)
        .unwrap();
    assert_eq!(
        found.code_mcp,
        McpState::Configured {
            file: "~/.omp/agent/mcp.json".to_owned(),
            name: "harness-code".to_owned()
        }
    );
    assert!(matches!(found.knowledge_mcp, McpState::Missing { .. }));
    assert!(matches!(found.memory_mcp, McpState::Missing { .. }));
}

#[test]
fn the_installer_is_fixed_per_os_and_held_to_https() {
    let user = std::path::Path::new("/home/u");

    for os in [OsKind::Linux, OsKind::MacOs] {
        let unix = installer(&Env::for_tests(user).with_os(os), Companion::Code);
        assert_eq!(unix.command, CODE_INSTALL_SH);
        assert!(unix.args[1].starts_with(
            "curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/MSpiechowicz/harness-useful-code/main/install.sh"
        ));
    }

    let windows = installer(&Env::for_tests(user).with_os(OsKind::Windows), Companion::Code);
    assert_eq!(windows.command, CODE_INSTALL_PS1);
    assert!(
        windows
            .args
            .last()
            .unwrap()
            .contains("harness-useful-code/main/install.ps1")
    );
    assert_eq!(Companion::Code.name(), "harness-code");
}

#[tokio::test]
async fn the_ecosystem_tab_lists_it_after_knowledge_with_its_version() {
    assert_eq!(ECOSYSTEM[3], EcosystemId::Code);

    let item = catalog_item(EcosystemId::Code, OsKind::Linux);
    assert_eq!(item.name, "Harness Code");
    assert_eq!(item.repo_url, "https://github.com/MSpiechowicz/harness-useful-code");
    assert_eq!(
        item.docs_url,
        "https://github.com/MSpiechowicz/harness-useful-code#readme"
    );
    assert!(
        item.notes
            .iter()
            .chain([&item.tagline, &item.description])
            .all(|text| !text.contains(';'))
    );

    let (_dir, env) = installed_binary("harness-code");
    let status = ecosystem_status(&env, &reports("harness-code 1.0.0\n")).await;
    let found = status.items.iter().find(|item| item.id == EcosystemId::Code).unwrap();
    assert!(found.installed);
    assert_eq!(found.version.as_deref(), Some("1.0.0"));
    // Windows installs under %LOCALAPPDATA%\\Programs: the `via` text is the Linux and macOS one.
    if !cfg!(windows) {
        assert_eq!(found.via, ["binary ~/.local/bin/harness-code"]);
    }
}
