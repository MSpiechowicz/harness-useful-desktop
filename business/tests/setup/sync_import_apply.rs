//! Applying a "Share setup" import: order, idempotency, guards, partial failure, and MCP merges that
//! only add allowed keys and keep everything else.

use crate::import_support;

use harness_business::BusinessError;
use harness_business::model::{
    HarnessId, SyncInvalidReason, SyncItemStatus, SyncOutcome, SyncProgress, SyncSkipReason,
};
use import_support::{
    BOTH, MCP_SECRET, Machine, OMP_MARKETPLACES, OMP_MCP, PI_MCP, apply, apply_done, argv, bundle, fake, item,
    outcome_of, selectable, statuses, typical,
};
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test]
async fn applying_runs_marketplaces_plugins_packages_then_merges_mcp_files_and_twice_runs_nothing() {
    let machine = Machine::new(BOTH);
    let runner = fake(&machine);
    let built = machine.plan(&typical());
    let selected = selectable(&built.plan);

    let (result, events) = apply_done(&machine, &runner, &built.stored, &selected).await;

    assert_eq!(
        runner.seen(),
        [
            argv(&["omp", "plugin", "marketplace", "add", "acme/useful-skills"]),
            argv(&[
                "omp",
                "plugin",
                "install",
                "harness-useful-skills@useful",
                "--scope",
                "user"
            ]),
            argv(&["omp", "plugin", "install", "npm-helper", "--scope", "user"]),
            argv(&["pi", "install", "npm:@demo/pi-review-kit@1.0.0"]),
        ]
    );

    let started: Vec<String> = events
        .iter()
        .filter_map(|event| match event {
            SyncProgress::Started { id, .. } => Some(id.clone()),
            SyncProgress::Finished { .. } => None,
        })
        .collect();
    assert_eq!(started, selected, "per harness: marketplaces, plugins, packages, MCP");
    assert_eq!(events.len(), selected.len() * 2);

    let marketplace = outcome_of(&result, "omp:marketplace:useful");
    assert_eq!(
        marketplace.argv,
        ["omp", "plugin", "marketplace", "add", "acme/useful-skills"]
    );
    assert_eq!(marketplace.summary.as_deref(), Some("Done."));

    // Only the allowed keys are written; a server needing env values is written switched off.
    assert_eq!(
        machine.json(OMP_MCP),
        json!({ "mcpServers": {
            "github": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-github"], "enabled": false },
            "remote": { "type": "http", "url": "https://mcp.example.com/mcp" },
        }})
    );
    assert_eq!(
        machine.json(PI_MCP),
        json!({ "mcpServers": { "docs": { "type": "http", "url": "https://docs.example.com/mcp" } } })
    );

    // A second review finds everything; the first plan applied again runs nothing either.
    let again = machine.plan(&typical());
    assert!(
        statuses(&again.plan)
            .iter()
            .all(|(_, status)| *status == SyncItemStatus::AlreadyPresent),
        "{:?}",
        statuses(&again.plan)
    );
    assert!(selectable(&again.plan).is_empty());

    let mcp_before = machine.text(OMP_MCP);
    let (repeated, _) = apply(&machine, &runner, &built.stored, &selected, true).await;
    let repeated = repeated.expect("applied again");

    assert!(repeated.ok);
    assert!(
        repeated
            .items
            .iter()
            .all(|item| item.outcome == SyncOutcome::AlreadyPresent)
    );
    assert_eq!(runner.seen().len(), 4, "nothing ran the second time");
    assert_eq!(machine.text(OMP_MCP), mcp_before);
}

