//! Which companion tools are installed (`ecosystem_status`).
//!
//! - Dashboard: the `harness-dashboard` binary ([`resolve_dashboard`]) and its bare version from
//!   `harness-dashboard version` (5 s limit). Its data directory, `usage.db` and `auth-token` are
//!   never read.
//! - Skills and sidebar: the user's OMP plugins ([`installed_plugins`]: marketplace installs such
//!   as `harness-useful-skills@harness-useful-skills`, and npm installs), with their versions and,
//!   from the cached marketplace catalogs ([`mark_updates`]), whether a newer one is offered.

use super::catalog::{DASHBOARD_BINARY, ECOSYSTEM, SIDEBAR_PLUGIN, SKILLS_PLUGIN, catalog_item};
use super::dashboard::resolve_dashboard;
use crate::env::Env;
use crate::harness::{VERSION_TIMEOUT, first_semver};
use crate::harnesses::omp::{installed_plugins, mark_updates, marketplaces, omp_paths};
use crate::model::{EcosystemId, EcosystemItem, EcosystemStatus, PluginInfo};
use crate::pi_family::tilde;
use crate::process::{ExecSpec, ProcessRunner, ResolvedBinary, launcher_search_dirs};

/// Output cap of the version probe.
const VERSION_OUTPUT_BYTES: usize = 64 * 1024;

/// Every tool with its detection on this machine. Reads the OMP plugin registries; call it from a
/// blocking-capable context.
pub async fn ecosystem_status(env: &Env, runner: &dyn ProcessRunner) -> EcosystemStatus {
    let paths = omp_paths(env, None);
    let mut plugins = installed_plugins(env, &paths).plugins;
    // Only the cached catalogs are read; nothing is fetched. Their notices belong to the Extensions view.
    let (catalogs, _) = marketplaces(env, &paths, &plugins);
    mark_updates(&mut plugins, &catalogs);
    let mut items = Vec::with_capacity(ECOSYSTEM.len());

    for id in ECOSYSTEM {
        let mut item = catalog_item(id, env.os);

        match id {
            EcosystemId::Dashboard => detect_dashboard(env, runner, &mut item).await,
            EcosystemId::Skills => detect_plugin(&plugins, SKILLS_PLUGIN, &mut item),
            EcosystemId::Sidebar => detect_plugin(&plugins, SIDEBAR_PLUGIN, &mut item),
        }

        items.push(item);
    }

    EcosystemStatus { items }
}

async fn detect_dashboard(env: &Env, runner: &dyn ProcessRunner, item: &mut EcosystemItem) {
    let Some(binary) = resolve_dashboard(env) else {
        return;
    };

    item.installed = true;
    item.via.push(format!("binary {}", tilde(env, &binary.path)));

    match dashboard_version(env, runner, &binary).await {
        Some(version) => item.version = Some(version),
        None => item
            .notes
            .push(format!("`{DASHBOARD_BINARY} version` did not report a version.")),
    }
}

/// `harness-dashboard version` of `binary` (bare semver), within [`VERSION_TIMEOUT`].
pub(crate) async fn dashboard_version(
    env: &Env,
    runner: &dyn ProcessRunner,
    binary: &ResolvedBinary,
) -> Option<String> {
    let spec = ExecSpec::new(&binary.path)
        .args(["version"])
        .search_dirs(launcher_search_dirs(env, &binary.path))
        .timeout(VERSION_TIMEOUT)
        .max_output_bytes(VERSION_OUTPUT_BYTES);

    let output = runner.run(&spec).await.ok()?;

    if !output.success() {
        return None;
    }

    first_semver(&output.stdout)
}

/// Marks `item` installed for every OMP plugin named `name` (`name@<marketplace>` or the npm
/// package `name`).
fn detect_plugin(plugins: &[PluginInfo], name: &str, item: &mut EcosystemItem) {
    let marketplace_prefix = format!("{name}@");
    let matching = plugins
        .iter()
        .filter(|plugin| plugin.id == name || plugin.id.starts_with(&marketplace_prefix));

    for plugin in matching {
        let version = plugin
            .version
            .as_deref()
            .map(|version| format!(" v{version}"))
            .unwrap_or_default();
        let disabled = if plugin.enabled { "" } else { " (disabled)" };

        item.installed = true;
        item.via.push(format!("OMP plugin {}{version}{disabled}", plugin.id));

        if item.version.is_none() {
            item.version = plugin.version.clone();
        }

        if plugin.update_available {
            item.update_available = true;
            item.latest_version = plugin.latest_version.clone();
        }
    }
}
