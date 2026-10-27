//! Replacing an HTTP MCP server of "Share setup" that exists with another definition at the same
//! endpoint (same scheme, host and port), in the files the app writes (OMP, PI, OpenCode 1 and
//! 2): offered unticked, only the managed keys (`type`, `command`, `args`, `url`; OpenCode: `type`,
//! `command`, `url`) change in place, every other key keeps its value and position, a changed entry
//! is resolved when the apply re-reads it, and no value read from a file leaves in the plan or the
//! result. Another endpoint replaces the whole entry (see `sync_import_replace_whole`).

use crate::import_support;

use harness_business::model::{
    HarnessId, McpTransport, SyncItemStatus, SyncMcpPreview, SyncNote, SyncOutcome, SyncSkipReason, SyncStep,
};
use import_support::{BOTH, Machine, OMP_MCP, OPENCODE_JSON, PI_MCP};
use import_support::{apply, apply_replaced, apply_selectable_in, assert_different_definitions};
use import_support::{assert_no_leaks, bundle, fake, github_and_remote, header, item, keys};
use import_support::{opencode_github_and_remote, outcome_of, plan_opencode_2, registry_v2, secret, selectable};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const OPENCODE: &[HarnessId] = &[HarnessId::OpenCode];

/// A stdio server needing a token, and an HTTP one needing a header.
fn shared() -> Value {
    bundle(json!({ "omp": { "mcpServers": github_and_remote() }}))
}

/// An OMP user file with the HTTP server defined otherwise, holding values and other keys.
fn existing_omp() -> Value {
    json!({
        "theme": "dark",
        "mcpServers": {
            "keep": { "command": "keeper" },
            "remote": { "type": "http", "headers": { "Authorization": header() }, "url": "https://mcp.example.com/old" },
        },
        "disabledServers": ["keep"],
    })
}

/// What of the existing files must not show in a plan or a result.
fn needles() -> Vec<String> {
    ["/old", "--old", "keeper", "old-cmd"]
        .map(str::to_owned)
        .into_iter()
        .chain([secret(), header()])
        .collect()
}

#[tokio::test]
async fn omp_replaces_only_the_managed_keys_and_keeps_everything_else_in_place() {
    let machine = Machine::new(BOTH);
    machine.write(OMP_MCP, &existing_omp());
    let runner = fake(&machine);
    let built = machine.plan(&shared());

    let row = item(&built.plan, "omp:mcp:remote");
    assert_eq!(row.status, SyncItemStatus::DifferentDefinition);
    assert!(row.selectable && !row.selected_by_default);
    assert!(!row.notes.contains(&SyncNote::WrittenDisabled));

    assert_eq!(
        item(&built.plan, "omp:mcp:remote").steps,
        [SyncStep::McpReplace {
            file: "~/.omp/agent/mcp.json".to_owned(),
            server: SyncMcpPreview {
                name: "remote".to_owned(),
                transport: McpTransport::Http,
                command: None,
                args: Vec::new(),
                url: Some("https://mcp.example.com/mcp".to_owned()),
                enabled: true,
            },
        }]
    );
    assert_no_leaks(&built.plan, &needles());

    let result = apply_replaced(&machine, &runner, &built, &["omp:mcp:remote"]).await;
    assert_no_leaks(&result, &needles());
    assert!(runner.seen().is_empty(), "a file shape runs no command");

    let written = machine.json(OMP_MCP);
    assert_eq!(keys(&written), ["theme", "mcpServers", "disabledServers"]);
    assert_eq!(keys(&written["mcpServers"]), ["keep", "remote"]);
    assert_eq!(
        written["mcpServers"]["remote"],
        json!({ "type": "http", "headers": { "Authorization": header() }, "url": "https://mcp.example.com/mcp" })
    );
    assert_eq!(keys(&written["mcpServers"]["remote"]), ["type", "headers", "url"]);
    assert_eq!(written["mcpServers"]["keep"], json!({ "command": "keeper" }));
    assert_eq!(written["disabledServers"], json!(["keep"]));
}

