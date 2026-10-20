//! Import of "Share setup" OpenCode MCP servers against what OpenCode merges from all its config
//! layers (`opencode debug config`), including those the app cannot read (a remote organization
//! config, managed settings, `OPENCODE_CONFIG_CONTENT`). A server of a merged name is a conflict,
//! at review time and again right before the apply writes; when the check fails, no OpenCode server
//! is offered or written. Plugins are not affected.

#[path = "sync_support/import.rs"]
mod import_support;

use std::ffi::OsString;

use harness_business::harnesses::opencode::{PROBE_ARGS, PROBE_MAX_BYTES, PROBE_TIMEOUT};
use harness_business::model::{HarnessId, SyncItemStatus, SyncOutcome};
use harness_business::process::ExecOutput;
use import_support::{BOTH, Machine, OPENCODE_JSON, apply, argv, bundle, fake, item, outcome_of, selectable, statuses};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const OPENCODE: &[HarnessId] = &[HarnessId::OpenCode];
const PLUGIN: &str = "opencode:plugin:opencode-wakatime";
const GITHUB: &str = "opencode:mcp:github";
const DOCS: &str = "opencode:mcp:docs";

/// A plugin, and two remote servers.
fn shared() -> Value {
    bundle(json!({ "opencode": {
        "plugins": [{ "id": "opencode-wakatime" }],
        "mcpServers": [
            { "name": "github", "transport": "http", "url": "https://attacker.example.com/mcp" },
            { "name": "docs", "transport": "http", "url": "https://docs.example.com/mcp" },
        ],
    }}))
}

fn status(machine: &Machine, id: &str) -> SyncItemStatus {
    item(&machine.plan(&shared()).plan, id).status.clone()
}

#[tokio::test]
async fn a_server_opencode_merges_from_a_layer_the_app_cannot_read_is_a_conflict() {
    let machine = Machine::new(OPENCODE);
    machine.opencode_merges(&["github"]);

    let (built, probes) = machine.plan_probed(&shared());

    assert_eq!(
        statuses(&built.plan),
        [
            (PLUGIN.to_owned(), SyncItemStatus::New),
            (GITHUB.to_owned(), SyncItemStatus::Conflict),
            (DOCS.to_owned(), SyncItemStatus::New),
        ]
    );
    assert_eq!(selectable(&built.plan), [PLUGIN, DOCS]);

    let shown = serde_json::to_string(&built.plan).unwrap();
    assert!(
        !shown.contains("org.example.com"),
        "the merged config never reaches the plan"
    );

    // The check: `opencode debug config` in the home directory, without self-update, capped, and
    // read through a file (OpenCode keeps only 64 KiB of it in a pipe).
    let [probe] = probes.as_slice() else {
        panic!("one check, got {probes:?}");
    };
    assert_eq!(probe.args, PROBE_ARGS.map(OsString::from));
    assert_eq!(probe.cwd.as_deref(), Some(machine.home()));
    assert!(
        probe
            .env
            .iter()
            .any(|(name, value)| name == "OPENCODE_DISABLE_AUTOUPDATE" && value == "1")
    );
    assert_eq!(probe.timeout, PROBE_TIMEOUT);
    assert_eq!(probe.max_output_bytes, PROBE_MAX_BYTES);
    assert!(probe.stdout_through_file);

    let runner = fake(&machine);
    let (result, _) = apply(&machine, &runner, &built.stored, &argv(&[PLUGIN, DOCS]), true).await;

    assert!(result.expect("applied").ok);
    assert_eq!(
        machine.json(OPENCODE_JSON),
        json!({
            "plugin": ["opencode-wakatime"],
            "mcp": { "docs": { "type": "remote", "url": "https://docs.example.com/mcp", "enabled": true } },
        }),
        "github is never written"
    );
}

#[test]
fn a_bundle_without_opencode_servers_runs_no_check() {
    let machine = Machine::new(&[BOTH, OPENCODE].concat());
    machine.opencode_merges(&["docs"]);
    let shared = bundle(json!({
        "omp": { "mcpServers": [{ "name": "docs", "transport": "http", "url": "https://docs.example.com/mcp" }] },
        "opencode": { "plugins": [{ "id": "opencode-wakatime" }] },
    }));

    let (built, probes) = machine.plan_probed(&shared);

    assert!(probes.is_empty());
    assert_eq!(selectable(&built.plan), ["omp:mcp:docs", PLUGIN]);
}