#[tokio::test]
async fn refused_requests_run_nothing_and_change_nothing() {
    let machine = Machine::new(BOTH);
    machine.write(
        OMP_MCP,
        &json!({ "mcpServers": { "remote": { "url": "https://other.example.com" } } }),
    );
    let runner = fake(&machine);
    let built = machine.plan(&bundle(json!({
        "omp": {
            "marketplaces": [{ "name": "useful", "source": "acme/useful-skills" }],
            "plugins": [
                { "id": "harness-useful-skills@useful" },
                { "id": "orphan@nowhere" },
                { "id": "--foo" },
            ],
            "mcpServers": [{ "name": "remote", "transport": "http", "url": "https://mcp.example.com/mcp" }],
        },
    })));
    let mcp_before = machine.text(OMP_MCP);

    assert_eq!(item(&built.plan, "omp:mcp:remote").status, SyncItemStatus::Conflict);
    assert_eq!(
        item(&built.plan, "omp:plugin:#3").status,
        SyncItemStatus::Invalid {
            reason: SyncInvalidReason::UnsafeValue
        }
    );

    let other = machine.plan(&bundle(json!({ "pi": { "packages": [{ "source": "npm:other" }] } })));
    let both = argv(&["omp:marketplace:useful", "omp:plugin:harness-useful-skills@useful"]);

    let attempts: Vec<(&str, Vec<String>, bool)> = vec![
        ("unconfirmed", both.clone(), false),
        ("empty selection", Vec::new(), true),
        ("unknown id", argv(&["omp:plugin:nope@useful"]), true),
        ("conflict", argv(&["omp:mcp:remote"]), true),
        ("blocked", argv(&["omp:plugin:orphan@nowhere"]), true),
        ("invalid", argv(&["omp:plugin:#3"]), true),
        (
            "plugin without its marketplace",
            argv(&["omp:plugin:harness-useful-skills@useful"]),
            true,
        ),
        ("another plan's item", selectable(&other.plan), true),
        (
            "one bad id among good ones",
            [both.clone(), argv(&["omp:mcp:remote"])].concat(),
            true,
        ),
    ];

    for (what, selected, confirmed) in attempts {
        let (result, events) = apply(&machine, &runner, &built.stored, &selected, confirmed).await;

        assert!(
            matches!(result, Err(BusinessError::InvalidInput(_))),
            "{what}: {result:?}"
        );
        assert!(events.is_empty(), "{what}");
    }

    assert!(runner.seen().is_empty());
    assert_eq!(machine.text(OMP_MCP), mcp_before);
    assert_eq!(machine.text(OMP_MARKETPLACES), None);

    // Once the marketplace is registered here, the plugin alone may be applied.
    machine.write(
        OMP_MARKETPLACES,
        &json!({ "marketplaces": [{ "name": "useful", "sourceType": "github", "sourceUri": "acme/useful-skills" }] }),
    );
    let (result, _) = apply(
        &machine,
        &runner,
        &built.stored,
        &argv(&["omp:plugin:harness-useful-skills@useful"]),
        true,
    )
    .await;
    assert!(result.expect("applied").ok);
    assert_eq!(runner.seen().len(), 1);
}

#[tokio::test]
async fn a_failed_marketplace_skips_its_plugins_and_the_rest_still_runs() {
    let machine = Machine::new(BOTH);
    let runner = fake(&machine).failing(&["omp", "plugin", "marketplace", "add", "acme/useful-skills"]);
    let built = machine.plan(&typical());

    let (result, _) = apply(&machine, &runner, &built.stored, &selectable(&built.plan), true).await;
    let result = result.expect("applied");

    assert!(!result.ok);
    let marketplace = outcome_of(&result, "omp:marketplace:useful");
    assert_eq!(marketplace.outcome, SyncOutcome::Failed);
    assert_eq!(
        marketplace.summary.as_deref(),
        Some("✘ Failed to add marketplace: Error: not found")
    );
    assert_eq!(
        outcome_of(&result, "omp:plugin:harness-useful-skills@useful").outcome,
        SyncOutcome::Skipped {
            reason: SyncSkipReason::DependencyFailed
        }
    );
    for done in [
        "omp:plugin:npm-helper",
        "omp:mcp:github",
        "pi:package:npm:@demo/pi-review-kit",
        "pi:mcp:docs",
    ] {
        assert_eq!(outcome_of(&result, done).outcome, SyncOutcome::Done, "{done}");
    }

    assert_eq!(
        runner.seen(),
        [
            argv(&["omp", "plugin", "marketplace", "add", "acme/useful-skills"]),
            argv(&["omp", "plugin", "install", "npm-helper", "--scope", "user"]),
            argv(&["pi", "install", "npm:@demo/pi-review-kit@1.0.0"]),
        ]
    );
}

