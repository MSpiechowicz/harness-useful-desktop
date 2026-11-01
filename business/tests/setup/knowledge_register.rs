//! The knowledge server in every harness's user MCP file: added by absolute path as `harness-knowledge
//! mcp`, found when it is there, kept out when the user removed it, and removed on its own, whether
//! the harness keeps it in `mcp.json` (OMP, PI), in OpenCode's config file (both generations) or
//! only through its CLI (Claude Code, Codex: a recording stand-in, never the real binary).

use crate::claude_support;
use crate::codex_support;

use std::path::PathBuf;

use harness_business::ecosystem::{
    KnowledgeRegistration, knowledge_mcp_state, mcp_target, register_knowledge_mcp, register_knowledge_mcp_cli,
    unregister_knowledge_mcp, unregister_knowledge_mcp_cli,
};
use harness_business::env::Env;
use harness_business::harness::Harness;
use harness_business::harness::OpenCodeGeneration;
use harness_business::harnesses::{Claude, Codex, Omp, OpenCode, Pi};
use harness_business::model::McpState;
use pretty_assertions::assert_eq;
use serde_json::json;

fn knowledge(env: &Env) -> PathBuf {
    env.home.join(".local/bin/harness-knowledge")
}

fn configured(file: &str) -> McpState {
    McpState::Configured {
        file: file.to_owned(),
        name: "harness-knowledge".to_owned(),
    }
}

#[test]
fn files_the_app_writes_take_the_entry_and_give_it_back() {
    let harnesses: Vec<(&str, Box<dyn Harness>)> = vec![
        ("omp", Box::new(Omp)),
        ("pi", Box::new(Pi)),
        ("opencode 1", Box::new(OpenCode::default())),
        (
            "opencode 2",
            Box::new(OpenCode::with_generation(OpenCodeGeneration::V2)),
        ),
    ];

    for (name, harness) in harnesses {
        let dir = tempfile::tempdir().unwrap();
        let env = Env::for_tests(dir.path());
        let file = mcp_target(&env, harness.as_ref()).unwrap_or_else(|| panic!("{name} has a file"));
        let program = knowledge(&env);

        assert!(
            matches!(
                knowledge_mcp_state(&env, &file),
                McpState::Missing { exists: false, .. }
            ),
            "{name}"
        );

        assert_eq!(
            register_knowledge_mcp(&file, &program, false),
            KnowledgeRegistration::Added,
            "{name}"
        );
        assert!(
            matches!(knowledge_mcp_state(&env, &file), McpState::Configured { ref name, .. } if name == "harness-knowledge"),
            "{name}"
        );
        assert!(
            std::fs::read_to_string(&file.path)
                .unwrap()
                .contains(&program.to_string_lossy().replace('\\', "\\\\")),
            "{name}"
        );

        assert_eq!(
            register_knowledge_mcp(&file, &program, true),
            KnowledgeRegistration::Present,
            "{name}"
        );
        assert_eq!(unregister_knowledge_mcp(&file), Ok(true), "{name}");
        assert!(
            matches!(knowledge_mcp_state(&env, &file), McpState::Missing { exists: true, .. }),
            "{name}"
        );

        // The user removed it: it stays out while the record says the app added it before.
        assert_eq!(
            register_knowledge_mcp(&file, &program, true),
            KnowledgeRegistration::SkippedRemoved,
            "{name}"
        );
        assert_eq!(unregister_knowledge_mcp(&file), Ok(false), "{name}");
    }
}

#[tokio::test]
async fn claude_code_is_changed_only_through_its_cli() {
    let dir = tempfile::tempdir().unwrap();
    let env = Env::for_tests(dir.path());
    let global = dir.path().join(".claude.json");
    let fake = claude_support::FakeClaude::new(&global);
    let claude = claude_support::binary(dir.path());
    let file = mcp_target(&env, &Claude).unwrap();
    let program = knowledge(&env);
    let entry = claude_support::stdio(&program.to_string_lossy());

    let added = register_knowledge_mcp_cli(claude_support::cli(&env, &fake, &claude), &file, &program, false).await;
    assert_eq!(added, KnowledgeRegistration::Added);
    assert_eq!(
        fake.argvs(),
        [[
            "mcp",
            "add-json",
            "--scope",
            "user",
            "harness-knowledge",
            &entry.to_string()
        ]
        .map(str::to_owned)]
    );
    assert_eq!(
        claude_support::read_object(&global)["mcpServers"],
        json!({ "harness-knowledge": entry })
    );
    assert_eq!(knowledge_mcp_state(&env, &file), configured("~/.claude.json"));

    let removed = unregister_knowledge_mcp_cli(claude_support::cli(&env, &fake, &claude), &file).await;
    assert_eq!(removed, Ok(true));
    assert_eq!(
        fake.argvs().last().unwrap(),
        &["mcp", "remove", "--scope", "user", "harness-knowledge"]
    );
}

#[tokio::test]
async fn codex_is_changed_only_through_its_cli() {
    let dir = tempfile::tempdir().unwrap();
    let env = Env::for_tests(dir.path());
    let config = dir.path().join(".codex/config.toml");
    let fake = codex_support::FakeCodex::new(&config);
    let codex = codex_support::binary(dir.path());
    let file = mcp_target(&env, &Codex).unwrap();
    let program = knowledge(&env);

    let added = register_knowledge_mcp_cli(codex_support::cli(&env, &fake, &codex), &file, &program, false).await;
    assert_eq!(added, KnowledgeRegistration::Added);
    assert_eq!(
        fake.argvs(),
        [codex_support::add_stdio(
            "harness-knowledge",
            &program.to_string_lossy()
        )]
    );
    assert_eq!(
        codex_support::read_toml(&config)["mcp_servers"],
        json!({ "harness-knowledge": codex_support::stdio(&program.to_string_lossy()) })
    );

    let removed = unregister_knowledge_mcp_cli(codex_support::cli(&env, &fake, &codex), &file).await;
    assert_eq!(removed, Ok(true));
    assert_eq!(
        fake.argvs().last().unwrap(),
        &codex_support::remove("harness-knowledge")
    );
}
