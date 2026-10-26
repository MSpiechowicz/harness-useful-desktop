//! The Claude Code side of the launch migration of the Harness Skills names: the new plugin and its
//! marketplace installed, the old plugin and marketplace removed, in the order the CLI needs.

use harness_business::model::{
    ExtensionAction, HarnessId, InstallScope, SkillsMigrationFailure, SkillsMigrationReport,
};
use pretty_assertions::assert_eq;

use super::{ALL, Executor, Machine, NEW_PLUGIN, OLD_PLUGIN, migrated};

fn install_new_plugin(with_marketplace: bool) -> Vec<ExtensionAction> {
    let install = ExtensionAction::PluginInstall {
        target: NEW_PLUGIN.to_owned(),
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

fn remove_old_plugin(with_marketplace: bool) -> Vec<ExtensionAction> {
    let uninstall = ExtensionAction::PluginUninstall {
        name: OLD_PLUGIN.to_owned(),
    };

    if with_marketplace {
        return vec![
            uninstall,
            ExtensionAction::MarketplaceRemove {
                name: "useful-skills-local".to_owned(),
            },
        ];
    }

    vec![uninstall]
}

fn claude_calls(steps: &[Vec<ExtensionAction>]) -> Vec<String> {
    steps.iter().map(|actions| format!("Claude: {actions:?}")).collect()
}

/// Migrates `machine` and checks that only Claude Code was, through exactly `steps`.
async fn assert_claude_migrated_through(machine: &Machine, steps: &[Vec<ExtensionAction>]) {
    let executor = Executor::default();

    let report = machine.migrate(&ALL, Some("0.4.0"), &executor).await;

    assert_eq!(report, migrated(&[HarnessId::Claude]));
    assert_eq!(executor.calls(), claude_calls(steps));
}

fn remove_old_marketplace() -> Vec<ExtensionAction> {
    vec![ExtensionAction::MarketplaceRemove {
        name: "useful-skills-local".to_owned(),
    }]
}

#[tokio::test]
async fn the_old_marketplace_of_the_same_repository_is_removed_before_the_new_one_is_added() {
    let machine = Machine::new();
    machine.claude_plugin(OLD_PLUGIN, true);
    // Claude Code registers a source once: adding it again under the new name would register nothing.
    machine.claude_marketplace("useful-skills-local");

    // Removing the marketplace takes the old plugin with it: no cleanup is left.
    assert_claude_migrated_through(&machine, &[remove_old_marketplace(), install_new_plugin(true)]).await;
}

#[tokio::test]
async fn a_claude_plugin_under_the_old_name_gets_the_new_plugin_and_loses_the_old_one() {
    let machine = Machine::new();
    machine.claude_plugin(OLD_PLUGIN, true);
    // The old marketplace is registered from another repository: it stands in nobody's way.
    machine.claude_marketplace_from("useful-skills-local", "someone/useful-skills");

    assert_claude_migrated_through(&machine, &[install_new_plugin(true), remove_old_plugin(true)]).await;
}

#[tokio::test]
async fn the_marketplace_is_not_added_again_when_the_new_one_is_registered() {
    let machine = Machine::new();
    machine.claude_plugin(OLD_PLUGIN, true);
    machine.claude_marketplace("harness-useful-skills");
    let executor = Executor::default();

    machine.migrate(&ALL, Some("0.4.0"), &executor).await;

    // The old marketplace is not registered, so there is nothing to remove with the old plugin.
    assert_eq!(
        executor.calls(),
        claude_calls(&[install_new_plugin(false), remove_old_plugin(false)])
    );
}

#[tokio::test]
async fn the_old_marketplace_stays_while_another_plugin_is_installed_from_it() {
    let machine = Machine::new();
    machine.claude_old_plugin_with_another();
    machine.claude_marketplace_from("useful-skills-local", "someone/useful-skills");
    let executor = Executor::default();

    machine.migrate(&ALL, Some("0.4.0"), &executor).await;

    assert_eq!(
        executor.calls(),
        claude_calls(&[install_new_plugin(true), remove_old_plugin(false)])
    );
}

#[tokio::test]
async fn nothing_is_removed_when_the_old_marketplace_of_the_same_repository_serves_another_plugin() {
    let machine = Machine::new();
    machine.claude_old_plugin_with_another();
    machine.claude_marketplace("useful-skills-local");
    let executor = Executor::default();

    let report = machine.migrate(&ALL, Some("0.4.0"), &executor).await;

    // The new marketplace cannot be registered beside it, and removing it would break the other plugin.
    assert_eq!(report, SkillsMigrationReport::default());
    assert_eq!(executor.calls(), Vec::<String>::new());
}

#[tokio::test]
async fn a_failed_install_after_the_removal_says_the_old_plugin_is_gone_and_how_to_install_the_new_one() {
    let machine = Machine::new();
    machine.claude_plugin(OLD_PLUGIN, true);
    machine.claude_marketplace("useful-skills-local");
    // The removal works, the install does not.
    let executor = Executor::scripted(&[None, Some("network down")]);

    let report = machine.migrate(&ALL, Some("0.4.0"), &executor).await;

    assert_eq!(report.migrated, []);
    assert_eq!(
        report.failed,
        [SkillsMigrationFailure {
            harness: HarnessId::Claude,
            reason:
                "The old plugin was removed, but the new one could not be installed: network down. Install it with \
                     `claude plugin marketplace add MSpiechowicz/harness-useful-skills` and \
                     `claude plugin install harness-useful-skills@harness-useful-skills`."
                    .to_owned()
        }]
    );
}

#[tokio::test]
async fn a_failed_removal_of_the_old_marketplace_is_reported_as_it_is() {
    let machine = Machine::new();
    machine.claude_plugin(OLD_PLUGIN, true);
    machine.claude_marketplace("useful-skills-local");
    let executor = Executor::failing(&["claude marketplace remove failed"]);

    let report = machine.migrate(&ALL, Some("0.4.0"), &executor).await;

    assert_eq!(
        report.failed,
        [SkillsMigrationFailure {
            harness: HarnessId::Claude,
            reason: "claude marketplace remove failed".to_owned()
        }]
    );
    assert_eq!(executor.calls(), claude_calls(&[remove_old_marketplace()]));
}

#[tokio::test]
async fn a_failed_removal_of_the_old_plugin_is_reported_and_retried_alone() {
    let machine = Machine::new();
    machine.claude_plugin(OLD_PLUGIN, true);
    machine.claude_marketplace_from("useful-skills-local", "someone/useful-skills");
    // The install works, the uninstall does not.
    let executor = Executor::scripted(&[None, Some("claude plugin uninstall failed")]);

    let report = machine.migrate(&ALL, Some("0.4.0"), &executor).await;

    assert_eq!(report.migrated, []);
    assert_eq!(
        report.failed,
        [SkillsMigrationFailure {
            harness: HarnessId::Claude,
            reason: "The new plugin is installed, but the old one could not be removed: claude plugin uninstall failed"
                .to_owned()
        }]
    );

    // The new plugin is in now (the install registered it), the old one is still there.
    machine.claude_both_plugins();
    let retry = machine.migrate(&ALL, Some("0.4.0"), &executor).await;

    assert_eq!(retry, migrated(&[HarnessId::Claude]));
    assert_eq!(
        executor.calls(),
        claude_calls(&[
            install_new_plugin(true),
            remove_old_plugin(true),
            // Nothing is installed again: the new plugin is there.
            remove_old_plugin(true),
        ])
    );
}

#[tokio::test]
async fn the_old_claude_plugin_alone_is_removed_when_the_new_one_is_already_installed() {
    let machine = Machine::new();
    machine.claude_both_plugins();

    assert_claude_migrated_through(&machine, &[remove_old_plugin(false)]).await;
}
