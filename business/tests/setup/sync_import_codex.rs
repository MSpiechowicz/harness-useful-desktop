//! Import of "Share setup" for Codex: marketplaces and plugins through `codex plugin …`, MCP
//! servers through `codex mcp add` (its `config.toml` is never written by the app), what is
//! already there, and Codex's own validation of received items. A recording fake answers for
//! `codex`; the real binary never runs.

use crate::import_support;

use harness_business::model::{
    HarnessId, SyncBlockReason, SyncImportPlan, SyncInvalidReason, SyncItemStatus, SyncNote, SyncOutcome, SyncStep,
};
use harness_business::sync::BuiltPlan;
use import_support::{
    BOTH, CODEX_CONFIG, FakeCli, Machine, apply, argv, assert_refused, bundle, item, outcome_of, selectable, statuses,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const CODEX: &[HarnessId] = &[HarnessId::Codex];

const MARKETPLACE: &str = "codex:marketplace:kit";
const PLUGIN: &str = "codex:plugin:lint@kit";
const FILES: &str = "codex:mcp:files";
const WEB: &str = "codex:mcp:web";

/// The marketplace `kit`, its plugin `lint`, a stdio server that needs a value and an HTTP one.
fn shared() -> Value {
    let files = json!({ "name": "files", "transport": "stdio", "command": "files-server", "args": ["--root", "."],
                        "envKeys": ["FILES_TOKEN"] });
    let web = json!({ "name": "web", "transport": "http", "url": "https://web.example.com/mcp" });

    bundle(json!({ "codex": {
        "marketplaces": [{ "name": "kit", "source": "acme/codex-kit" }],
        "plugins": [{ "id": "lint@kit", "marketplace": "kit", "version": "3.1.0" }],
        "mcpServers": [files, web],
    }}))
}

/// A Codex machine, its fake `codex` and the review of `shared` on it.
struct Review {
    machine: Machine,
    runner: FakeCli,
    built: BuiltPlan,
}

fn review(installed: &[HarnessId], shared: &Value) -> Review {
    let machine = Machine::new(installed);
    let runner = FakeCli::new(&machine).naming("acme/codex-kit", "kit");
    let built = machine.plan(shared);

    Review { machine, runner, built }
}

impl Review {
    /// Applies `ids` of the review, confirmed.
    async fn apply(&self, ids: &[String]) -> harness_business::Result<harness_business::model::SyncImportResult> {
        apply(&self.machine, &self.runner, &self.built.stored, ids, true)
            .await
            .0
    }

    fn write_config(&self, text: &str) {
        std::fs::create_dir_all(self.machine.home().join(".codex")).unwrap();
        std::fs::write(self.machine.home().join(CODEX_CONFIG), text).unwrap();
    }
}

/// Every item id of `plan`, selectable or not.
fn every_id(plan: &SyncImportPlan) -> Vec<String> {
    statuses(plan).into_iter().map(|(id, _)| id).collect()
}

/// Why the item `id` of `plan` is invalid.
fn invalid_reason(plan: &SyncImportPlan, id: &str) -> SyncInvalidReason {
    match &item(plan, id).status {
        SyncItemStatus::Invalid { reason } => *reason,
        other => panic!("{id} is {other:?}"),
    }
}

fn add_marketplace() -> Vec<String> {
    argv(&["codex", "plugin", "marketplace", "add", "acme/codex-kit"])
}

fn add_files() -> Vec<String> {
    argv(&["codex", "mcp", "add", "files", "--", "files-server", "--root", "."])
}

fn add_web() -> Vec<String> {
    argv(&["codex", "mcp", "add", "web", "--url", "https://web.example.com/mcp"])
}

#[tokio::test]
async fn a_new_machine_reviews_every_item_with_the_codex_command_that_adds_it() {
    let review = review(CODEX, &shared());
    let plan = &review.built.plan;

    let new = |id: &str| (id.to_owned(), SyncItemStatus::New);
    assert_eq!(statuses(plan), [new(MARKETPLACE), new(PLUGIN), new(FILES), new(WEB)]);

    let steps = |id: &str| item(plan, id).steps.clone();
    let cli = |command: Vec<String>| vec![SyncStep::Cli { command }];
    assert_eq!(steps(MARKETPLACE), cli(add_marketplace()));
    assert_eq!(steps(PLUGIN), cli(argv(&["codex", "plugin", "add", "lint@kit"])));
    assert_eq!(steps(FILES), cli(add_files()));
    assert_eq!(steps(WEB), cli(add_web()));

    assert_eq!(item(plan, PLUGIN).depends_on.as_deref(), Some(MARKETPLACE));
    let files = item(plan, FILES);
    assert_eq!(files.notes, [SyncNote::EnvValuesNotShared]);
    assert!(files.runs_program && !files.selected_by_default);

    assert_eq!(review.machine.text(CODEX_CONFIG), None, "the review writes nothing");
}

#[tokio::test]
async fn apply_runs_exactly_the_reviewed_commands_once() {
    let review = review(CODEX, &shared());
    let all = selectable(&review.built.plan);

    let result = review.apply(&all).await.expect("applied");
    assert!(result.ok, "{result:?}");
    assert_eq!(outcome_of(&result, FILES).argv, add_files());

    let expected = [
        add_marketplace(),
        argv(&["codex", "plugin", "add", "lint@kit"]),
        add_files(),
        add_web(),
    ];
    assert_eq!(review.runner.seen(), expected);
    assert_eq!(
        review.machine.toml(CODEX_CONFIG)["mcp_servers"],
        json!({
            "files": { "command": "files-server", "args": ["--root", "."] },
            "web": { "url": "https://web.example.com/mcp" },
        })
    );

    // Applying again runs nothing: every item is found here.
    let again = review.apply(&all).await.expect("applied");
    assert!(
        again
            .items
            .iter()
            .all(|item| item.outcome == SyncOutcome::AlreadyPresent)
    );
    assert_eq!(review.runner.seen(), expected);
}

#[tokio::test]
async fn what_is_here_is_compared_and_only_a_ticked_different_server_is_replaced() {
    let config = "# Mine\n[marketplaces.kit]\nsource_type = \"git\"\nsource = \"someone-else/kit\"\n\n[mcp_servers.files]\ncommand = \"files-server\"\nargs = [\"--root\", \".\"]\nenv = { FILES_TOKEN = \"x\" }\n\n[mcp_servers.web]\nurl = \"https://elsewhere.example.com/mcp\"\n";
    let machine = Machine::new(CODEX);
    std::fs::create_dir_all(machine.home().join(".codex")).unwrap();
    std::fs::write(machine.home().join(CODEX_CONFIG), config).unwrap();
    let runner = FakeCli::new(&machine);
    let built = machine.plan(&shared());

    assert_eq!(
        statuses(&built.plan),
        [
            (MARKETPLACE.to_owned(), SyncItemStatus::Conflict),
            (PLUGIN.to_owned(), SyncItemStatus::Conflict),
            (FILES.to_owned(), SyncItemStatus::AlreadyPresent),
            (WEB.to_owned(), SyncItemStatus::DifferentDefinition),
        ]
    );
    assert_eq!(selectable(&built.plan), [WEB]);
    assert!(!item(&built.plan, WEB).selected_by_default);

    let (refused, _) = apply(&machine, &runner, &built.stored, &every_id(&built.plan), true).await;
    assert_refused(refused);
    assert!(runner.seen().is_empty());
    assert_eq!(machine.text(CODEX_CONFIG).as_deref(), Some(config));

    let (result, _) = apply(&machine, &runner, &built.stored, &[WEB.to_owned()], true).await;
    let result = result.expect("applied");

    assert!(result.ok, "{result:?}");
    assert_eq!(outcome_of(&result, WEB).outcome, SyncOutcome::Replaced);
    assert_eq!(
        runner.seen(),
        [
            argv(&["codex", "mcp", "remove", "web"]),
            argv(&["codex", "mcp", "add", "web", "--url", "https://web.example.com/mcp"]),
        ]
    );

    let servers = machine.toml(CODEX_CONFIG)["mcp_servers"].clone();
    assert_eq!(servers["web"], json!({ "url": "https://web.example.com/mcp" }));
    assert_eq!(
        servers["files"]["env"],
        json!({ "FILES_TOKEN": "x" }),
        "the others are untouched"
    );
}

#[tokio::test]
async fn a_server_that_appears_after_the_review_is_left_alone() {
    let review = review(CODEX, &shared());
    let theirs = "[mcp_servers.web]\nurl = \"https://mine.example.com/mcp\"\n";
    review.write_config(theirs);

    let result = review.apply(&[WEB.to_owned()]).await.expect("applied");

    assert_eq!(outcome_of(&result, WEB).outcome, SyncOutcome::Conflict);
    assert!(review.runner.seen().is_empty(), "mcp add never ran");
    assert_eq!(review.machine.text(CODEX_CONFIG).as_deref(), Some(theirs));
}

#[tokio::test]
async fn codex_rules_refuse_bare_plugins_substitutions_and_sse() {
    let hostile = bundle(json!({ "codex": {
        "plugins": [{ "id": "lint" }],
        "mcpServers": [
            { "name": "declared", "transport": "stdio", "command": "server", "args": ["${FILES_TOKEN}"],
              "envKeys": ["FILES_TOKEN"] },
            { "name": "events", "transport": "sse", "url": "https://events.example.com/sse" },
            { "name": "flag", "transport": "stdio", "command": "--help" },
        ],
    }}));
    let review = review(CODEX, &hostile);
    let plan = &review.built.plan;

    assert_eq!(
        invalid_reason(plan, "codex:plugin:#1"),
        SyncInvalidReason::BadName,
        "a bare name"
    );
    assert_eq!(
        invalid_reason(plan, "codex:mcp:#1"),
        SyncInvalidReason::UnsafeValue,
        "a ${{…}}"
    );
    assert_eq!(
        invalid_reason(plan, "codex:mcp:#2"),
        SyncInvalidReason::UnsupportedSource,
        "sse"
    );
    assert!(selectable(plan).is_empty());

    assert_refused(review.apply(&every_id(plan)).await);
    assert!(review.runner.seen().is_empty());
}

#[tokio::test]
async fn codex_that_is_not_installed_blocks_every_item() {
    let review = review(BOTH, &shared());
    let blocked = SyncItemStatus::Blocked {
        reason: SyncBlockReason::HarnessNotInstalled,
    };

    assert!(
        statuses(&review.built.plan)
            .iter()
            .all(|(_, status)| *status == blocked)
    );

    assert_refused(review.apply(&[WEB.to_owned()]).await);
    assert!(review.runner.seen().is_empty());
}