#[tokio::test]
async fn a_harness_without_a_binary_skips_its_cli_items_but_still_gets_its_mcp_servers() {
    let mut machine = Machine::new(BOTH);
    let runner = fake(&machine);
    let built = machine.plan(&typical());

    // The OMP binary is not found at apply time; its MCP file needs none.
    machine.installed = vec![HarnessId::Pi];
    let (result, _) = apply(&machine, &runner, &built.stored, &selectable(&built.plan), true).await;
    let result = result.expect("applied");

    for skipped in [
        "omp:marketplace:useful",
        "omp:plugin:harness-useful-skills@useful",
        "omp:plugin:npm-helper",
    ] {
        assert_eq!(
            outcome_of(&result, skipped).outcome,
            SyncOutcome::Skipped {
                reason: SyncSkipReason::HarnessNotInstalled
            },
            "{skipped}"
        );
    }
    for done in ["omp:mcp:github", "omp:mcp:remote", "pi:package:npm:@demo/pi-review-kit"] {
        assert_eq!(outcome_of(&result, done).outcome, SyncOutcome::Done, "{done}");
    }

    assert_eq!(
        runner.seen(),
        [argv(&["pi", "install", "npm:@demo/pi-review-kit@1.0.0"])]
    );
    assert_eq!(
        machine.json(OMP_MCP)["mcpServers"]["remote"],
        json!({ "type": "http", "url": "https://mcp.example.com/mcp" })
    );
}

#[tokio::test]
async fn existing_mcp_files_keep_everything_and_existing_definitions_never_leave() {
    let machine = Machine::new(BOTH);
    machine.write(
        OMP_MCP,
        &json!({
            "theme": "dark",
            "mcpServers": {
                "github": { "command": "other-cmd", "env": { "GITHUB_TOKEN": MCP_SECRET } },
                "keep": { "command": "keeper", "args": ["--quiet"] },
            },
            "disabledServers": ["keep"],
        }),
    );
    machine.write(
        PI_MCP,
        &json!({ "mcpServers": { "docs": {
            "type": "http", "url": "https://docs.example.com/mcp", "headers": { "Authorization": format!("Bearer {MCP_SECRET}") },
        }}}),
    );
    let runner = fake(&machine);
    let built = machine.plan(&typical());

    assert_eq!(item(&built.plan, "omp:mcp:github").status, SyncItemStatus::Conflict);
    assert_eq!(item(&built.plan, "pi:mcp:docs").status, SyncItemStatus::AlreadyPresent);

    let text = serde_json::to_string(&built.plan).unwrap();
    for leaked in [MCP_SECRET, "other-cmd", "keeper", "Bearer"] {
        assert!(!text.contains(leaked), "the plan leaks {leaked:?}");
    }

    let (result, _) = apply(&machine, &runner, &built.stored, &argv(&["omp:mcp:remote"]), true).await;
    assert_eq!(
        outcome_of(&result.expect("applied"), "omp:mcp:remote").outcome,
        SyncOutcome::Done
    );

    let written = machine.json(OMP_MCP);
    let keys: Vec<&String> = written.as_object().unwrap().keys().collect();
    let servers: Vec<&String> = written["mcpServers"].as_object().unwrap().keys().collect();
    assert_eq!(keys, ["theme", "mcpServers", "disabledServers"]);
    assert_eq!(servers, ["github", "keep", "remote"]);
    assert_eq!(
        written["mcpServers"]["github"],
        json!({ "command": "other-cmd", "env": { "GITHUB_TOKEN": MCP_SECRET } })
    );
    assert_eq!(
        written["mcpServers"]["remote"],
        json!({ "type": "http", "url": "https://mcp.example.com/mcp" })
    );
    assert!(runner.seen().is_empty());
}

