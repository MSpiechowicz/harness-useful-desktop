//! Import of "Share setup" for Claude Code: marketplaces and plugins through `claude plugin …`, MCP
//! servers through `claude mcp add-json --scope user` (Claude Code's global file is never merged by
//! the app), what is already there, and Claude's own validation of received items. A recording
//! fake answers for `claude`; the real binary never runs.

#[path = "sync_support/import.rs"]
mod import_support;

use harness_business::model::{
    HarnessId, SyncBlockReason, SyncInvalidReason, SyncItemStatus, SyncNote, SyncOutcome, SyncStep,
};
use import_support::{
    BOTH, CLAUDE_GLOBAL, CLAUDE_MARKETPLACES, CLAUDE_PLUGINS, FakeCli, Machine, apply, argv, assert_refused, bundle,
    item, outcome_of, selectable, statuses,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const CLAUDE: &[HarnessId] = &[HarnessId::Claude];

const MARKETPLACE: &str = "claude:marketplace:tools";
const PLUGIN: &str = "claude:plugin:formatter@tools";
const DOCS: &str = "claude:mcp:docs";
const SEARCH: &str = "claude:mcp:search";

const DOCS_JSON: &str = r#"{"type":"stdio","command":"docs-server","args":["--stdio"]}"#;
const SEARCH_JSON: &str = r#"{"type":"http","url":"https://search.example.com/mcp"}"#;

/// A marketplace, a plugin from it, a stdio server that needs a value and an HTTP server.
fn shared() -> Value {
    bundle(json!({
        "claude": {
            "marketplaces": [{ "name": "tools", "source": "acme/claude-tools" }],
            "plugins": [{ "id": "formatter@tools", "marketplace": "tools", "version": "2.0.0" }],
            "mcpServers": [
                { "name": "docs", "transport": "stdio", "command": "docs-server", "args": ["--stdio"],
                  "envKeys": ["DOCS_TOKEN"] },
                { "name": "search", "transport": "http", "url": "https://search.example.com/mcp" },
            ],
        },
    }))
}

fn fake_claude(machine: &Machine) -> FakeCli {
    FakeCli::new(machine).naming("acme/claude-tools", "tools")
}

fn add_json(name: &str, json: &str) -> Vec<String> {
    argv(&["claude", "mcp", "add-json", "--scope", "user", name, json])
}

fn marketplace_add() -> Vec<String> {
    argv(&[
        "claude",
        "plugin",
        "marketplace",
        "add",
        "acme/claude-tools",
        "--scope",
        "user",
        "--json",
    ])
}

fn plugin_install(id: &str) -> Vec<String> {
    argv(&["claude", "plugin", "install", id, "--scope", "user", "--json"])
}

fn user_servers(machine: &Machine) -> Value {
    machine.json(CLAUDE_GLOBAL)["mcpServers"].clone()
}

#[tokio::test]
async fn a_new_machine_reviews_every_item_with_the_claude_command_that_adds_it() {
    let machine = Machine::new(CLAUDE);
    let built = machine.plan(&shared());

    assert_eq!(
        statuses(&built.plan),
        [MARKETPLACE, PLUGIN, DOCS, SEARCH].map(|id| (id.to_owned(), SyncItemStatus::New))
    );
    assert_eq!(
        item(&built.plan, MARKETPLACE).steps,
        [SyncStep::Cli {
            command: marketplace_add()
        }]
    );

    let plugin = item(&built.plan, PLUGIN);
    assert_eq!(
        plugin.steps,
        [SyncStep::Cli {
            command: plugin_install("formatter@tools")
        }]
    );
    assert_eq!(plugin.depends_on.as_deref(), Some(MARKETPLACE));
    assert_eq!(
        plugin.notes,
        [SyncNote::VersionInformational, SyncNote::NeedsSharedMarketplace]
    );

    let docs = item(&built.plan, DOCS);
    assert_eq!(
        docs.steps,
        [SyncStep::Cli {
            command: add_json("docs", DOCS_JSON)
        }]
    );
    assert_eq!(
        docs.notes,
        [SyncNote::EnvValuesNotShared],
        "Claude Code cannot write it switched off"
    );
    assert!(docs.runs_program && !docs.selected_by_default);

    let search = item(&built.plan, SEARCH);
    assert_eq!(
        search.steps,
        [SyncStep::Cli {
            command: add_json("search", SEARCH_JSON)
        }]
    );
    assert!(search.selected_by_default);

    assert_eq!(machine.text(CLAUDE_GLOBAL), None, "the review writes nothing");
}

#[tokio::test]
async fn apply_runs_exactly_the_reviewed_commands_once() {
    let machine = Machine::new(CLAUDE);
    let runner = fake_claude(&machine);
    let built = machine.plan(&shared());
    let all = selectable(&built.plan);

    let (result, _) = apply(&machine, &runner, &built.stored, &all, true).await;
    let result = result.expect("applied");

    assert!(result.ok, "{result:?}");
    assert_eq!(
        runner.seen(),
        [
            marketplace_add(),
            plugin_install("formatter@tools"),
            add_json("docs", DOCS_JSON),
            add_json("search", SEARCH_JSON),
        ]
    );
    assert!(result.items.iter().all(|item| item.outcome == SyncOutcome::Done));
    assert_eq!(outcome_of(&result, DOCS).argv, add_json("docs", DOCS_JSON));
    assert_eq!(
        user_servers(&machine),
        json!({
            "docs": { "type": "stdio", "command": "docs-server", "args": ["--stdio"] },
            "search": { "type": "http", "url": "https://search.example.com/mcp" },
        })
    );

    // Applying again runs nothing: every item is found here.
    let (again, _) = apply(&machine, &runner, &built.stored, &all, true).await;
    let again = again.expect("applied");

    assert!(
        again
            .items
            .iter()
            .all(|item| item.outcome == SyncOutcome::AlreadyPresent)
    );
    assert_eq!(runner.seen().len(), 4);

    let reviewed = machine.plan(&shared());
    assert_eq!(
        statuses(&reviewed.plan),
        [
            (MARKETPLACE.to_owned(), SyncItemStatus::AlreadyPresent),
            (
                PLUGIN.to_owned(),
                SyncItemStatus::DifferentVersion {
                    installed: "1.0.0".to_owned(),
                    shared: "2.0.0".to_owned(),
                }
            ),
            (DOCS.to_owned(), SyncItemStatus::AlreadyPresent),
            (SEARCH.to_owned(), SyncItemStatus::AlreadyPresent),
        ]
    );
    assert!(selectable(&reviewed.plan).is_empty());
}

/// Claude Code with a marketplace of another source, an older plugin, a project-only plugin and
/// two user servers.
fn busy_machine() -> Machine {
    let machine = Machine::new(CLAUDE);

    machine.write(
        CLAUDE_MARKETPLACES,
        &json!({
            "tools": { "source": { "source": "github", "repo": "someone-else/tools" } },
            "other": { "source": { "source": "git", "url": "https://git.example.com/other.git" } },
        }),
    );
    machine.write(
        CLAUDE_PLUGINS,
        &json!({ "version": 2, "plugins": {
            "lint@other": [{ "scope": "user", "version": "1.0.0" }],
            "mine@other": [{ "scope": "local", "projectPath": "/somewhere" }],
            "org@other": [{ "scope": "managed", "version": "1.0.0" }],
        }}),
    );
    machine.write(
        CLAUDE_GLOBAL,
        &json!({
            "numStartups": 3,
            "mcpServers": {
                "docs": { "type": "stdio", "command": "docs-server", "args": ["--stdio"], "env": { "DOCS_TOKEN": "x" } },
                "search": { "type": "http", "url": "https://elsewhere.example.com/mcp" },
            },
        }),
    );

    machine
}

#[tokio::test]
async fn what_is_here_is_compared_and_never_overwritten() {
    let machine = busy_machine();
    let runner = fake_claude(&machine);
    let before = machine.text(CLAUDE_GLOBAL).unwrap();

    let shared = bundle(json!({
        "claude": {
            "marketplaces": [
                { "name": "tools", "source": "acme/claude-tools" },
                { "name": "other", "source": "https://git.example.com/other" },
            ],
            "plugins": [
                { "id": "formatter@tools" },
                { "id": "lint@other", "version": "1.2.0" },
                { "id": "mine@other" },
                { "id": "org@other" },
            ],
            "mcpServers": [
                { "name": "docs", "transport": "stdio", "command": "docs-server", "args": ["--stdio"] },
                { "name": "search", "transport": "http", "url": "https://search.example.com/mcp" },
                { "name": "fresh", "transport": "http", "url": "https://fresh.example.com/mcp" },
            ],
        },
    }));
    let built = machine.plan(&shared);

    assert_eq!(
        statuses(&built.plan),
        [
            (MARKETPLACE.to_owned(), SyncItemStatus::Conflict),
            ("claude:marketplace:other".to_owned(), SyncItemStatus::AlreadyPresent),
            (PLUGIN.to_owned(), SyncItemStatus::Conflict),
            (
                "claude:plugin:lint@other".to_owned(),
                SyncItemStatus::DifferentVersion {
                    installed: "1.0.0".to_owned(),
                    shared: "1.2.0".to_owned(),
                }
            ),
            ("claude:plugin:mine@other".to_owned(), SyncItemStatus::New),
            ("claude:plugin:org@other".to_owned(), SyncItemStatus::AlreadyPresent),
            (DOCS.to_owned(), SyncItemStatus::AlreadyPresent),
            (SEARCH.to_owned(), SyncItemStatus::Conflict),
            ("claude:mcp:fresh".to_owned(), SyncItemStatus::New),
        ]
    );

    let (result, _) = apply(&machine, &runner, &built.stored, &selectable(&built.plan), true).await;
    assert!(result.expect("applied").ok);
    assert_eq!(
        runner.seen(),
        [
            plugin_install("mine@other"),
            add_json("fresh", r#"{"type":"http","url":"https://fresh.example.com/mcp"}"#),
        ]
    );

    let servers = user_servers(&machine);
    let mut kept: Value = serde_json::from_str(&before).unwrap();
    kept["mcpServers"]["fresh"] = json!({ "type": "http", "url": "https://fresh.example.com/mcp" });
    assert_eq!(servers, kept["mcpServers"], "existing servers are left as they were");
}

#[tokio::test]
async fn a_server_that_appears_after_the_review_is_left_alone() {
    let machine = Machine::new(CLAUDE);
    let runner = fake_claude(&machine);
    let built = machine.plan(&shared());

    let theirs = json!({ "mcpServers": { "search": { "type": "http", "url": "https://mine.example.com/mcp" } } });
    machine.write(CLAUDE_GLOBAL, &theirs);

    let (result, _) = apply(&machine, &runner, &built.stored, &[SEARCH.to_owned()], true).await;
    let result = result.expect("applied");

    assert_eq!(outcome_of(&result, SEARCH).outcome, SyncOutcome::Conflict);
    assert!(runner.seen().is_empty(), "add-json never ran");
    assert_eq!(machine.json(CLAUDE_GLOBAL), theirs);
}

#[tokio::test]
async fn a_failed_add_json_is_reported_with_its_command_line() {
    let machine = Machine::new(CLAUDE);
    let failing = add_json("search", SEARCH_JSON);
    let words: Vec<&str> = failing.iter().map(String::as_str).collect();
    let runner = fake_claude(&machine).failing(&words);
    let built = machine.plan(&shared());

    let (result, _) = apply(&machine, &runner, &built.stored, &[SEARCH.to_owned()], true).await;
    let result = result.expect("applied");
    let search = outcome_of(&result, SEARCH);

    assert!(!result.ok);
    assert_eq!(search.outcome, SyncOutcome::Failed);
    assert_eq!(search.argv, failing);
    assert!(search.summary.is_some());
    assert_eq!(machine.text(CLAUDE_GLOBAL), None);
}

#[tokio::test]
async fn hostile_values_are_invalid_and_nothing_runs() {
    let machine = Machine::new(CLAUDE);
    let runner = fake_claude(&machine);
    let canary = ["CANARY", "-", "7f3a9c"].concat();

    let hostile = bundle(json!({
        "claude": {
            "marketplaces": [
                { "name": "creds", "source": format!("https://deploy:{canary}@git.example.com/x.git") },
                { "name": "local", "source": "/home/someone/market" },
                { "name": "flag", "source": "--help/x" },
                { "name": "ssh", "source": "git@github.com:acme/x.git" },
            ],
            "plugins": [
                { "id": "formatter" },
                { "id": "../x@tools" },
                { "id": "formatter@tools", "marketplace": "other" },
            ],
            "mcpServers": [
                { "name": "declared", "transport": "stdio", "command": "server", "args": ["${DOCS_TOKEN}"],
                  "envKeys": ["DOCS_TOKEN"] },
                { "name": "env-url", "transport": "http", "url": "https://${HOST}/mcp" },
                { "name": "env-command", "transport": "stdio", "command": "${SERVER}" },
                { "name": "shell", "transport": "stdio", "command": "bash", "args": ["-c", "id"] },
                { "name": "harness-memory", "transport": "stdio", "command": "x" },
                { "name": "app", "transport": "stdio", "command": "harness-dashboard" },
            ],
        },
    }));
    let built = machine.plan(&hostile);

    assert!(selectable(&built.plan).is_empty());
    assert!(
        statuses(&built.plan)
            .iter()
            .all(|(_, status)| matches!(status, SyncItemStatus::Invalid { .. })),
        "{:?}",
        statuses(&built.plan)
    );
    assert_eq!(built.plan.invalid_count, 13);
    assert!(!format!("{:?}", built.plan).contains(&canary));

    let reason = |id: &str| match &item(&built.plan, id).status {
        SyncItemStatus::Invalid { reason } => *reason,
        other => panic!("{id} is {other:?}"),
    };
    assert_eq!(reason("claude:plugin:#1"), SyncInvalidReason::BadName, "a bare name");
    assert_eq!(
        reason("claude:mcp:#1"),
        SyncInvalidReason::UnsafeValue,
        "a declared ${{…}}"
    );

    let ids: Vec<String> = statuses(&built.plan).into_iter().map(|(id, _)| id).collect();
    let (result, _) = apply(&machine, &runner, &built.stored, &ids, true).await;
    assert_refused(result);
    assert!(runner.seen().is_empty());
    assert_eq!(machine.text(CLAUDE_GLOBAL), None);
}

#[tokio::test]
async fn the_stricter_claude_rules_do_not_apply_to_other_harnesses() {
    let machine = Machine::new(BOTH);
    let server = json!({ "name": "declared", "transport": "stdio", "command": "server",
                          "args": ["${DOCS_TOKEN}"], "envKeys": ["DOCS_TOKEN"] });
    let built = machine.plan(&bundle(json!({
        "omp": { "plugins": [{ "id": "formatter" }], "mcpServers": [server] },
    })));

    assert_eq!(
        statuses(&built.plan),
        [
            ("omp:plugin:formatter".to_owned(), SyncItemStatus::New),
            ("omp:mcp:declared".to_owned(), SyncItemStatus::New),
        ]
    );
}

#[tokio::test]
async fn claude_code_that_is_not_installed_blocks_every_item() {
    let machine = Machine::new(BOTH);
    let runner = fake_claude(&machine);
    let built = machine.plan(&shared());

    assert!(statuses(&built.plan).iter().all(|(_, status)| *status
        == SyncItemStatus::Blocked {
            reason: SyncBlockReason::HarnessNotInstalled
        }));
    assert!(!built.plan.sections[0].installed);

    let (result, _) = apply(&machine, &runner, &built.stored, &[SEARCH.to_owned()], true).await;
    assert_refused(result);
    assert!(runner.seen().is_empty());
}
