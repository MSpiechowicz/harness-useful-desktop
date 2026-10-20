//! What the "Recommended setup" step offers per machine: the dashboard installer, the skills per
//! harness (installed, marketplace actions, manual command) and the MCP states of the dashboard,
//! the memory server and the app's own server.

use std::path::Path;

use harness_business::Result;
use harness_business::ecosystem::catalog::{DASHBOARD_INSTALL_PS1, DASHBOARD_INSTALL_SH};
use harness_business::ecosystem::setup_status;
use harness_business::env::{Env, OsKind};
use harness_business::harness::{Harness, HarnessRegistry};
use harness_business::harnesses::{Omp, OpenCode, Pi};
use harness_business::model::{
    Detection, ExtensionAction, HarnessId, HarnessSetup, HarnessSummary, InstallScope, McpState, RecommendedSetup,
    SkillsSetup, VersionSupport,
};
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ProcessRunner};
use pretty_assertions::assert_eq;
use serde_json::json;

/// Every probe fails (no dashboard version).
struct NoVersion;

impl ProcessRunner for NoVersion {
    fn run<'a>(&'a self, _spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        Box::pin(async {
            Ok(ExecOutput {
                status: Some(1),
                ..ExecOutput::default()
            })
        })
    }
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn summary(harness: &dyn Harness, installed: bool) -> HarnessSummary {
    HarnessSummary {
        info: harness.info(),
        capabilities: harness.capabilities(),
        detection: Detection {
            installed,
            binary_path: None,
            is_batch_script: false,
            version: None,
            version_support: VersionSupport::Untested,
            config_dir: None,
            config_dir_exists: false,
            install_hint: None,
            error: None,
        },
    }
}

async fn status(env: &Env, summaries: &[HarnessSummary]) -> RecommendedSetup {
    setup_status(env, &NoVersion, &HarnessRegistry::with_defaults(), summaries).await
}

fn harness(setup: &RecommendedSetup, id: HarnessId) -> &HarnessSetup {
    setup
        .harnesses
        .iter()
        .find(|entry| entry.harness == id)
        .expect("harness")
}

fn install_skills_plugin(home: &Path) {
    write(
        &home.join(".omp/plugins/cache/plugins/harness-useful-skills/package.json"),
        &json!({ "name": "harness-useful-skills", "version": "0.2.36" }).to_string(),
    );
    write(
        &home.join(".omp/plugins/installed_plugins.json"),
        &json!({
            "version": 2,
            "plugins": {
                "harness-useful-skills@harness-useful-skills": [{
                    "scope": "user",
                    "installPath": "~/.omp/plugins/cache/plugins/harness-useful-skills",
                    "version": "0.2.36"
                }]
            }
        })
        .to_string(),
    );
}

fn register_skills_marketplace(home: &Path) {
    write(
        &home.join(".omp/marketplaces.json"),
        &json!({
            "version": 1,
            "marketplaces": [{
                "name": "harness-useful-skills",
                "sourceType": "github",
                "sourceUri": "MSpiechowicz/harness-useful-skills",
                "catalogPath": "~/.omp/plugins/cache/marketplaces/harness-useful-skills/marketplace.json"
            }]
        })
        .to_string(),
    );
}

fn plugin_install() -> ExtensionAction {
    ExtensionAction::PluginInstall {
        target: "harness-useful-skills@harness-useful-skills".to_owned(),
        scope: InstallScope::User,
    }
}

