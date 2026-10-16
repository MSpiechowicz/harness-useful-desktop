//! Canary test for the adapters: the fixture homes hide `CANARY-7f3a9c` in every secret value,
//! every deny-listed file (`agent.db`, `opencode.db`, `auth.json`, `mcp-auth.json`,
//! `models-store.json`, OpenCode's `storage/` transcripts and logs), skill, template, agent and
//! command bodies, plugin settings, MCP command-line arguments and a catalog outside the
//! marketplace cache. No view any adapter produces may contain it.

mod common;

use common::{CANARY, demo_project, env_at, json_text, omp_home, opencode_home, pi_home};
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
    let opencode = opencode_home();
    let opencode_overrides = env_at(&opencode)
        .with_var(
            "OPENCODE_CONFIG",
            opencode.join("custom-opencode.jsonc").to_string_lossy(),
        )
        .with_var("OPENCODE_CONFIG_DIR", opencode.join("team-opencode").to_string_lossy())
        .with_var("OPENCODE_PERMISSION", format!(r#"{{"bash":"{CANARY}"}}"#))
        .with_var("OPENCODE_CONFIG_CONTENT", format!(r#"{{"model":"{CANARY}"}}"#));
    let envs = [
        env_at(&omp),
        env_at(&omp).with_var("OMP_PROFILE", "work"),
        env_at(&pi),
        env_at(&opencode),
        opencode_overrides,
    ];

    for env in envs {
        let text = every_view(&env);

        assert!(!text.contains(CANARY), "canary leaked for home {}", env.home.display());
    }
}

#[test]
fn non_secret_configuration_stays_visible() {
    let omp_text = every_view(&env_at(&omp_home()));
    let pi_text = every_view(&env_at(&pi_home()));
    let opencode_text = every_view(&env_at(&opencode_home()));

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

    for visible in [
        "anthropic/claude-sonnet-4-5",
        "desktop-client",
        "https://mcp.linear.app/mcp",
        "ANTHROPIC_API_KEY",
        "{file:./house-rules.md}",
        "Reviews staged changes for bugs and style.",
    ] {
        assert!(
            opencode_text.contains(visible),
            "expected {visible} in the OpenCode views"
        );
    }
}
