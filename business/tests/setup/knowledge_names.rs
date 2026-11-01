//! Nothing of Harness Memory leaks into what the app does for Harness Knowledge: the shared
//! `sidecar_*` modules take the names of the server they run for, so the config file, the data
//! folder, the binary and its version banner, the `check` environment and the MCP entry of one
//! server never read or touch another's.

use crate::knowledge_stand_in;
use crate::mcp_support;
use crate::team_support;

use harness_business::ecosystem::catalog::{KNOWLEDGE_SIDECAR, MEMORY_SIDECAR};
use harness_business::ecosystem::{
    KNOWLEDGE_SESSION_VAR, KnowledgeRegistration, McpFile, Shape, knowledge_config_path, memory_config_path,
    parse_knowledge_version, parse_memory_version, register_knowledge_mcp, register_memory_mcp, resolve_knowledge,
    resolve_memory, unregister_knowledge_mcp,
};
use harness_business::env::Env;
use harness_business::model::MemoryRemoteKind;
use knowledge_stand_in::{ALL_FEATURES, cli, installed};
use mcp_support::{Home, read};
use pretty_assertions::assert_eq;
use serde_json::json;
use team_support::{exits, wait};

#[test]
fn each_server_reads_its_own_config_variable_and_default_folder() {
    let dir = tempfile::tempdir().unwrap();
    let absolute = dir.path().join("elsewhere.toml");
    let env = Env::for_tests(dir.path()).with_var("HARNESS_KNOWLEDGE_CONFIG", absolute.to_string_lossy());

    assert_eq!(knowledge_config_path(&env), Some(absolute));
    assert_ne!(
        memory_config_path(&env),
        knowledge_config_path(&env),
        "memory ignores the knowledge variable"
    );

    let relative = Env::for_tests(dir.path()).with_var("HARNESS_KNOWLEDGE_CONFIG", "relative.toml");
    assert_eq!(
        knowledge_config_path(&relative),
        None,
        "the server refuses a relative file"
    );
    assert!(memory_config_path(&relative).is_some());

    let default = knowledge_config_path(&Env::for_tests(dir.path())).unwrap();
    assert!(default.to_string_lossy().contains("harness-knowledge"));
    assert!(!default.to_string_lossy().contains("harness-memory"));
}

#[test]
fn the_data_folder_and_the_session_variable_carry_the_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("kdata");
    let env = Env::for_tests(dir.path()).with_var("HARNESS_KNOWLEDGE_DIR", folder.to_string_lossy());

    assert_eq!(KNOWLEDGE_SIDECAR.data_dir(&env), Some(folder));
    assert_ne!(MEMORY_SIDECAR.data_dir(&env), KNOWLEDGE_SIDECAR.data_dir(&env));
    assert_eq!(KNOWLEDGE_SESSION_VAR, KNOWLEDGE_SIDECAR.var("SESSION"));
}

#[test]
fn only_the_servers_own_binary_and_banner_count() {
    let dir = tempfile::tempdir().unwrap();
    let (env, memory_path) = crate::memory_cli_support::installed_at(dir.path());
    crate::memory_cli_support::install(&memory_path, None);

    assert_eq!(resolve_knowledge(&env), None, "a memory server is no knowledge server");
    assert!(resolve_memory(&env).is_some());

    let (_other, knowledge_env) = installed();
    assert!(resolve_knowledge(&knowledge_env).is_some());
    assert_eq!(resolve_memory(&knowledge_env), None);

    assert_eq!(
        parse_knowledge_version("harness-knowledge 1.0.0\n").as_deref(),
        Some("1.0.0")
    );
    assert_eq!(parse_knowledge_version("harness-memory 1.0.0\n"), None);
    assert_eq!(parse_memory_version("harness-knowledge 1.0.0\n"), None);
}

#[test]
fn the_check_environment_names_only_knowledge_variables() {
    let (_dir, env) = installed();
    let server = cli(ALL_FEATURES).starting(&["check"], exits(0, "{\"ok\":true}\n", ""));
    let url = "libsql://k.example.com";

    wait(harness_business::ecosystem::knowledge_remote_test(
        &env,
        &server,
        MemoryRemoteKind::Libsql,
        Some(url.to_owned()),
        Some("tok".to_owned()),
    ));

    let spec = server.runs.lock().unwrap().last().unwrap().clone();
    let set: Vec<String> = spec
        .env
        .iter()
        .map(|(name, _)| name.to_string_lossy().into_owned())
        .collect();
    assert!(set.contains(&"HARNESS_KNOWLEDGE_DATABASE_URL".to_owned()));
    assert!(set.contains(&"HARNESS_KNOWLEDGE_DATABASE_TOKEN".to_owned()));

    let removed: Vec<String> = spec
        .env_remove_prefixes
        .iter()
        .map(|name| name.to_string_lossy().into_owned())
        .collect();
    for name in ["URL", "TOKEN", "KIND"] {
        assert!(
            removed.contains(&format!("HARNESS_KNOWLEDGE_DATABASE_{name}")),
            "{name}"
        );
    }
    assert!(removed.contains(&"HARNESS_KNOWLEDGE_CONFIG".to_owned()));
    assert!(set.iter().chain(&removed).all(|name| !name.contains("MEMORY")));
}

fn mcp_json(home: &Home) -> McpFile {
    McpFile {
        path: home.agent("mcp.json"),
        also_read: Vec::new(),
        shape: Shape::McpServers,
    }
}

#[test]
fn the_mcp_entry_is_the_knowledge_servers_and_never_the_memory_servers() {
    let home = Home::new();
    let file = mcp_json(&home);
    let memory = home.dir.path().join(".local/bin/harness-memory");
    let knowledge = home.dir.path().join(".local/bin/harness-knowledge");

    assert_eq!(
        register_memory_mcp(&file, &memory, false),
        harness_business::ecosystem::MemoryRegistration::Added
    );
    assert_eq!(
        register_knowledge_mcp(&file, &knowledge, false),
        KnowledgeRegistration::Added
    );

    let servers = read(&home.agent("mcp.json"));
    assert_eq!(
        servers["mcpServers"]["harness-knowledge"],
        json!({ "command": knowledge.to_string_lossy(), "args": ["mcp"] })
    );
    assert_eq!(
        servers["mcpServers"]["harness-memory"]["command"],
        json!(memory.to_string_lossy())
    );

    assert_eq!(
        register_knowledge_mcp(&file, &knowledge, false),
        KnowledgeRegistration::Present
    );

    assert_eq!(unregister_knowledge_mcp(&file), Ok(true));
    let left = read(&home.agent("mcp.json"));
    assert!(left["mcpServers"].get("harness-knowledge").is_none());
    assert!(
        left["mcpServers"].get("harness-memory").is_some(),
        "memory's entry stays"
    );
    assert_eq!(unregister_knowledge_mcp(&file), Ok(false));
}