#[tokio::test]
async fn an_empty_machine_is_offered_everything() {
    let home = tempfile::tempdir().unwrap();
    let env = Env::for_tests(home.path()).with_os(OsKind::Linux);
    let summaries = [summary(&Omp, true), summary(&Pi, false)];

    let setup = status(&env, &summaries).await;

    assert!(!setup.dashboard.installed);
    assert_eq!(setup.dashboard.version, None);
    assert!(setup.dashboard.supported_on_this_os);
    assert_eq!(setup.dashboard.command, DASHBOARD_INSTALL_SH);
    assert!(
        setup
            .dashboard
            .side_effects
            .iter()
            .any(|line| line.contains("~/.local/bin"))
    );
    assert!(
        setup
            .dashboard
            .side_effects
            .iter()
            .any(|line| line.contains("applications menu"))
    );

    assert_eq!(
        setup.harnesses.iter().map(|entry| entry.harness).collect::<Vec<_>>(),
        [HarnessId::Omp, HarnessId::Pi, HarnessId::OpenCode]
    );

    let omp = harness(&setup, HarnessId::Omp);
    assert!(omp.installed);
    assert_eq!(
        omp.skills,
        SkillsSetup::Install {
            actions: vec![
                ExtensionAction::MarketplaceAdd {
                    source: "MSpiechowicz/harness-useful-skills".to_owned()
                },
                plugin_install(),
            ]
        }
    );
    assert_eq!(
        omp.mcp,
        McpState::Missing {
            file: "~/.omp/agent/mcp.json".to_owned(),
            exists: false
        }
    );

    assert_eq!(
        omp.memory_mcp,
        McpState::Missing {
            file: "~/.omp/agent/mcp.json".to_owned(),
            exists: false
        }
    );
    assert_eq!(
        omp.desktop_mcp,
        McpState::Missing {
            file: "~/.omp/agent/mcp.json".to_owned(),
            exists: false
        }
    );

    let pi = harness(&setup, HarnessId::Pi);
    assert!(!pi.installed, "a harness that was not detected is reported as missing");
    assert!(matches!(
        &pi.skills,
        SkillsSetup::Install { actions }
            if actions == &[ExtensionAction::PackageInstall { source: "git:github.com/MSpiechowicz/harness-useful-skills".to_owned() }]
    ));
    assert_eq!(
        pi.mcp,
        McpState::Missing {
            file: "~/.pi/agent/mcp.json".to_owned(),
            exists: false
        }
    );

    // OpenCode's skills offer depends on `node` and the install record: see setup_opencode_skills.rs.
    let opencode = harness(&setup, HarnessId::OpenCode);
    let opencode_missing = McpState::Missing {
        file: "~/.config/opencode/opencode.json".to_owned(),
        exists: false,
    };
    assert_eq!(opencode.mcp, opencode_missing);
    assert_eq!(opencode.memory_mcp, opencode_missing);
    assert_eq!(opencode.desktop_mcp, opencode_missing);
}

#[tokio::test]
async fn opencodes_servers_are_read_from_its_global_config() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join(".config/opencode/opencode.jsonc");
    let entries = json!({
        "$schema": "https://opencode.ai/config.json",
        "mcp": {
            "harness-dashboard": { "type": "local", "command": ["harness-dashboard", "mcp"], "enabled": true },
            "notes": { "type": "local", "command": [home.path().join(".local/bin/harness-memory"), "mcp"], "enabled": false }
        }
    });
    write(&path, &entries.to_string());

    let setup = status(&Env::for_tests(home.path()), &[summary(&OpenCode, true)]).await;
    let opencode = harness(&setup, HarnessId::OpenCode);

    assert!(opencode.installed);
    assert_eq!(
        opencode.mcp,
        McpState::Configured {
            file: "~/.config/opencode/opencode.jsonc".to_owned(),
            name: "harness-dashboard".to_owned()
        }
    );
    assert_eq!(
        opencode.memory_mcp,
        McpState::Disabled {
            file: "~/.config/opencode/opencode.jsonc".to_owned(),
            name: "notes".to_owned()
        }
    );
}

#[tokio::test]
async fn a_server_in_the_file_opencode_config_names_counts() {
    let home = tempfile::tempdir().unwrap();
    let team = home.path().join("team/opencode.json");
    let text = json!({
        "mcp": { "harness-dashboard": { "type": "local", "command": ["harness-dashboard", "mcp"] } }
    })
    .to_string();
    write(&team, &text);
    let env = Env::for_tests(home.path());

    let named = env.clone().with_var("OPENCODE_CONFIG", team.to_string_lossy());
    let setup = status(&named, &[summary(&OpenCode, true)]).await;
    assert_eq!(
        harness(&setup, HarnessId::OpenCode).mcp,
        McpState::Configured {
            file: "~/team/opencode.json".to_owned(),
            name: "harness-dashboard".to_owned()
        }
    );
    assert_eq!(std::fs::read_to_string(&team).unwrap(), text);

    // A relative path depends on OpenCode's working directory, so it is not read.
    let relative = env.with_var("OPENCODE_CONFIG", "team/opencode.json");
    let setup = status(&relative, &[summary(&OpenCode, true)]).await;
    assert!(
        matches!(harness(&setup, HarnessId::OpenCode).mcp, McpState::Missing { .. }),
        "{:?}",
        harness(&setup, HarnessId::OpenCode).mcp
    );
}

#[tokio::test]
async fn an_opencode_config_with_comments_asks_for_a_manual_edit() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join(".config/opencode/opencode.jsonc");
    let text = "{\n  // keep this\n  \"$schema\": \"https://opencode.ai/config.json\"\n}\n";
    write(&path, text);

    let setup = status(&Env::for_tests(home.path()), &[]).await;
    let opencode = harness(&setup, HarnessId::OpenCode);

    for state in [&opencode.mcp, &opencode.memory_mcp] {
        assert!(
            matches!(state, McpState::ManualEdit { file, snippet }
                if file == "~/.config/opencode/opencode.jsonc" && snippet.contains("\"type\": \"local\"")),
            "{state:?}"
        );
    }
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text, "unchanged byte for byte");
}

