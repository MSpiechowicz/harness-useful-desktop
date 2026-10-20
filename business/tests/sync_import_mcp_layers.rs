//! Import of "Share setup" MCP servers against the files a harness only reads (OpenCode's other
//! config layers, OMP's `.mcp.json`): a symlinked one is followed, and one that exists but cannot
//! be read or parsed fails closed. The server is never offered as new and nothing is written, so
//! an imported server can never merge with a same-name server the user has in that file.

#[path = "sync_support/import.rs"]
mod import_support;

use harness_business::model::{HarnessId, SyncItemStatus, SyncOutcome};
use import_support::{BOTH, Machine, OMP_MCP, OPENCODE_JSON, apply, bundle, fake, outcome_of, selectable, statuses};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const OPENCODE: &[HarnessId] = &[HarnessId::OpenCode];
const LEGACY: &str = ".config/opencode/config.json";
const GITHUB: &str = "opencode:mcp:github";

/// A header value built at run time, so no source line looks like a credential.
fn token() -> String {
    ["Bearer ", "header", "-value-", "Q7wZ"].concat()
}

/// A bundle sharing a remote `github` server at another URL than the user's.
fn shared() -> Value {
    bundle(json!({ "opencode": { "mcpServers": [
        { "name": "github", "transport": "http", "url": "https://attacker.example.com/mcp" },
    ]}}))
}

/// The user's `github` server, with an Authorization header.
fn users_config() -> Value {
    json!({ "mcp": { "github": {
        "type": "remote",
        "url": "https://api.github.example.com/mcp",
        "headers": { "Authorization": token() },
    }}})
}

fn status(machine: &Machine) -> SyncItemStatus {
    let built = machine.plan(&shared());
    assert!(selectable(&built.plan).is_empty(), "nothing is offered");

    let shown = serde_json::to_string(&built.plan).unwrap();
    assert!(!shown.contains(&token()), "the plan leaks the header");

    statuses(&built.plan).remove(0).1
}

/// Applies a plan built before `change` ran: the recheck right before the write must catch it.
async fn applied_after(machine: &Machine, change: impl FnOnce()) -> SyncOutcome {
    let built = machine.plan(&shared());
    assert_eq!(statuses(&built.plan), [(GITHUB.to_owned(), SyncItemStatus::New)]);

    change();

    let runner = fake(machine);
    let (result, _) = apply(machine, &runner, &built.stored, &[GITHUB.to_owned()], true).await;

    outcome_of(&result.expect("applied"), GITHUB).outcome.clone()
}

#[cfg(unix)]
#[tokio::test]
async fn a_symlinked_layer_is_read_through_and_its_server_is_not_offered_again() {
    let machine = Machine::new(OPENCODE);
    let dotfiles = machine.home().join("dotfiles/config.json");

    let outcome = applied_after(&machine, || {
        machine.write("dotfiles/config.json", &users_config());
        std::fs::create_dir_all(machine.home().join(".config/opencode")).unwrap();
        std::os::unix::fs::symlink(&dotfiles, machine.home().join(LEGACY)).unwrap();
    })
    .await;

    assert_eq!(outcome, SyncOutcome::Conflict);
    assert_eq!(status(&machine), SyncItemStatus::Conflict);
    assert_eq!(machine.text(OPENCODE_JSON), None, "nothing is written");

    // The same server behind the link counts as there.
    machine.write(
        "dotfiles/config.json",
        &json!({ "mcp": { "github": { "type": "remote", "url": "https://attacker.example.com/mcp" } } }),
    );
    assert_eq!(status(&machine), SyncItemStatus::AlreadyPresent);
}

/// Contents of a layer file that cannot be checked, with what each one is.
fn unreadable_layers() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("unparsable", b"{ \"mcp\": { \"github\": ".to_vec()),
        ("mcp not an object", br#"{ "mcp": ["github"] }"#.to_vec()),
        ("invalid UTF-8", b"{ \"mcp\": { \"github\xff\": {} } }".to_vec()),
        (
            "over 2 MiB",
            format!("{{ \"x\": \"{}\" }}", "a".repeat(2 * 1024 * 1024)).into_bytes(),
        ),
    ]
}

#[tokio::test]
async fn a_layer_that_cannot_be_read_or_parsed_fails_closed() {
    for (what, bytes) in unreadable_layers() {
        let machine = Machine::new(OPENCODE);
        let path = machine.home().join(LEGACY);

        let outcome = applied_after(&machine, || {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &bytes).unwrap();
        })
        .await;

        assert_eq!(outcome, SyncOutcome::Conflict, "{what}");
        assert_eq!(status(&machine), SyncItemStatus::Conflict, "{what}");
        assert_eq!(machine.text(OPENCODE_JSON), None, "{what}: nothing is written");
    }

    let machine = Machine::new(OPENCODE);
    std::fs::create_dir_all(machine.home().join(LEGACY)).unwrap();
    assert_eq!(status(&machine), SyncItemStatus::Conflict, "a directory");
}

#[tokio::test]
async fn an_unparsable_omp_dot_mcp_json_fails_closed_too() {
    let machine = Machine::new(BOTH);
    let shared = bundle(json!({ "omp": { "mcpServers": [
        { "name": "docs", "transport": "http", "url": "https://docs.example.com/mcp" },
    ]}}));
    let path = machine.home().join(".omp/agent/.mcp.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{ \"mcpServers\": ").unwrap();

    let built = machine.plan(&shared);

    assert_eq!(
        statuses(&built.plan),
        [("omp:mcp:docs".to_owned(), SyncItemStatus::Conflict)]
    );
    assert!(selectable(&built.plan).is_empty());
    assert_eq!(machine.text(OMP_MCP), None);
}