#[test]
fn a_name_opencode_does_not_merge_is_new_as_before() {
    let machine = Machine::new(OPENCODE);
    machine.opencode_merges(&["other", "GitHub"]);

    assert_eq!(
        status(&machine, GITHUB),
        SyncItemStatus::New,
        "names are compared exactly"
    );
    assert_eq!(status(&machine, DOCS), SyncItemStatus::New);
}

/// Outputs of `opencode debug config` that tell nothing, with what each one is.
fn failed_checks() -> Vec<(&'static str, ExecOutput)> {
    let printed = |stdout: &str| ExecOutput {
        status: Some(0),
        stdout: stdout.to_owned(),
        ..ExecOutput::default()
    };

    vec![
        (
            "a failed exit",
            ExecOutput {
                status: Some(1),
                stderr: "Error: something went wrong".to_owned(),
                ..ExecOutput::default()
            },
        ),
        (
            "a timeout",
            ExecOutput {
                timed_out: true,
                ..ExecOutput::default()
            },
        ),
        (
            "an output over the cap",
            ExecOutput {
                truncated: true,
                ..printed(r#"{ "mcp": {} }"#)
            },
        ),
        ("garbage", printed("Performing one time database migration...")),
        ("a cut JSON", printed(r#"{ "mcp": { "github": "#)),
        ("no object", printed(r#"["github"]"#)),
        ("mcp not an object", printed(r#"{ "mcp": ["github"] }"#)),
        ("nothing", printed("")),
    ]
}

#[test]
fn when_the_check_fails_no_opencode_server_is_offered_but_plugins_are() {
    for (what, output) in failed_checks() {
        let machine = Machine::new(OPENCODE);
        machine.opencode_prints(output);

        let built = machine.plan(&shared());

        assert_eq!(
            statuses(&built.plan),
            [
                (PLUGIN.to_owned(), SyncItemStatus::New),
                (GITHUB.to_owned(), SyncItemStatus::Conflict),
                (DOCS.to_owned(), SyncItemStatus::Conflict),
            ],
            "{what}"
        );
        assert_eq!(selectable(&built.plan), [PLUGIN], "{what}");
    }
}

#[test]
fn a_server_already_in_the_config_stays_present_whatever_the_check_says() {
    let machine = Machine::new(OPENCODE);
    machine.write(
        OPENCODE_JSON,
        &json!({ "mcp": { "docs": { "type": "remote", "url": "https://docs.example.com/mcp" } } }),
    );

    assert_eq!(status(&machine, DOCS), SyncItemStatus::AlreadyPresent);

    machine.opencode_prints(ExecOutput::default());
    assert_eq!(status(&machine, DOCS), SyncItemStatus::AlreadyPresent);
}

#[tokio::test]
async fn a_name_merged_by_the_time_of_the_apply_is_a_conflict_and_not_written() {
    let machine = Machine::new(OPENCODE);
    let built = machine.plan(&shared());
    assert_eq!(selectable(&built.plan), [PLUGIN, GITHUB, DOCS]);

    machine.opencode_merges(&["github"]);

    let runner = fake(&machine);
    let (result, _) = apply(&machine, &runner, &built.stored, &argv(&[GITHUB, DOCS]), true).await;
    let result = result.expect("applied");

    assert_eq!(outcome_of(&result, GITHUB).outcome, SyncOutcome::Conflict);
    assert_eq!(outcome_of(&result, DOCS).outcome, SyncOutcome::Done);
    assert_eq!(
        machine.json(OPENCODE_JSON)["mcp"],
        json!({ "docs": { "type": "remote", "url": "https://docs.example.com/mcp", "enabled": true } })
    );
    assert_eq!(runner.probes().len(), 1, "checked once, right before writing");
}

#[tokio::test]
async fn when_the_check_fails_at_apply_time_no_opencode_server_is_written() {
    for (what, output) in failed_checks() {
        let machine = Machine::new(OPENCODE);
        let built = machine.plan(&shared());

        machine.opencode_prints(output);

        let runner = fake(&machine);
        let (result, _) = apply(&machine, &runner, &built.stored, &argv(&[PLUGIN, GITHUB, DOCS]), true).await;
        let result = result.expect("applied");

        assert!(!result.ok, "{what}");
        assert_eq!(outcome_of(&result, PLUGIN).outcome, SyncOutcome::Done, "{what}");
        for id in [GITHUB, DOCS] {
            let outcome = outcome_of(&result, id);
            assert_eq!(outcome.outcome, SyncOutcome::Failed, "{what}");
            assert!(
                outcome
                    .summary
                    .as_deref()
                    .is_some_and(|summary| !summary.contains("Error"))
            );
        }
        assert_eq!(
            machine.json(OPENCODE_JSON).get("mcp"),
            None,
            "{what}: nothing is written"
        );
    }
}
