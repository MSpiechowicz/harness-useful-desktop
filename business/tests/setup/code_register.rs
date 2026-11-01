//! The code server in every harness's user MCP file: added by absolute path as `harness-code mcp`,
//! found when it is there, kept out when the user removed it (the record says the app added it
//! before), added back once the record is cleared, and removed on its own, whether the harness keeps
//! it in `mcp.json` (OMP, PI), in OpenCode's config file (both generations) or only through its CLI
//! (Claude Code, Codex: a recording stand-in, never the real binary).

use crate::claude_support;
use crate::codex_support;

use harness_business::ecosystem::{
    CODE_RECORD_FILE, CodeRecord, CodeRegistration, code_mcp_state, mcp_target, register_code_mcp,
    register_code_mcp_cli, unregister_code_mcp, unregister_code_mcp_cli,
};
use harness_business::env::Env;
use harness_business::harness::{Harness, OpenCodeGeneration};
use harness_business::harnesses::{Claude, Codex, Omp, OpenCode, Pi};
use harness_business::model::{HarnessId, McpState};
use pretty_assertions::assert_eq;
use serde_json::json;

fn server_in(env: &Env) -> std::path::PathBuf {
    env.home.join(".local/bin/harness-code")
}

fn configured(file: &str) -> McpState {
    McpState::Configured {
        file: file.to_owned(),
        name: "harness-code".to_owned(),
    }
}

#[test]
fn files_the_app_writes_take_the_entry_keep_it_out_once_removed_and_give_it_back() {
    let harnesses: Vec<(HarnessId, Box<dyn Harness>)> = vec![
        (HarnessId::Omp, Box::new(Omp)),
        (HarnessId::Pi, Box::new(Pi)),
        (HarnessId::OpenCode, Box::new(OpenCode::default())),
        (
            HarnessId::OpenCode,
            Box::new(OpenCode::with_generation(OpenCodeGeneration::V2)),
        ),
    ];

    for (id, harness) in harnesses {
        let home = tempfile::tempdir().unwrap();
        let env = Env::for_tests(home.path());
        let file = mcp_target(&env, harness.as_ref()).expect("a user MCP file");
        let server = server_in(&env);
        let config = home.path().join("config");
        let mut record = CodeRecord::load(&config);

        assert!(matches!(
            code_mcp_state(&env, &file),
            McpState::Missing { exists: false, .. }
        ));

        let added = register_code_mcp(&file, &server, record.is_recorded(&env, &file.path));
        assert_eq!(added, CodeRegistration::Added);
        record.note(&env, id, &file.path, added);
        record.save().unwrap();

        assert!(matches!(code_mcp_state(&env, &file), McpState::Configured { ref name, .. } if name == "harness-code"));
        assert!(
            std::fs::read_to_string(&file.path)
                .unwrap()
                .contains(&server.to_string_lossy().replace('\\', "\\\\"))
        );
        assert!(config.join(CODE_RECORD_FILE).exists());

        // The user removed it: it stays out while the record says the app added it before.
        assert_eq!(unregister_code_mcp(&file), Ok(true));
        let again = CodeRecord::load(&config);
        assert_eq!(
            register_code_mcp(&file, &server, again.is_recorded(&env, &file.path)),
            CodeRegistration::SkippedRemoved
        );
        assert_eq!(unregister_code_mcp(&file), Ok(false));

        // Turning the code graph on clears the record, so the entry comes back everywhere.
        CodeRecord::clear(&config).unwrap();
        let fresh = CodeRecord::load(&config);
        assert_eq!(
            register_code_mcp(&file, &server, fresh.is_recorded(&env, &file.path)),
            CodeRegistration::Added
        );
        assert_eq!(
            register_code_mcp(&file, &server, true),
            CodeRegistration::Present,
            "an entry that is there is left alone"
        );
    }
}

#[tokio::test]
async fn claude_code_is_changed_only_through_its_cli() {
    let home = tempfile::tempdir().unwrap();
    let env = Env::for_tests(home.path());
    let global = home.path().join(".claude.json");
    let recorder = claude_support::FakeClaude::new(&global);
    let claude = claude_support::binary(home.path());
    let target = mcp_target(&env, &Claude).unwrap();
    let server = server_in(&env);
    let entry = claude_support::stdio(&server.to_string_lossy());

    let added = register_code_mcp_cli(claude_support::cli(&env, &recorder, &claude), &target, &server, false).await;
    assert_eq!(added, CodeRegistration::Added);
    assert_eq!(
        recorder.argvs(),
        [["mcp", "add-json", "--scope", "user", "harness-code", &entry.to_string()].map(str::to_owned)]
    );
    assert_eq!(
        claude_support::read_object(&global)["mcpServers"],
        json!({ "harness-code": entry })
    );
    assert_eq!(code_mcp_state(&env, &target), configured("~/.claude.json"));

    let removed = unregister_code_mcp_cli(claude_support::cli(&env, &recorder, &claude), &target).await;
    assert_eq!(removed, Ok(true));
    assert_eq!(
        recorder.argvs().last().unwrap(),
        &["mcp", "remove", "--scope", "user", "harness-code"]
    );
}

#[tokio::test]
async fn codex_is_changed_only_through_its_cli() {
    let home = tempfile::tempdir().unwrap();
    let env = Env::for_tests(home.path());
    let config = home.path().join(".codex/config.toml");
    let recorder = codex_support::FakeCodex::new(&config);
    let codex = codex_support::binary(home.path());
    let target = mcp_target(&env, &Codex).unwrap();
    let server = server_in(&env);

    let added = register_code_mcp_cli(codex_support::cli(&env, &recorder, &codex), &target, &server, false).await;
    assert_eq!(added, CodeRegistration::Added);
    assert_eq!(
        recorder.argvs(),
        [codex_support::add_stdio("harness-code", &server.to_string_lossy())]
    );
    assert_eq!(
        codex_support::read_toml(&config)["mcp_servers"],
        json!({ "harness-code": codex_support::stdio(&server.to_string_lossy()) })
    );

    let removed = unregister_code_mcp_cli(codex_support::cli(&env, &recorder, &codex), &target).await;
    assert_eq!(removed, Ok(true));
    assert_eq!(recorder.argvs().last().unwrap(), &codex_support::remove("harness-code"));
}