#[tokio::test]
async fn without_a_detection_a_harness_counts_as_missing() {
    let home = tempfile::tempdir().unwrap();
    let setup = status(&Env::for_tests(home.path()), &[]).await;

    assert!(setup.harnesses.iter().all(|entry| !entry.installed));
    assert_eq!(setup.harnesses.len(), 3);
}

#[tokio::test]
async fn an_installed_skills_plugin_is_reported_with_its_version() {
    let home = tempfile::tempdir().unwrap();
    install_skills_plugin(home.path());

    let setup = status(&Env::for_tests(home.path()), &[]).await;

    assert_eq!(
        harness(&setup, HarnessId::Omp).skills,
        SkillsSetup::Installed {
            version: Some("0.2.36".to_owned())
        }
    );
}

#[tokio::test]
async fn a_registered_marketplace_is_not_added_again() {
    let home = tempfile::tempdir().unwrap();
    register_skills_marketplace(home.path());

    let setup = status(&Env::for_tests(home.path()), &[]).await;

    assert_eq!(
        harness(&setup, HarnessId::Omp).skills,
        SkillsSetup::Install {
            actions: vec![plugin_install()]
        }
    );
}

#[tokio::test]
async fn pi_skills_count_as_installed_from_a_user_package() {
    let home = tempfile::tempdir().unwrap();
    write(
        &home.path().join(".pi/agent/settings.json"),
        &json!({ "packages": [{ "source": "git:github.com/MSpiechowicz/harness-useful-skills" }] }).to_string(),
    );

    let setup = status(&Env::for_tests(home.path()), &[]).await;

    assert_eq!(
        harness(&setup, HarnessId::Pi).skills,
        SkillsSetup::Installed { version: None }
    );
}

#[tokio::test]
async fn the_mcp_state_reads_the_active_omp_profile_only() {
    let home = tempfile::tempdir().unwrap();
    let entry = r#"{"mcpServers":{"harness-dashboard":{"command":"harness-dashboard","args":["mcp"]}}}"#;
    write(&home.path().join(".omp/agent/mcp.json"), entry);
    std::fs::create_dir_all(home.path().join(".omp/profiles/work/agent")).unwrap();

    let default_profile = status(&Env::for_tests(home.path()), &[]).await;
    assert_eq!(
        harness(&default_profile, HarnessId::Omp).mcp,
        McpState::Configured {
            file: "~/.omp/agent/mcp.json".to_owned(),
            name: "harness-dashboard".to_owned()
        }
    );

    let work = status(&Env::for_tests(home.path()).with_var("OMP_PROFILE", "work"), &[]).await;
    assert_eq!(
        harness(&work, HarnessId::Omp).mcp,
        McpState::Missing {
            file: "~/.omp/profiles/work/agent/mcp.json".to_owned(),
            exists: false
        }
    );
}

#[tokio::test]
async fn the_memory_and_desktop_servers_are_reported_next_to_the_dashboards() {
    let home = tempfile::tempdir().unwrap();
    let entries = json!({
        "mcpServers": {
            "harness-dashboard": { "command": "harness-dashboard", "args": ["mcp"], "enabled": false },
            "harness-memory": { "command": home.path().join(".local/bin/harness-memory"), "args": ["mcp"] },
            "harness-desktop": { "command": "/opt/harness-desktop/harness-desktop", "args": ["mcp"] }
        }
    });
    write(&home.path().join(".omp/agent/mcp.json"), &entries.to_string());

    let setup = status(&Env::for_tests(home.path()), &[]).await;
    let omp = harness(&setup, HarnessId::Omp);

    assert_eq!(
        omp.mcp,
        McpState::Disabled {
            file: "~/.omp/agent/mcp.json".to_owned(),
            name: "harness-dashboard".to_owned()
        }
    );
    assert_eq!(
        omp.memory_mcp,
        McpState::Configured {
            file: "~/.omp/agent/mcp.json".to_owned(),
            name: "harness-memory".to_owned()
        }
    );
    assert_eq!(
        omp.desktop_mcp,
        McpState::Configured {
            file: "~/.omp/agent/mcp.json".to_owned(),
            name: "harness-desktop".to_owned()
        }
    );
    assert_eq!(
        harness(&setup, HarnessId::Pi).memory_mcp,
        McpState::Missing {
            file: "~/.pi/agent/mcp.json".to_owned(),
            exists: false
        }
    );
}

#[tokio::test]
async fn windows_shows_the_powershell_installer() {
    let home = tempfile::tempdir().unwrap();
    let setup = status(&Env::for_tests(home.path()).with_os(OsKind::Windows), &[]).await;

    assert_eq!(setup.dashboard.command, DASHBOARD_INSTALL_PS1);
    assert!(
        setup
            .dashboard
            .side_effects
            .iter()
            .any(|line| line.contains("Start menu"))
    );
}
