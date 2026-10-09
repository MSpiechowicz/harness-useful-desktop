//! Read-only smoke test against the real machine. Ignored by default; run with
//! `cargo test -p harness_business --test real_home_smoke -- --ignored --nocapture`.
//!
//! It prints counts only — never paths, names or values — and fails only when an adapter returns
//! an error. The project is the current directory (this repository).

use harness_business::env::Env;
use harness_business::harness::HarnessRegistry;
use harness_business::model::{ConfigView, ExtensionsView};

fn counts(config: &ConfigView, extensions: &ExtensionsView) -> String {
    let parse_errors = config.sections.iter().filter(|section| section.error.is_some()).count();
    let marketplace_errors = extensions
        .marketplaces
        .iter()
        .filter(|marketplace| marketplace.error.is_some())
        .count();
    let enabled_mcp = extensions.mcp_servers.iter().filter(|server| server.enabled).count();

    format!(
        "sections={} section_errors={} config_notices={} plugins={} marketplaces={} marketplace_errors={} \
         marketplace_entries={} skills={} templates={} mcp_servers={} mcp_enabled={} themes={} packages={} \
         modules={} extension_notices={}",
        config.sections.len(),
        parse_errors,
        config.notices.len(),
        extensions.plugins.len(),
        extensions.marketplaces.len(),
        marketplace_errors,
        extensions
            .marketplaces
            .iter()
            .map(|marketplace| marketplace.entries.len())
            .sum::<usize>(),
        extensions.skills.len(),
        extensions.prompt_templates.len(),
        extensions.mcp_servers.len(),
        enabled_mcp,
        extensions.themes.len(),
        extensions.packages.len(),
        extensions.extension_modules.len(),
        extensions.notices.len(),
    )
}

#[test]
#[ignore = "reads the real home directory; run manually"]
fn real_home_smoke() {
    let env = Env::from_process().unwrap();
    let project = std::env::current_dir()
        .ok()
        .and_then(|dir| dunce::canonicalize(dir).ok());

    for harness in HarnessRegistry::with_defaults().all() {
        for (label, scope) in [("user", None), ("project", project.as_deref())] {
            let config = harness.config_view(&env, scope).unwrap();
            let extensions = harness.extensions_view(&env, scope).unwrap();

            println!("{:?} {label}: {}", harness.id(), counts(&config, &extensions));
        }
    }
}