#[tokio::test]
async fn pi_replaces_in_its_own_file() {
    let machine = Machine::new(BOTH);
    machine.write(
        PI_MCP,
        &json!({ "mcpServers": { "docs": {
            "type": "http", "url": "https://docs.example.com/old", "headers": { "Authorization": header() },
        }}}),
    );
    let runner = fake(&machine);
    let built = machine.plan(&bundle(json!({ "pi": { "mcpServers": [
        { "name": "docs", "transport": "http", "url": "https://docs.example.com/mcp" },
    ]}})));

    assert_eq!(
        item(&built.plan, "pi:mcp:docs").status,
        SyncItemStatus::DifferentDefinition
    );

    let (result, _) = apply(&machine, &runner, &built.stored, &["pi:mcp:docs".to_owned()], true).await;

    assert_eq!(
        outcome_of(&result.expect("applied"), "pi:mcp:docs").outcome,
        SyncOutcome::Replaced
    );
    assert_eq!(
        machine.json(PI_MCP)["mcpServers"]["docs"],
        json!({ "type": "http", "url": "https://docs.example.com/mcp", "headers": { "Authorization": header() } })
    );
}

#[tokio::test]
async fn opencode_1_keeps_enabled_and_headers_and_the_order() {
    let machine = Machine::new(OPENCODE);
    machine.write(
        OPENCODE_JSON,
        &json!({ "mcp": {
            "remote": { "enabled": true, "type": "remote", "url": "https://mcp.example.com/old", "headers": { "Authorization": header() } },
        }}),
    );
    let runner = fake(&machine);
    let built = machine.plan(&opencode_github_and_remote());

    let row = item(&built.plan, "opencode:mcp:remote");
    assert_eq!(row.status, SyncItemStatus::DifferentDefinition);
    assert!(row.selectable && !row.selected_by_default);
    assert!(
        !row.notes.contains(&SyncNote::OpenCodeMergedCheck),
        "it is in the file the app writes"
    );

    let result = apply_replaced(&machine, &runner, &built, &["opencode:mcp:remote"]).await;
    assert_no_leaks(&result, &needles());
    assert!(runner.probes().is_empty(), "no merged-names check: nothing is added");

    let written = machine.json(OPENCODE_JSON);
    assert_eq!(
        written["mcp"]["remote"],
        json!({ "enabled": true, "type": "remote", "url": "https://mcp.example.com/mcp", "headers": { "Authorization": header() } })
    );
    assert_eq!(keys(&written["mcp"]["remote"]), ["enabled", "type", "url", "headers"]);
}

#[tokio::test]
async fn opencode_2_replaces_a_nested_entry_and_a_legacy_only_one_in_place() {
    let machine = Machine::new(OPENCODE);
    machine.write(
        OPENCODE_JSON,
        &json!({ "mcp": {
            "legacy": { "type": "remote", "url": "https://mcp.example.com/old-legacy", "headers": { "Authorization": header() } },
            "servers": {
                "nested": { "type": "remote", "disabled": true, "url": "https://mcp.example.com/old", "headers": { "Authorization": header() } },
            },
        }}),
    );
    let runner = fake(&machine);
    let built = plan_opencode_2(
        &machine,
        json!([
            { "name": "nested", "transport": "http", "url": "https://mcp.example.com/nested" },
            { "name": "legacy", "transport": "http", "url": "https://mcp.example.com/legacy" },
        ]),
    );

    assert_different_definitions(&built.plan, &["opencode:mcp:nested", "opencode:mcp:legacy"]);

    let result = apply_selectable_in(&machine, &runner, &registry_v2(), &built).await;
    assert_no_leaks(&result, &needles());

    assert_eq!(
        machine.json(OPENCODE_JSON),
        json!({ "mcp": {
            "legacy": { "type": "remote", "url": "https://mcp.example.com/legacy", "headers": { "Authorization": header() } },
            "servers": {
                "nested": { "type": "remote", "disabled": true, "url": "https://mcp.example.com/nested", "headers": { "Authorization": header() } },
            },
        }}),
        "the nested entry keeps `disabled` and `headers`; the legacy one stays where it is"
    );
}

