//! Import of "Share setup" MCP servers against the files a harness only reads (OpenCode's other
//! config layers, OMP's `.mcp.json`): a symlinked one is followed, and one that exists but cannot
//! be read or parsed fails closed. The server is never offered as new and nothing is written, so
//! an imported server can never merge with a same-name server the user has in that file. A server
//! is present only when every file holding its name agrees, and a target file the app refuses to
//! write offers none of its servers. A conflict that comes from a file the app cannot read carries
//! the note `configUnreadable`; one with a different definition does not.

use crate::import_support;

use harness_business::model::{HarnessId, SyncImportItem, SyncItemOutcome, SyncItemStatus, SyncNote, SyncOutcome};
use import_support::{
    BOTH, Machine, OMP_MCP, OPENCODE_JSON, apply, bundle, fake, item, outcome_of, selectable, statuses,
};
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

/// The reviewed `github` server, never offered.
fn reviewed(machine: &Machine) -> SyncImportItem {
    let built = machine.plan(&shared());
    assert!(selectable(&built.plan).is_empty(), "nothing is offered");

    let shown = serde_json::to_string(&built.plan).unwrap();
    assert!(!shown.contains(&token()), "the plan leaks the header");

    item(&built.plan, GITHUB).clone()
}

fn status(machine: &Machine) -> SyncItemStatus {
    reviewed(machine).status
}

/// Applies a plan built before `change` ran: the recheck right before the write must catch it.
async fn applied_after(machine: &Machine, change: impl FnOnce()) -> SyncItemOutcome {
    let built = machine.plan(&shared());
    assert_eq!(statuses(&built.plan), [(GITHUB.to_owned(), SyncItemStatus::New)]);

    change();

    let runner = fake(machine);
    let (result, _) = apply(machine, &runner, &built.stored, &[GITHUB.to_owned()], true).await;

    outcome_of(&result.expect("applied"), GITHUB).clone()
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

    // A different definition: a plain conflict.
    assert_eq!(outcome.outcome, SyncOutcome::Conflict);
    assert!(outcome.notes.is_empty());
    let reviewed = reviewed(&machine);
    assert_eq!(reviewed.status, SyncItemStatus::Conflict);
    assert!(reviewed.notes.is_empty());
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

        assert_eq!(outcome.outcome, SyncOutcome::Conflict, "{what}");
        assert_eq!(outcome.notes, [SyncNote::ConfigUnreadable], "{what}");
        let reviewed = reviewed(&machine);
        assert_eq!(reviewed.status, SyncItemStatus::Conflict, "{what}");
        assert_eq!(reviewed.notes, [SyncNote::ConfigUnreadable], "{what}");
        assert_eq!(machine.text(OPENCODE_JSON), None, "{what}: nothing is written");
    }

    let machine = Machine::new(OPENCODE);
    std::fs::create_dir_all(machine.home().join(LEGACY)).unwrap();
    let reviewed = reviewed(&machine);
    assert_eq!(reviewed.status, SyncItemStatus::Conflict, "a directory");
    assert_eq!(reviewed.notes, [SyncNote::ConfigUnreadable], "a directory");
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
    assert_eq!(item(&built.plan, "omp:mcp:docs").notes, [SyncNote::ConfigUnreadable]);
    assert!(selectable(&built.plan).is_empty());
    assert_eq!(machine.text(OMP_MCP), None);
}

#[tokio::test]
async fn an_omp_dot_mcp_json_broken_after_the_review_is_an_unreadable_conflict() {
    let machine = Machine::new(BOTH);
    let shared = bundle(json!({ "omp": { "mcpServers": [
        { "name": "docs", "transport": "http", "url": "https://docs.example.com/mcp" },
    ]}}));
    let built = machine.plan(&shared);
    assert_eq!(selectable(&built.plan), ["omp:mcp:docs"]);

    let path = machine.home().join(".omp/agent/.mcp.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{ \"mcpServers\": ").unwrap();

    let runner = fake(&machine);
    let (result, _) = apply(&machine, &runner, &built.stored, &["omp:mcp:docs".to_owned()], true).await;
    let outcome = outcome_of(&result.expect("applied"), "omp:mcp:docs").clone();

    assert_eq!(outcome.outcome, SyncOutcome::Conflict);
    assert_eq!(outcome.notes, [SyncNote::ConfigUnreadable]);
    assert_eq!(machine.text(OMP_MCP), None, "nothing is written");
}

