//! Canary test for the adapters: the fixture homes hide `CANARY-7f3a9c` in every secret value,
//! every deny-listed file (`agent.db`, `opencode.db`, `auth.json`, `mcp-auth.json`,
//! `models-store.json`, OpenCode's `storage/` transcripts and logs, Claude Code's login store,
//! prompt history, caches and backups), skill, template, agent and command bodies, plugin
//! settings, MCP command-line arguments and a catalog outside the marketplace cache. No view any
//! adapter produces may contain it. Claude Code's fixture also hides it in the private keys of its
//! global file, in managed settings and in a second config directory (`claude-work`, for
//! `CLAUDE_CONFIG_DIR`) whose private files must be refused although the folder is not `.claude`.

use crate::common;

use std::path::{Path, PathBuf};

use common::{CANARY, claude_home, demo_project, env_at, files_below, json_text, omp_home, opencode_home, pi_home};
use harness_business::BusinessError;
use harness_business::env::{Env, OsKind};
use harness_business::harness::HarnessRegistry;
use harness_business::security::SafeReader;

/// The Claude fixture home with `CLAUDE_CONFIG_DIR` naming its `claude-work` folder.
fn claude_work_env(home: &Path) -> Env {
    env_at(home).with_var("CLAUDE_CONFIG_DIR", home.join("claude-work").to_string_lossy())
}

/// The Claude fixture home with its managed settings in reach (`%ProgramFiles%\ClaudeCode`).
fn claude_managed_env(home: &Path) -> Env {
    env_at(home)
        .with_os(OsKind::Windows)
        .with_var("ProgramFiles", home.join("Program Files").to_string_lossy())
}

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
    let claude = claude_home();
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
        env_at(&claude),
        env_at(&claude).with_var("CLAUDE_CONFIG_DIR", claude.join(".claude").to_string_lossy()),
        claude_work_env(&claude),
        claude_managed_env(&claude),
    ];

    for env in envs {
        let text = every_view(&env);

        assert!(!text.contains(CANARY), "canary leaked for home {}", env.home.display());
    }
}

#[test]
fn a_renamed_claude_config_directory_refuses_its_private_files() {
    let home = claude_home();
    let work = home.join("claude-work");
    let _env = claude_work_env(&home);
    let readable = [work.join("settings.json"), work.join(".claude.json")];
    let private: Vec<PathBuf> = files_below(&work)
        .into_iter()
        .filter(|path| !readable.contains(path))
        .collect();

    assert!(private.len() >= 13, "the fixture keeps its private files");

    for path in private {
        let read = SafeReader::read_to_string(&path, SafeReader::DEFAULT_MAX_BYTES);

        assert!(
            matches!(read, Err(BusinessError::Forbidden(_))),
            "{} must be refused",
            path.display()
        );
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

#[test]
fn non_secret_claude_configuration_stays_visible() {
    let home = claude_home();
    let default_text = every_view(&env_at(&home));
    let work_text = every_view(&claude_work_env(&home));
    let managed_text = every_view(&claude_managed_env(&home));

    for visible in [
        "claude-sonnet-4-5",
        "Bash(git status)",
        "@modelcontextprotocol/server-github",
        "https://mcp.linear.app/mcp",
        "GITHUB_PERSONAL_ACCESS_TOKEN",
        "bun run lint",
        "Bash(bun run test)",
        "Bash(git push)",
        "DEMO_API_TOKEN",
    ] {
        assert!(default_text.contains(visible), "expected {visible} in the Claude views");
    }

    assert!(work_text.contains("claude-haiku-4-5"));
    assert!(work_text.contains("https://docs.example.com/mcp"));
    assert!(managed_text.contains("Bash(curl:*)"));
    assert!(managed_text.contains("claude-opus-4-1"));
}
