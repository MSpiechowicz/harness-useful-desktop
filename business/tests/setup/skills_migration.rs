//! The launch migration of the Harness Skills names: an install found only under the names before
//! Harness Skills 0.4.0 is moved by the install path Recommended setup uses, once, never to an older
//! release, and again at a later launch after a failure.

use crate::claude_support;

use std::path::Path;
use std::sync::Mutex;

use claude_support::write_json;
use harness_business::ecosystem::{Companion, MigrationExecutor, migrate_skills_names};
use harness_business::env::Env;
use harness_business::harness::{Harness, HarnessRegistry};
use harness_business::model::{
    ExtensionAction, HarnessId, InstallScope, SkillsMigrationFailure, SkillsMigrationReport,
};
use harness_business::process::BoxFuture;
use pretty_assertions::assert_eq;
use serde_json::json;

/// Records what it was asked to run and answers from a script (success once the script is empty).
#[derive(Default)]
struct Executor {
    calls: Mutex<Vec<String>>,
    failures: Mutex<Vec<String>>,
}

impl Executor {
    fn failing(reasons: &[&str]) -> Self {
        Self {
            calls: Mutex::default(),
            failures: Mutex::new(reasons.iter().rev().map(|reason| (*reason).to_owned()).collect()),
        }
    }

    fn answer(&self, call: String) -> Result<(), String> {
        self.calls.lock().unwrap().push(call);

        match self.failures.lock().unwrap().pop() {
            Some(reason) => Err(reason),
            None => Ok(()),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl MigrationExecutor for Executor {
    fn run_actions<'a>(
        &'a self,
        harness: HarnessId,
        actions: &'a [ExtensionAction],
    ) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async move { self.answer(format!("{harness:?}: {actions:?}")) })
    }

    fn run_installer(&self, companion: Companion) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(async move { self.answer(format!("{companion:?}")) })
    }
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A home, its config folder for the record and the environment.
struct Machine {
    home: tempfile::TempDir,
    config: tempfile::TempDir,
    env: Env,
}

impl Machine {
    fn new() -> Self {
        let home = tempfile::tempdir().unwrap();
        let env = Env::for_tests(home.path());

        Self {
            home,
            config: tempfile::tempdir().unwrap(),
            env,
        }
    }

    fn codex_record(&self, folder: &str, version: &str) {
        write(
            &self.home.path().join(".codex").join(folder).join("install-record.json"),
            &json!({ "version": version }).to_string(),
        );
    }

    fn claude_plugin(&self, id: &str, enabled: bool) {
        let config = self.home.path().join(".claude");
        write_json(
            &config.join("plugins/installed_plugins.json"),
            &json!({ "version": 2, "plugins": { id: [{ "scope": "user", "installPath": "/x", "version": "0.3.14" }] } }),
        );
        write_json(
            &config.join("settings.json"),
            &json!({ "enabledPlugins": { id: enabled } }),
        );
    }

    fn claude_marketplace(&self, name: &str) {
        write_json(
            &self.home.path().join(".claude/plugins/known_marketplaces.json"),
            &json!({ name: { "source": { "source": "github", "repo": "MSpiechowicz/harness-useful-skills" }, "installLocation": "/x" } }),
        );
    }

    async fn migrate(&self, ids: &[HarnessId], latest: Option<&str>, executor: &Executor) -> SkillsMigrationReport {
        let registry = HarnessRegistry::with_defaults();
        let harnesses: Vec<&dyn Harness> = ids.iter().map(|id| registry.get(*id).unwrap()).collect();

        migrate_skills_names(&self.env, self.config.path(), &harnesses, latest, executor).await
    }
}

const ALL: [HarnessId; 4] = [HarnessId::Omp, HarnessId::Claude, HarnessId::OpenCode, HarnessId::Codex];

fn migrated(ids: &[HarnessId]) -> SkillsMigrationReport {
    SkillsMigrationReport {
        migrated: ids.to_vec(),
        failed: Vec::new(),
    }
}

