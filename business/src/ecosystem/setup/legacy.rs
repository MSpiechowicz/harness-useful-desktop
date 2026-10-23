//! Moving the Harness Skills of a harness from the names before Harness Skills 0.4.0 to the new
//! ones: what the launch migration runs ([`crate::ecosystem::skills_migration`]).

use super::github::same_repository;
use super::registries::marketplace_registries;
use crate::ecosystem::catalog::{LEGACY_SKILLS_PLUGIN, SKILLS_PLUGIN};
use crate::ecosystem::installer::Companion;
use crate::ecosystem::skills_record::skills_install;
use crate::env::Env;
use crate::harness::{Harness, SkillsHost, SkillsStrategy};
use crate::model::{ExtensionAction, InstallScope};

/// What moves the Harness Skills of `harness` from the names before 0.4.0 to the new ones: for a
/// marketplace harness the install of the new plugin (with its marketplace, which the old one's
/// registration does not stand in for, since it carries the old name), for an installer host its
/// installer. `None` when nothing is installed under the old names only. A plugin the user
/// switched off is left alone, so is an install that already exists under the new names.
pub fn legacy_skills_step(env: &Env, harness: &dyn Harness) -> Option<LegacySkillsStep> {
    match harness.companion_setup().skills {
        SkillsStrategy::Marketplace { source, plugin } => {
            let name = plugin.split_once('@').map_or(plugin, |(name, _)| name);
            let legacy = legacy_plugin_name(name)?;
            let registries = marketplace_registries(env, harness);

            if registries.plugins.iter().any(|found| is_plugin_named(&found.id, name)) {
                return None;
            }

            let old = registries
                .plugins
                .iter()
                .find(|found| is_plugin_named(&found.id, legacy))?;

            if !old.enabled {
                return None;
            }

            let registered = registries
                .marketplaces
                .iter()
                .any(|(marketplace, from)| marketplace == name && same_repository(from, source));

            Some(LegacySkillsStep::Actions(install_actions(registered, source, plugin)))
        }
        SkillsStrategy::Installer(host) => {
            if !skills_install(env, host).legacy {
                return None;
            }

            let companion = match host {
                SkillsHost::OpenCode => Companion::OpenCodeSkills,
                SkillsHost::Codex => Companion::CodexSkills,
            };

            Some(LegacySkillsStep::Installer(companion))
        }
        SkillsStrategy::Package { .. } | SkillsStrategy::Manual { .. } | SkillsStrategy::Unsupported { .. } => None,
    }
}

/// What [`legacy_skills_step`] runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacySkillsStep {
    /// These extension actions, in order, through the harness's CLI.
    Actions(Vec<ExtensionAction>),
    /// The Harness Skills' installer script of this companion.
    Installer(Companion),
}

/// True when plugin ID `id` is the plugin `name`, from any marketplace (`name` or `name@…`).
pub(super) fn is_plugin_named(id: &str, name: &str) -> bool {
    id == name || id.strip_prefix(name).is_some_and(|rest| rest.starts_with('@'))
}

/// The name `name` had before Harness Skills 0.4.0, for the skills plugin only.
pub(super) fn legacy_plugin_name(name: &str) -> Option<&'static str> {
    (name == SKILLS_PLUGIN).then_some(LEGACY_SKILLS_PLUGIN)
}

/// The marketplace add (unless `registered`) and the user-scope install of `plugin`.
pub(super) fn install_actions(registered: bool, source: &str, plugin: &str) -> Vec<ExtensionAction> {
    let mut actions = Vec::new();

    if !registered {
        actions.push(ExtensionAction::MarketplaceAdd {
            source: source.to_owned(),
        });
    }

    actions.push(ExtensionAction::PluginInstall {
        target: plugin.to_owned(),
        scope: InstallScope::User,
    });

    actions
}
