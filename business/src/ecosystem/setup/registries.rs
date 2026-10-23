//! The plugin and marketplace registries Recommended setup reads to tell whether the Harness Skills
//! plugin is installed in a marketplace harness, and whether its marketplace is registered.

use crate::env::Env;
use crate::harness::Harness;
use crate::harnesses::omp::{installed_plugins, marketplaces};
use crate::harnesses::{claude, codex};
use crate::model::HarnessId;

/// What a marketplace harness has installed and registered for its user.
pub(super) struct MarketplaceRegistries {
    pub plugins: Vec<FoundPlugin>,
    /// The ID of every plugin the registry lists, whatever scope it is installed in (a project
    /// plugin included), which decides whether a marketplace is still in use.
    pub all_plugin_ids: Vec<String>,
    /// Name and source of each registered marketplace.
    pub marketplaces: Vec<(String, String)>,
}

/// A plugin installed for the user.
pub(super) struct FoundPlugin {
    pub id: String,
    pub version: Option<String>,
    /// False when the harness's settings do not switch it on.
    pub enabled: bool,
}

/// The plugin and marketplace registries of `harness`: Claude Code's and Codex's (each plugin on
/// unless its settings switch it off), OMP's (the user's plugins, every one counted as on, and its
/// marketplace registry). PI and OpenCode have no marketplaces, so they have none.
pub(super) fn marketplace_registries(env: &Env, harness: &dyn Harness) -> MarketplaceRegistries {
    match harness.id() {
        HarnessId::Claude => MarketplaceRegistries {
            plugins: claude::user_plugins(env)
                .into_iter()
                .map(|plugin| FoundPlugin {
                    id: plugin.id,
                    version: plugin.version,
                    enabled: plugin.enabled,
                })
                .collect(),
            all_plugin_ids: claude::plugin_records(env)
                .into_iter()
                .map(|plugin| plugin.id)
                .collect(),
            marketplaces: claude::known_marketplaces(env)
                .into_iter()
                .map(|known| (known.name, known.source))
                .collect(),
        },
        HarnessId::Codex => {
            let plugins: Vec<FoundPlugin> = codex::plugin_records(env)
                .into_iter()
                .map(|plugin| FoundPlugin {
                    id: plugin.id,
                    version: plugin.version,
                    enabled: plugin.enabled,
                })
                .collect();

            MarketplaceRegistries {
                all_plugin_ids: plugins.iter().map(|plugin| plugin.id.clone()).collect(),
                plugins,
                marketplaces: codex::marketplace_records(env)
                    .into_iter()
                    .map(|record| (record.name, record.source))
                    .collect(),
            }
        }
        HarnessId::Omp => omp_registries(env, harness),
        HarnessId::Pi | HarnessId::OpenCode => MarketplaceRegistries {
            plugins: Vec::new(),
            all_plugin_ids: Vec::new(),
            marketplaces: Vec::new(),
        },
    }
}
/// OMP's registries (see [`marketplace_registries`]).
fn omp_registries(env: &Env, harness: &dyn Harness) -> MarketplaceRegistries {
    let paths = harness.paths(env, None);
    let plugins = installed_plugins(env, &paths).plugins;
    let (catalogs, _) = marketplaces(env, &paths, &plugins);

    MarketplaceRegistries {
        all_plugin_ids: plugins.iter().map(|plugin| plugin.id.clone()).collect(),
        plugins: plugins
            .into_iter()
            .map(|plugin| FoundPlugin {
                id: plugin.id,
                version: plugin.version,
                enabled: true,
            })
            .collect(),
        marketplaces: catalogs
            .into_iter()
            .map(|catalog| (catalog.name, catalog.source))
            .collect(),
    }
}