#[tokio::test]
async fn an_install_under_the_old_names_is_migrated_once() {
    let machine = Machine::new();
    machine.codex_record("useful-skills", "0.3.14");
    let executor = Executor::default();

    let report = machine.migrate(&ALL, Some("0.4.0"), &executor).await;

    assert_eq!(report, migrated(&[HarnessId::Codex]));
    assert_eq!(executor.calls(), ["CodexSkills"]);

    // The installer could not rename it (the old record is still there): not run at every launch.
    let again = machine.migrate(&ALL, Some("0.4.0"), &executor).await;

    assert_eq!(again, SkillsMigrationReport::default());
    assert_eq!(executor.calls().len(), 1);
    assert!(machine.config.path().join("skills-names-migration.json").is_file());
}

#[tokio::test]
async fn nothing_old_means_nothing_runs() {
    let machine = Machine::new();
    let executor = Executor::default();

    assert_eq!(
        machine.migrate(&ALL, Some("0.4.0"), &executor).await,
        SkillsMigrationReport::default()
    );

    // Installs under the new names, a plugin switched off, and a plugin of another name.
    machine.codex_record("harness-useful-skills", "0.4.0");
    machine.claude_plugin("useful-skills@useful-skills-local", false);
    assert_eq!(
        machine.migrate(&ALL, Some("0.4.0"), &executor).await,
        SkillsMigrationReport::default()
    );

    machine.claude_plugin("other@market", true);
    assert_eq!(
        machine.migrate(&ALL, Some("0.4.0"), &executor).await,
        SkillsMigrationReport::default()
    );

    assert_eq!(executor.calls(), Vec::<String>::new());
    assert!(
        !machine.config.path().join("skills-names-migration.json").exists(),
        "nothing changed, nothing is written"
    );
}

#[tokio::test]
async fn a_failure_is_reported_and_retried_at_the_next_launch() {
    let machine = Machine::new();
    machine.codex_record("useful-skills", "0.3.14");
    let executor = Executor::failing(&["the codex command was not found"]);

    let report = machine.migrate(&ALL, Some("0.4.0"), &executor).await;

    assert_eq!(report.migrated, []);
    assert_eq!(
        report.failed,
        [SkillsMigrationFailure {
            harness: HarnessId::Codex,
            reason: "the codex command was not found".to_owned()
        }]
    );

    let retry = machine.migrate(&ALL, Some("0.4.0"), &executor).await;

    assert_eq!(retry, migrated(&[HarnessId::Codex]));
    assert_eq!(executor.calls(), ["CodexSkills", "CodexSkills"]);
}

#[tokio::test]
async fn it_never_runs_without_a_release_that_has_the_new_names() {
    let machine = Machine::new();
    machine.codex_record("useful-skills", "0.3.14");
    let executor = Executor::default();

    for latest in [
        None,
        Some("0.3.14"),
        Some("0.3.99"),
        Some("0.3.9-rc.1"),
        Some("0.4.0-rc.1"),
        Some("nonsense"),
    ] {
        assert_eq!(
            machine.migrate(&ALL, latest, &executor).await,
            SkillsMigrationReport::default(),
            "{latest:?}"
        );
    }

    assert_eq!(executor.calls(), Vec::<String>::new());

    for latest in ["0.4.0", "0.4.1", "1.0.0"] {
        let fresh = Machine::new();
        fresh.codex_record("useful-skills", "0.3.14");

        assert_eq!(
            fresh.migrate(&ALL, Some(latest), &Executor::default()).await,
            migrated(&[HarnessId::Codex]),
            "{latest}"
        );
    }
}

#[tokio::test]
async fn old_names_that_come_back_are_migrated_again() {
    let machine = Machine::new();
    machine.codex_record("useful-skills", "0.3.14");
    let executor = Executor::default();

    machine.migrate(&ALL, Some("0.4.0"), &executor).await;

    // The installer renamed it.
    std::fs::remove_dir_all(machine.home.path().join(".codex/useful-skills")).unwrap();
    machine.codex_record("harness-useful-skills", "0.4.0");
    machine.migrate(&ALL, Some("0.4.0"), &executor).await;

    // An older copy is installed again.
    std::fs::remove_dir_all(machine.home.path().join(".codex/harness-useful-skills")).unwrap();
    machine.codex_record("useful-skills", "0.3.14");
    let report = machine.migrate(&ALL, Some("0.4.0"), &executor).await;

    assert_eq!(report, migrated(&[HarnessId::Codex]));
    assert_eq!(executor.calls().len(), 2);
}