#[tokio::test]
async fn env_references_are_only_written_for_declared_keys_and_the_dashboard_name_never_is() {
    let machine = Machine::new(BOTH);
    let runner = fake(&machine);
    let built = machine.plan(&bundle(json!({ "omp": { "mcpServers": [
        { "name": "query-ref", "transport": "http", "url": "https://c.example.com/mcp?k=${GITHUB_TOKEN}" },
        { "name": "host-ref", "transport": "sse", "url": "https://${ANTHROPIC_API_KEY}.c.example.com/mcp",
          "envKeys": ["ANTHROPIC_API_KEY"] },
        { "name": "home-arg", "transport": "stdio", "command": "my-server", "args": ["$HOME"] },
        { "name": "harness-dashboard", "transport": "http", "url": "https://c.example.com/mcp" },
        { "name": "declared", "transport": "stdio", "command": "my-server", "args": ["--api-key", "${API_KEY}"],
          "envKeys": ["API_KEY"] },
    ]}})));

    let invalid = |reason| SyncItemStatus::Invalid { reason };
    let statuses: Vec<SyncItemStatus> = statuses(&built.plan).into_iter().map(|(_, status)| status).collect();
    assert_eq!(
        statuses,
        [
            invalid(SyncInvalidReason::UnsafeValue),
            invalid(SyncInvalidReason::UnsafeValue),
            invalid(SyncInvalidReason::UnsafeValue),
            invalid(SyncInvalidReason::BadName),
            SyncItemStatus::New,
        ]
    );
    assert_eq!(selectable(&built.plan), ["omp:mcp:declared"]);

    let all = import_support::ids(&built.plan);
    let (result, events) = apply(&machine, &runner, &built.stored, &all, true).await;
    assert!(matches!(result, Err(BusinessError::InvalidInput(_))), "{result:?}");
    assert!(events.is_empty());
    assert_eq!(machine.text(OMP_MCP), None);

    let (result, _) = apply(&machine, &runner, &built.stored, &argv(&["omp:mcp:declared"]), true).await;
    assert!(result.expect("applied").ok);
    assert_eq!(
        machine.json(OMP_MCP),
        json!({ "mcpServers": {
            "declared": { "command": "my-server", "args": ["--api-key", "${API_KEY}"], "enabled": false },
        }})
    );
}

/// A file refused at review time offers none of its servers; one refused only by the time of the
/// apply skips them.
#[tokio::test]
async fn a_refused_mcp_file_skips_its_servers() {
    let machine = Machine::new(BOTH);
    std::fs::create_dir_all(machine.home().join(".omp/agent")).unwrap();
    std::fs::write(machine.home().join(OMP_MCP), "{ not json").unwrap();

    let refused = machine.plan(&typical());
    for id in ["omp:mcp:github", "omp:mcp:remote"] {
        assert_eq!(item(&refused.plan, id).status, SyncItemStatus::Conflict);
        assert!(!selectable(&refused.plan).contains(&id.to_owned()));
    }

    std::fs::remove_file(machine.home().join(OMP_MCP)).unwrap();
    let built = machine.plan(&typical());
    std::fs::write(machine.home().join(OMP_MCP), "{ not json").unwrap();

    let runner = fake(&machine);
    let (result, _) = apply(
        &machine,
        &runner,
        &built.stored,
        &argv(&["omp:mcp:github", "omp:mcp:remote"]),
        true,
    )
    .await;
    let result = result.expect("applied");

    assert!(!result.ok);
    for id in ["omp:mcp:github", "omp:mcp:remote"] {
        assert!(matches!(
            outcome_of(&result, id).outcome,
            SyncOutcome::Skipped {
                reason: SyncSkipReason::McpFileRefused { .. }
            }
        ));
    }
    assert_eq!(machine.text(OMP_MCP).as_deref(), Some("{ not json"));
}
