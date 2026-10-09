//! Canary test for the adapters: the fixture homes hide `CANARY-7f3a9c` in every secret value,
//! every deny-listed file (`agent.db`, `auth.json`, `mcp-auth.json`, `models-store.json`), skill
//! and template bodies, plugin settings and a catalog outside the marketplace cache. No view any
//! adapter produces may contain it.

mod common;

use common::{CANARY, demo_project, env_at, json_text, omp_home, pi_home};
use harness_business::env::Env;
use harness_business::harness::HarnessRegistry;

/// Every view of every registered harness for `env`, with and without the demo project.
fn every_view(env: &Env) -> String {
    let project = demo_project();
    let mut text = String::new();

    for harness in HarnessRegistry::with_defaults().all() {
        for scope in [None, Some(project.as_path())] {
            text.push_str(&json_text(harness.config_view(env, scope).unwrap()));
            text.push_str(&json_text(harness.extensions_view(env, scope).unwrap()));
        }
        text.push_str(&json_text(harness.catalog().merged(&[])));
    }

    text
}

#[test]
fn no_view_reveals_the_canary() {
    let omp = omp_home();
    let pi = pi_home();
    let envs = [env_at(&omp), env_at(&omp).with_var("OMP_PROFILE", "work"), env_at(&pi)];

    for env in envs {
        let text = every_view(&env);

        assert!(!text.contains(CANARY), "canary leaked for home {}", env.home.display());
    }
}

#[test]
fn non_secret_configuration_stays_visible() {
    let omp_text = every_view(&env_at(&omp_home()));
    let pi_text = every_view(&env_at(&pi_home()));

    for visible in [
        "anthropic/claude-sonnet-4-5",
        "@modelcontextprotocol/server-github",
        "desktop-client",
        "https://mcp.linear.app/mcp",
        "Lints and formats on save.",
    ] {
        assert!(omp_text.contains(visible), "expected {visible} in the OMP views");
    }

    for visible in [
        "claude-sonnet-4-5",
        "ctrl+p",
        "OPENAI_API_KEY",
        "Review staged git changes",
    ] {
        assert!(pi_text.contains(visible), "expected {visible} in the PI views");
    }
}
