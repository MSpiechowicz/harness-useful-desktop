//! Moving the Harness Skills of a harness from the names before Harness Skills 0.4.0 to the new
//! ones: what the launch migration runs ([`crate::ecosystem::skills_migration`]).

use super::github::same_repository;
use super::registries::{MarketplaceRegistries, marketplace_registries};
use crate::ecosystem::installer::Companion;
use crate::ecosystem::skills_record::skills_install;
use crate::env::Env;
use crate::harness::{Harness, SkillsHost, SkillsStrategy};
use crate::model::{ExtensionAction, InstallScope};

/// What moves the Harness Skills of `harness` from the names before 0.4.0 to the new ones.
///
/// - A marketplace harness that renamed its plugin ID (Claude Code, [`SkillsStrategy::Marketplace`]
///   with a `legacy` ID): the install of the new plugin with its marketplace (the old one's
///   registration does not stand in for it, since it carries the old name, and when the same
///   repository is registered under the old name that registration is removed first), then the
///   removal of the old plugin, because Claude Code keeps the two as separate plugins and both would load, and of
///   the old marketplace when nothing else is installed from it. Once the new plugin is there the
///   step is the removal alone. A plugin the user switched off is left alone.
/// - An installer host (OpenCode, Codex): its installer, which migrates the old plugin, MCP entry
///   and data by itself.
/// - OMP: nothing. Its plugin ID did not change, and the plugin moves its own data on its first
///   session.
///
/// `None` when nothing is installed under the old names only.
pub fn legacy_skills_step(env: &Env, harness: &dyn Harness) -> Option<LegacySkillsStep> {
    match harness.companion_setup().skills {
        SkillsStrategy::Marketplace {
            source,
            plugin,
            legacy: Some(legacy),
        } => marketplace_step(&marketplace_registries(env, harness), source, plugin, legacy),
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
        SkillsStrategy::Marketplace { legacy: None, .. }
        | SkillsStrategy::Package { .. }
        | SkillsStrategy::Manual { .. }
        | SkillsStrategy::Unsupported { .. } => None,
    }
}

/// The step of a marketplace harness whose plugin `legacy` is installed (see [`legacy_skills_step`]).
///
/// Claude Code names a marketplace once per source: adding the repository again while it is
/// registered under another name (the old one) answers "already on disk" and registers nothing, so
/// the install of the new plugin would not find its marketplace. That old registration is removed
/// first, and with it the old plugin, when no other plugin is installed from it. If one is, the
/// step is `None`: the old install keeps working and nothing else is broken for it.
fn marketplace_step(
    registries: &MarketplaceRegistries,
    source: &str,
    plugin: &str,
    legacy: &str,
) -> Option<LegacySkillsStep> {
    let name = plugin.split_once('@').map_or(plugin, |(name, _)| name);
    let old = registries.plugins.iter().find(|found| found.id == legacy)?;

    if !old.enabled {
        return None;
    }

    let cleanup = cleanup_actions(registries, legacy);
    let installed = registries.plugins.iter().any(|found| is_plugin_named(&found.id, name));

    if installed {
        return Some(LegacySkillsStep::Actions {
            remove_first: Vec::new(),
            install: Vec::new(),
            cleanup,
        });
    }

    match source_registration(registries, name, source, Some(legacy)) {
        SourceRegistration::Named => Some(LegacySkillsStep::Actions {
            remove_first: Vec::new(),
            install: install_actions(true, source, plugin),
            cleanup,
        }),
        SourceRegistration::Absent => Some(LegacySkillsStep::Actions {
            remove_first: Vec::new(),
            install: install_actions(false, source, plugin),
            cleanup,
        }),
        SourceRegistration::Other { in_use: true, .. } => None,
        SourceRegistration::Other { name: holder, .. } => {
            // Removing the marketplace of the old plugin takes the plugin with it.
            let cleanup = if legacy.rsplit_once('@').is_some_and(|(_, from)| from == holder) {
                Vec::new()
            } else {
                cleanup
            };

            Some(LegacySkillsStep::Actions {
                remove_first: vec![ExtensionAction::MarketplaceRemove { name: holder }],
                install: install_actions(false, source, plugin),
                cleanup,
            })
        }
    }
}

/// How the repository `source` is registered as a marketplace (see [`source_registration`]).
pub(super) enum SourceRegistration {
    /// Under the name the plugin's ID expects.
    Named,
    /// Not at all.
    Absent,
    /// Under another name (the old one), which the harness answers "already on disk" to when the
    /// source is added again, so the expected marketplace is never registered. `in_use`: a plugin
    /// is installed from it, which removing the marketplace would take away.
    Other { name: String, in_use: bool },
}

/// How `source` is registered, for a plugin named `name`. A plugin `except` (the one being
/// replaced) does not count as using the marketplace.
pub(super) fn source_registration(
    registries: &MarketplaceRegistries,
    name: &str,
    source: &str,
    except: Option<&str>,
) -> SourceRegistration {
    let from_source = |(_, from): &&(String, String)| same_repository(from, source);

    if registries
        .marketplaces
        .iter()
        .filter(from_source)
        .any(|(marketplace, _)| marketplace == name)
    {
        return SourceRegistration::Named;
    }

    match registries.marketplaces.iter().find(from_source) {
        Some((holder, _)) => SourceRegistration::Other {
            in_use: marketplace_in_use(registries, holder, except),
            name: holder.clone(),
        },
        None => SourceRegistration::Absent,
    }
}

/// True when a plugin other than `except` (if any) is installed (in any scope) from `marketplace`.
fn marketplace_in_use(registries: &MarketplaceRegistries, marketplace: &str, except: Option<&str>) -> bool {
    registries
        .all_plugin_ids
        .iter()
        .filter(|id| Some(id.as_str()) != except)
        .any(|id| id.rsplit_once('@').is_some_and(|(_, from)| from == marketplace))
}

/// The removal of the plugin `legacy`, and of its marketplace when it is registered and no other
/// plugin (of any scope) is installed from it.
fn cleanup_actions(registries: &MarketplaceRegistries, legacy: &str) -> Vec<ExtensionAction> {
    let mut actions = vec![ExtensionAction::PluginUninstall {
        name: legacy.to_owned(),
    }];

    let Some((_, marketplace)) = legacy.split_once('@') else {
        return actions;
    };

    let registered = registries.marketplaces.iter().any(|(name, _)| name == marketplace);

    if registered && !marketplace_in_use(registries, marketplace, Some(legacy)) {
        actions.push(ExtensionAction::MarketplaceRemove {
            name: marketplace.to_owned(),
        });
    }

    actions
}

/// What [`legacy_skills_step`] runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacySkillsStep {
    /// Extension actions through the harness's CLI, in this order: `remove_first` (empty unless the
    /// old marketplace registration stands in the way of the new one, see [`marketplace_step`]; a
    /// failure after it leaves no Harness Skills), `install` (empty when the new plugin is already
    /// installed), then `cleanup`, which removes what the old names left.
    Actions {
        remove_first: Vec<ExtensionAction>,
        install: Vec<ExtensionAction>,
        cleanup: Vec<ExtensionAction>,
    },
    /// The Harness Skills' installer script of this companion.
    Installer(Companion),
}

/// True when plugin ID `id` is the plugin `name`, from any marketplace (`name` or `name@…`).
pub(super) fn is_plugin_named(id: &str, name: &str) -> bool {
    id == name || id.strip_prefix(name).is_some_and(|rest| rest.starts_with('@'))
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
