//! Import review of "Share setup": statuses against this machine, the exact steps, hostile bundles,
//! and nothing from the machine leaking into the review. Applying is in `sync_import_apply.rs`.

use crate::import_support;

use harness_business::model::{
    HarnessId, McpTransport, SyncBlockReason, SyncItemStatus, SyncMcpPreview, SyncNote, SyncStep,
};
use harness_business::sync::codec::decode_input;
use import_support::{
    BOTH, Machine, OMP_MARKETPLACES, OMP_MCP, OMP_PLUGINS, PI_SETTINGS, apply, argv, assert_refused, bundle, fake, ids,
    item, selectable, statuses, typical,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[tokio::test]
async fn an_empty_machine_blocks_every_harness_and_refuses_to_apply() {
    let machine = Machine::new(&[]);
    let runner = fake(&machine);
    let built = machine.plan(&typical());

    assert!(built.plan.sections.iter().all(|section| !section.installed));
    for (id, status) in statuses(&built.plan) {
        assert_eq!(
            status,
            SyncItemStatus::Blocked {
                reason: SyncBlockReason::HarnessNotInstalled
            },
            "{id}"
        );
    }
    assert!(selectable(&built.plan).is_empty());
    assert!(built.stored.selectable_ids().is_empty());

    let (result, events) = apply(&machine, &runner, &built.stored, &ids(&built.plan), true).await;
    assert_refused(result);
    assert!(events.is_empty());
    assert!(runner.seen().is_empty());
    assert_eq!(machine.text(OMP_MCP), None);
}

#[tokio::test]
async fn a_clean_machine_reviews_every_item_as_new_with_its_exact_steps() {
    let machine = Machine::new(BOTH);
    let built = machine.plan(&typical());
    let plan = &built.plan;

    assert_eq!(built.stored.id(), plan.plan_id);
    assert_eq!(plan.app.as_deref(), Some("0.15.0"));
    assert_eq!(
        plan.sections.iter().map(|section| section.harness).collect::<Vec<_>>(),
        [HarnessId::Omp, HarnessId::Pi]
    );
    assert_eq!(
        ids(plan),
        [
            "omp:marketplace:useful",
            "omp:plugin:harness-useful-skills@useful",
            "omp:plugin:npm-helper",
            "omp:mcp:github",
            "omp:mcp:remote",
            "pi:package:npm:@demo/pi-review-kit",
            "pi:mcp:docs",
        ]
    );
    assert!(statuses(plan).iter().all(|(_, status)| *status == SyncItemStatus::New));
    assert_eq!(selectable(plan), ids(plan));
    assert_eq!(built.stored.harnesses(), BOTH);

    let marketplace = item(plan, "omp:marketplace:useful");
    assert!(marketplace.selected_by_default);
    assert_eq!(marketplace.detail.as_deref(), Some("acme/useful-skills"));
    assert_eq!(
        marketplace.steps,
        [SyncStep::Cli {
            command: argv(&["omp", "plugin", "marketplace", "add", "acme/useful-skills"])
        }]
    );

    let plugin = item(plan, "omp:plugin:harness-useful-skills@useful");
    assert_eq!(plugin.depends_on.as_deref(), Some("omp:marketplace:useful"));
    assert_eq!(plugin.version.as_deref(), Some("0.2.36"));
    assert_eq!(
        plugin.notes,
        [SyncNote::VersionInformational, SyncNote::NeedsSharedMarketplace]
    );
    assert_eq!(
        plugin.steps,
        [SyncStep::Cli {
            command: argv(&[
                "omp",
                "plugin",
                "install",
                "harness-useful-skills@useful",
                "--scope",
                "user"
            ])
        }]
    );
    assert_eq!(item(plan, "omp:plugin:npm-helper").depends_on, None);

    let github = item(plan, "omp:mcp:github");
    assert!(github.runs_program);
    assert!(github.selectable);
    assert!(!github.selected_by_default, "stdio servers start unticked");
    assert_eq!(github.env_keys, ["GITHUB_TOKEN"]);
    assert_eq!(github.notes, [SyncNote::EnvValuesNotShared, SyncNote::WrittenDisabled]);
    assert_eq!(
        github.steps,
        [SyncStep::McpMerge {
            file: "~/.omp/agent/mcp.json".to_owned(),
            server: SyncMcpPreview {
                name: "github".to_owned(),
                transport: McpTransport::Stdio,
                command: Some("npx".to_owned()),
                args: argv(&["-y", "@modelcontextprotocol/server-github"]),
                url: None,
                enabled: false,
            },
        }]
    );

    let remote = item(plan, "omp:mcp:remote");
    assert!(!remote.runs_program);
    assert!(remote.selected_by_default);

    assert_eq!(
        item(plan, "pi:package:npm:@demo/pi-review-kit").steps,
        [SyncStep::Cli {
            command: argv(&["pi", "install", "npm:@demo/pi-review-kit@1.0.0"])
        }]
    );
    assert!(matches!(
        &item(plan, "pi:mcp:docs").steps[..],
        [SyncStep::McpMerge { file, server }] if file == "~/.pi/agent/mcp.json" && server.enabled
    ));

    // Every preview gets its own id.
    assert_ne!(machine.plan(&typical()).plan.plan_id, plan.plan_id);
}

#[tokio::test]
async fn statuses_compare_the_bundle_with_this_machine() {
    let machine = Machine::new(BOTH);
    machine.write(
        OMP_MARKETPLACES,
        &json!({ "marketplaces": [
            { "name": "useful", "sourceType": "github", "sourceUri": "someone-else/useful-skills" },
            { "name": "here", "sourceType": "github", "sourceUri": "acme/here" },
        ]}),
    );
    machine.write(
        OMP_PLUGINS,
        &json!({ "plugins": {
            "old@here": [{ "scope": "user", "version": "1.0.0" }],
            "same@here": [{ "scope": "user", "version": "2.0.0" }],
        }}),
    );
    machine.write(
        PI_SETTINGS,
        &json!({ "packages": ["npm:@demo/pi-review-kit@0.9.0", "git:github.com/demo/kit"] }),
    );

    let built = machine.plan(&bundle(json!({
        "omp": {
            "marketplaces": [{ "name": "useful", "source": "acme/useful-skills" }],
            "plugins": [
                { "id": "harness-useful-skills@useful", "version": "0.2.36" },
                { "id": "old@here", "version": "1.1.0" },
                { "id": "same@here", "version": "2.0.0" },
                { "id": "fresh@here" },
                { "id": "orphan@nowhere" },
            ],
            "packages": [{ "source": "npm:left-pad" }],
        },
        "pi": { "packages": [
            { "source": "npm:@demo/pi-review-kit@1.0.0" },
            { "source": "git:github.com/demo/kit" },
        ]},
    })));

    assert_eq!(
        statuses(&built.plan),
        [
            ("omp:marketplace:useful".to_owned(), SyncItemStatus::Conflict),
            (
                "omp:plugin:harness-useful-skills@useful".to_owned(),
                SyncItemStatus::Conflict
            ),
            (
                "omp:plugin:old@here".to_owned(),
                SyncItemStatus::DifferentVersion {
                    installed: "1.0.0".to_owned(),
                    shared: "1.1.0".to_owned()
                }
            ),
            ("omp:plugin:same@here".to_owned(), SyncItemStatus::AlreadyPresent),
            ("omp:plugin:fresh@here".to_owned(), SyncItemStatus::New),
            (
                "omp:plugin:orphan@nowhere".to_owned(),
                SyncItemStatus::Blocked {
                    reason: SyncBlockReason::MarketplaceMissing
                }
            ),
            (
                "omp:package:npm:left-pad".to_owned(),
                SyncItemStatus::Blocked {
                    reason: SyncBlockReason::NotSupported
                }
            ),
            (
                "pi:package:npm:@demo/pi-review-kit".to_owned(),
                SyncItemStatus::DifferentVersion {
                    installed: "npm:@demo/pi-review-kit@0.9.0".to_owned(),
                    shared: "npm:@demo/pi-review-kit@1.0.0".to_owned()
                }
            ),
            (
                "pi:package:git:github.com/demo/kit".to_owned(),
                SyncItemStatus::AlreadyPresent
            ),
        ]
    );
    assert_eq!(selectable(&built.plan), ["omp:plugin:fresh@here"]);

    let conflict = item(&built.plan, "omp:marketplace:useful");
    assert!(conflict.steps.is_empty());
    assert!(!conflict.selected_by_default);

    // The local marketplace source never reaches the review.
    let text = serde_json::to_string(&built.plan).unwrap();
    assert!(!text.contains("someone-else"));
}

#[tokio::test]
async fn git_packages_at_another_ref_are_another_version() {
    let machine = Machine::new(BOTH);
    machine.write(
        PI_SETTINGS,
        &json!({ "packages": [
            "git:github.com/demo/kit@feature/x",
            "https://github.com/demo/web.git#main",
            "git:git@github.com:demo/ssh@v1",
        ]}),
    );

    let built = machine.plan(&bundle(json!({ "pi": { "packages": [
        { "source": "git:github.com/demo/kit@feature/y" },
        { "source": "https://github.com/demo/web@v2" },
        { "source": "git:git@github.com:demo/ssh@v2" },
    ]}})));

    let different = |installed: &str, shared: &str| SyncItemStatus::DifferentVersion {
        installed: installed.to_owned(),
        shared: shared.to_owned(),
    };
    assert_eq!(
        statuses(&built.plan),
        [
            (
                "pi:package:git:github.com/demo/kit".to_owned(),
                different("git:github.com/demo/kit@feature/x", "git:github.com/demo/kit@feature/y")
            ),
            (
                "pi:package:https://github.com/demo/web".to_owned(),
                // An installed source the import would refuse is not displayed.
                different("unknown", "https://github.com/demo/web@v2")
            ),
            (
                "pi:package:git:git@github.com:demo/ssh".to_owned(),
                different("git:git@github.com:demo/ssh@v1", "git:git@github.com:demo/ssh@v2")
            ),
        ]
    );
}

#[tokio::test]
async fn hostile_bundles_are_invalid_and_never_reach_a_command_line() {
    let hostile_sources = [
        "--foo",
        "npm:x;rm -rf ~",
        "$(id)",
        "npm:`id`",
        "npm:x\ny",
        "../x",
        "/etc/passwd",
        "~/x",
        "file:..",
        "https://u:t@host",
        "git:tok@host/x",
        "npm:--registry=https://evil.example.com",
        "git:github.com/a/b@--upload-pack=x",
        "npm:pkg@file:../x",
        "npm:pkg@/abs/x",
        "npm:./x",
        "npm:pkg@http://evil.example.com/pkg.tgz",
    ];
    let machine = Machine::new(BOTH);
    let runner = fake(&machine);

    let packages: Vec<Value> = hostile_sources
        .iter()
        .map(|source| json!({ "source": source }))
        .chain([json!({ "source": "npm:fine-package" })])
        .collect();
    let plugins: Vec<Value> = hostile_sources.iter().map(|id| json!({ "id": id })).collect();
    let marketplaces: Vec<Value> = hostile_sources
        .iter()
        .map(|source| json!({ "name": "evil", "source": source }))
        .collect();
    let servers = json!([
        { "name": "a\u{202E}b", "transport": "http", "url": "https://mcp.example.com" },
        { "name": "sh", "transport": "stdio", "command": "/bin/sh" }, // guard-allow: installer-shell
        { "name": "bash", "transport": "stdio", "command": "bash.exe", "args": ["-c", "id"] }, // guard-allow: installer-shell
        { "name": "ftp", "transport": "http", "url": "ftp://mcp.example.com" },
    ]);

    let built = machine.plan(&bundle(json!({
        "omp": { "marketplaces": marketplaces, "plugins": plugins, "mcpServers": servers },
        "pi": { "packages": packages },
    })));

    let all = ids(&built.plan);
    let invalid: Vec<&String> = all.iter().filter(|id| *id != "pi:package:npm:fine-package").collect();
    for id in &invalid {
        assert!(
            matches!(item(&built.plan, id).status, SyncItemStatus::Invalid { .. }),
            "{id} is not invalid"
        );
        assert!(item(&built.plan, id).steps.is_empty());
    }
    assert_eq!(invalid.len(), hostile_sources.len() * 3 + 4);
    assert_eq!(built.plan.invalid_count as usize, invalid.len());
    assert_eq!(built.stored.selectable_ids(), ["pi:package:npm:fine-package"]);

    let (result, _) = apply(&machine, &runner, &built.stored, &all, true).await;
    assert_refused(result);
    assert!(runner.seen().is_empty());

    let (result, _) = apply(
        &machine,
        &runner,
        &built.stored,
        &argv(&["pi:package:npm:fine-package"]),
        true,
    )
    .await;
    assert!(result.expect("applied").ok);

    let seen = runner.seen();
    assert_eq!(seen, [argv(&["pi", "install", "npm:fine-package"])]);
    for value in hostile_sources {
        assert!(
            !seen.concat().iter().any(|arg| arg.contains(value)),
            "{value:?} reached a command line"
        );
    }

    // Too many items, or a huge string, reject the whole input: there is no plan to apply.
    let too_many: Vec<Value> = (0..201).map(|n| json!({ "source": format!("npm:p{n}") })).collect();
    assert!(decode_input(&bundle(json!({ "pi": { "packages": too_many } })).to_string()).is_err());
    let huge = "x".repeat(1024 * 1024);
    assert!(decode_input(&bundle(json!({ "pi": { "packages": [{ "source": huge }] } })).to_string()).is_err());
}