/// The bundle's `github` server, as an OpenCode entry.
fn same_as_shared() -> Value {
    json!({ "mcp": { "github": { "type": "remote", "url": "https://attacker.example.com/mcp" } } })
}

#[test]
fn every_layer_holding_the_name_must_agree_for_it_to_be_present() {
    let machine = Machine::new(OPENCODE);
    machine.write(OPENCODE_JSON, &same_as_shared());

    // The global file agrees, a later layer holds the user's server with its header.
    machine.write(LEGACY, &users_config());
    let reviewed_now = reviewed(&machine);
    assert_eq!(reviewed_now.status, SyncItemStatus::Conflict);
    assert!(reviewed_now.notes.is_empty(), "a different definition");

    // One that cannot be read fails closed even when another layer agrees.
    std::fs::write(machine.home().join(LEGACY), "{ \"mcp\": ").unwrap();
    let reviewed_now = reviewed(&machine);
    assert_eq!(reviewed_now.status, SyncItemStatus::Conflict);
    assert_eq!(reviewed_now.notes, [SyncNote::ConfigUnreadable]);

    machine.write(LEGACY, &same_as_shared());
    assert_eq!(status(&machine), SyncItemStatus::AlreadyPresent);
}

#[test]
fn an_omp_dot_mcp_json_that_disagrees_with_the_user_file_is_a_conflict() {
    let machine = Machine::new(BOTH);
    let shared = bundle(json!({ "omp": { "mcpServers": [
        { "name": "docs", "transport": "http", "url": "https://docs.example.com/mcp" },
    ]}}));
    machine.write(
        OMP_MCP,
        &json!({ "mcpServers": { "docs": { "type": "http", "url": "https://docs.example.com/mcp" } } }),
    );
    machine.write(
        ".omp/agent/.mcp.json",
        &json!({ "mcpServers": { "docs": { "type": "http", "url": "https://mine.example.com/mcp" } } }),
    );

    let built = machine.plan(&shared);

    assert_eq!(
        statuses(&built.plan),
        [("omp:mcp:docs".to_owned(), SyncItemStatus::Conflict)]
    );
    assert!(
        item(&built.plan, "omp:mcp:docs").notes.is_empty(),
        "a different definition"
    );
}

/// A target file the app refuses to write (here a symlink) offers none of its servers, even one it
/// holds already, for every harness.
#[cfg(unix)]
#[test]
fn a_refused_target_file_offers_nothing() {
    let cases = [
        (
            OPENCODE,
            OPENCODE_JSON,
            "opencode:mcp:github",
            shared(),
            same_as_shared(),
        ),
        (
            BOTH,
            OMP_MCP,
            "omp:mcp:github",
            bundle(json!({ "omp": { "mcpServers": [
                { "name": "github", "transport": "http", "url": "https://attacker.example.com/mcp" },
            ]}})),
            json!({ "mcpServers": { "github": { "type": "http", "url": "https://attacker.example.com/mcp" } } }),
        ),
    ];

    for (installed, target, id, shared, held) in cases {
        let machine = Machine::new(installed);
        machine.write("dotfiles/target.json", &held);
        let link = machine.home().join(target);
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(machine.home().join("dotfiles/target.json"), &link).unwrap();

        let built = machine.plan(&shared);

        assert_eq!(
            statuses(&built.plan),
            [(id.to_owned(), SyncItemStatus::Conflict)],
            "{id}"
        );
        assert_eq!(item(&built.plan, id).notes, [SyncNote::ConfigUnreadable], "{id}");
        assert!(selectable(&built.plan).is_empty(), "{id}");
    }
}