/// The review is made, then the file changes: the apply decides on what it reads now.
#[tokio::test]
async fn a_changed_entry_is_resolved_when_the_apply_reads_it_again() {
    let machine = Machine::new(BOTH);
    machine.write(OMP_MCP, &existing_omp());
    let runner = fake(&machine);
    let built = machine.plan(&shared());
    let remote = ["omp:mcp:remote".to_owned()];

    // Still different, whatever else changed (the new env value is kept).
    let mut moved = existing_omp();
    moved["mcpServers"]["remote"]["headers"]["Authorization"] = json!("rotated");
    machine.write(OMP_MCP, &moved);
    let (result, _) = apply(&machine, &runner, &built.stored, &remote, true).await;
    assert_eq!(
        outcome_of(&result.expect("applied"), &remote[0]).outcome,
        SyncOutcome::Replaced
    );
    assert_eq!(
        machine.json(OMP_MCP)["mcpServers"]["remote"]["headers"],
        json!({ "Authorization": "rotated" })
    );

    // The same plan again: it is the shared definition now.
    let (result, _) = apply(&machine, &runner, &built.stored, &remote, true).await;
    assert_eq!(
        outcome_of(&result.expect("applied"), &remote[0]).outcome,
        SyncOutcome::AlreadyPresent
    );

    // Removed meanwhile: a plain add (a server that needs a value starts switched off).
    machine.write(OMP_MCP, &json!({ "mcpServers": {} }));
    let (result, _) = apply(&machine, &runner, &built.stored, &remote, true).await;
    assert_eq!(
        outcome_of(&result.expect("applied"), &remote[0]).outcome,
        SyncOutcome::Done
    );
    assert_eq!(
        machine.json(OMP_MCP)["mcpServers"]["remote"],
        json!({ "type": "http", "url": "https://mcp.example.com/mcp", "enabled": false })
    );
}

#[tokio::test]
async fn a_name_a_file_the_app_only_reads_now_has_is_a_conflict_and_nothing_is_written() {
    let machine = Machine::new(BOTH);
    machine.write(OMP_MCP, &existing_omp());
    let runner = fake(&machine);
    let built = machine.plan(&shared());
    let before = machine.text(OMP_MCP);
    let remote = ["omp:mcp:remote".to_owned()];

    machine.write(
        ".omp/agent/.mcp.json",
        &json!({ "mcpServers": { "remote": { "type": "http", "url": "https://layered.example.com/mcp" } } }),
    );
    let (result, _) = apply(&machine, &runner, &built.stored, &remote, true).await;
    let result = result.expect("applied");

    assert_eq!(outcome_of(&result, "omp:mcp:remote").outcome, SyncOutcome::Conflict);
    assert!(!result.ok);
    assert_eq!(machine.text(OMP_MCP), before);

    std::fs::write(machine.home().join(".omp/agent/.mcp.json"), "{ not json").unwrap();
    let (result, _) = apply(&machine, &runner, &built.stored, &remote, true).await;
    let result = result.expect("applied");
    let outcome = outcome_of(&result, "omp:mcp:remote");

    assert_eq!(outcome.outcome, SyncOutcome::Conflict);
    assert_eq!(outcome.notes, [SyncNote::ConfigUnreadable]);
    assert_eq!(machine.text(OMP_MCP), before);

    std::fs::remove_file(machine.home().join(".omp/agent/.mcp.json")).unwrap();
    std::fs::write(machine.home().join(OMP_MCP), "{ not json").unwrap();
    let (result, _) = apply(&machine, &runner, &built.stored, &remote, true).await;

    assert!(matches!(
        outcome_of(&result.expect("applied"), "omp:mcp:remote").outcome,
        SyncOutcome::Skipped {
            reason: SyncSkipReason::McpFileRefused { .. }
        }
    ));
}

#[test]
fn a_server_a_layer_also_defines_is_a_conflict_that_cannot_be_ticked() {
    let machine = Machine::new(BOTH);
    machine.write(OMP_MCP, &existing_omp());
    machine.write(
        ".omp/agent/.mcp.json",
        &json!({ "mcpServers": { "remote": { "type": "http", "url": "https://layered.example.com/mcp" } } }),
    );
    let built = machine.plan(&shared());

    assert_eq!(item(&built.plan, "omp:mcp:remote").status, SyncItemStatus::Conflict);
    assert_eq!(selectable(&built.plan), ["omp:mcp:github"], "only the new server");
}