#[tokio::test]
async fn only_the_given_harnesses_are_touched() {
    let machine = Machine::new();
    machine.codex_record("useful-skills", "0.3.14");
    let executor = Executor::default();

    let report = machine.migrate(&[HarnessId::OpenCode], Some("0.4.0"), &executor).await;

    assert_eq!(report, SkillsMigrationReport::default());
    assert_eq!(executor.calls(), Vec::<String>::new());
}

#[tokio::test]
async fn the_opencode_installer_migrates_an_old_record() {
    let machine = Machine::new();
    write(
        &machine
            .home
            .path()
            .join(".config/opencode/plugins/useful-skills/install-record.json"),
        r#"{"version":"0.3.14"}"#,
    );
    let executor = Executor::default();

    assert_eq!(
        machine.migrate(&ALL, Some("0.4.0"), &executor).await,
        migrated(&[HarnessId::OpenCode])
    );
    assert_eq!(executor.calls(), ["OpenCodeSkills"]);
}

fn install_new_plugin(with_marketplace: bool) -> Vec<ExtensionAction> {
    let install = ExtensionAction::PluginInstall {
        target: "harness-useful-skills@harness-useful-skills".to_owned(),
        scope: InstallScope::User,
    };

    if with_marketplace {
        return vec![
            ExtensionAction::MarketplaceAdd {
                source: "MSpiechowicz/harness-useful-skills".to_owned(),
            },
            install,
        ];
    }

    vec![install]
}

#[tokio::test]
async fn a_claude_plugin_under_the_old_name_gets_the_new_plugin_with_its_marketplace() {
    let machine = Machine::new();
    machine.claude_plugin("useful-skills@useful-skills-local", true);
    // The old marketplace is registered from the same repository, but under the old name.
    machine.claude_marketplace("useful-skills-local");
    let executor = Executor::default();

    let report = machine.migrate(&ALL, Some("0.4.0"), &executor).await;

    assert_eq!(report, migrated(&[HarnessId::Claude]));
    assert_eq!(executor.calls(), [format!("Claude: {:?}", install_new_plugin(true))]);
}

#[tokio::test]
async fn the_marketplace_is_not_added_again_when_the_new_one_is_registered() {
    let machine = Machine::new();
    machine.claude_plugin("useful-skills@useful-skills-local", true);
    machine.claude_marketplace("harness-useful-skills");
    let executor = Executor::default();

    machine.migrate(&ALL, Some("0.4.0"), &executor).await;

    assert_eq!(executor.calls(), [format!("Claude: {:?}", install_new_plugin(false))]);
}

#[tokio::test]
async fn an_omp_plugin_under_the_old_name_is_migrated_too() {
    let machine = Machine::new();
    let home = machine.home.path();
    write(
        &home.join(".omp/plugins/cache/plugins/useful-skills/package.json"),
        &json!({ "name": "useful-skills", "version": "0.3.14" }).to_string(),
    );
    write(
        &home.join(".omp/plugins/installed_plugins.json"),
        &json!({ "version": 2, "plugins": { "useful-skills@harness-useful-skills": [{
            "scope": "user", "installPath": "~/.omp/plugins/cache/plugins/useful-skills", "version": "0.3.14"
        }] } })
        .to_string(),
    );
    write(
        &home.join(".omp/plugins/omp-plugins.lock.json"),
        &json!({ "plugins": { "useful-skills": { "version": "0.3.14", "enabled": true } } }).to_string(),
    );
    let executor = Executor::default();

    let report = machine.migrate(&ALL, Some("0.4.0"), &executor).await;

    assert_eq!(report, migrated(&[HarnessId::Omp]));
    assert!(executor.calls()[0].starts_with("Omp: "), "{:?}", executor.calls());
}
