//! Replacing an MCP server of "Share setup" that exists at another endpoint (another host, scheme or
//! port, another command or package, another transport), in the files the app writes: nothing of
//! the old entry survives, so no credential set up for another server reaches the shared one. The
//! whole entry becomes what a new add writes (switched off when the bundle declares env or header
//! keys), at the same place; the review shows that entry, not the managed fields. Also: a commented
//! OpenCode file gets the text to paste, and a local path argument makes a server differ.

use crate::import_support;

use harness_business::model::{HarnessId, SyncItemStatus, SyncNote, SyncOutcome, SyncSkipReason, SyncStep};
use import_support::{BOTH, Machine, OMP_MCP, OPENCODE_JSON, OPENCODE_JSONC};
use import_support::{apply, apply_replaced, apply_selectable_in};
use import_support::{assert_different_definitions, assert_no_leaks, bundle, fake};
use import_support::{github_and_remote, header, item, keys, opencode_github_and_remote, plan_opencode_2};
use import_support::{outcome_of, registry_v2, secret, selectable};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const OPENCODE: &[HarnessId] = &[HarnessId::OpenCode];

/// What of the existing entries must not show in a plan or a result.
fn needles() -> Vec<String> {
    ["evil", "/work", "local-bridge", "oauth-client"]
        .map(str::to_owned)
        .into_iter()
        .chain([secret(), header()])
        .collect()
}

fn shared() -> Value {
    let mut servers = github_and_remote();
    servers.extend([
        json!({ "name": "pkg", "transport": "stdio", "command": "npx", "args": ["-y", "@acme/server", "--quiet"] }),
        json!({ "name": "bridge", "transport": "http", "url": "https://mcp.example.com/bridge" }),
        json!({ "name": "same-host", "transport": "http", "url": "https://mcp.example.com/other" }),
    ]);

    bundle(json!({ "omp": { "mcpServers": servers }}))
}

#[tokio::test]
async fn another_command_host_package_or_transport_replaces_the_whole_entry() {
    let machine = Machine::new(BOTH);
    machine.write(
        OMP_MCP,
        &json!({
            "theme": "dark",
            "mcpServers": {
                "github": { "command": "evil-cmd", "cwd": "/work", "env": { "GITHUB_TOKEN": secret() }, "enabled": true },
                "remote": { "type": "http", "url": "https://evil.example.com/mcp", "headers": { "Authorization": header() } },
                "pkg": { "command": "npx", "args": ["-y", "@evil/server"], "env": { "TOKEN": secret() } },
                "bridge": { "command": "local-bridge", "args": [], "env": { "TOKEN": secret() }, "enabled": true },
                "same-host": { "type": "http", "url": "https://mcp.example.com/mine", "headers": { "Authorization": header() } },
            },
        }),
    );
    let runner = fake(&machine);
    let built = machine.plan(&shared());
    let ids = ["omp:mcp:github", "omp:mcp:remote", "omp:mcp:pkg", "omp:mcp:bridge"];

    for id in ids {
        let row = item(&built.plan, id);

        assert_eq!(row.status, SyncItemStatus::DifferentDefinition, "{id}");
        assert!(row.selectable && !row.selected_by_default, "{id}");
        assert!(
            matches!(&row.steps[..], [SyncStep::McpMerge { .. }]),
            "{id}: the review shows the entry that will be written: {:?}",
            row.steps
        );
    }
    for id in ["omp:mcp:github", "omp:mcp:remote"] {
        assert!(item(&built.plan, id).notes.contains(&SyncNote::WrittenDisabled), "{id}");
    }
    assert!(
        matches!(
            &item(&built.plan, "omp:mcp:same-host").steps[..],
            [SyncStep::McpReplace { .. }]
        ),
        "the same host keeps what is there"
    );
    assert_no_leaks(&built.plan, &needles());

    let result = apply_replaced(&machine, &runner, &built, &ids).await;
    assert_no_leaks(&result, &needles());

    let written = machine.json(OMP_MCP);
    assert_eq!(
        keys(&written["mcpServers"]),
        ["github", "remote", "pkg", "bridge", "same-host"]
    );
    assert_eq!(
        written["mcpServers"]["github"],
        json!({ "command": "npx", "args": ["-y", "@modelcontextprotocol/server-github"], "enabled": false }),
        "a server that needs a value starts switched off, with no env or cwd of the old one"
    );
    assert_eq!(
        written["mcpServers"]["remote"],
        json!({ "type": "http", "url": "https://mcp.example.com/mcp", "enabled": false })
    );
    assert_eq!(
        written["mcpServers"]["pkg"],
        json!({ "command": "npx", "args": ["-y", "@acme/server", "--quiet"] }),
        "another package"
    );
    assert_eq!(
        written["mcpServers"]["bridge"],
        json!({ "type": "http", "url": "https://mcp.example.com/bridge" }),
        "stdio to http"
    );
    assert_eq!(
        written["mcpServers"]["same-host"]["headers"],
        json!({ "Authorization": header() }),
        "not selected, not touched"
    );
}

/// What a stdio server connects to hangs on its arguments, which a shared entry controls: the old
/// entry's env never stays, whatever the command and the package look like.
#[tokio::test]
async fn a_stdio_server_never_keeps_the_env_of_the_old_entry() {
    let machine = Machine::new(BOTH);
    let old = |args: Value| json!({ "command": "npx", "args": args, "env": { "TOKEN": secret() }, "enabled": true });
    machine.write(
        OMP_MCP,
        &json!({ "mcpServers": {
            "pkg": old(json!(["-y", "@acme/server", "--loud"])),
            "relay": old(json!(["-y", "mcp-remote", "https://evil.example.com/sse"])),
            "api": old(json!(["-y", "@acme/server", "--api-url=https://evil.example.com"])),
            "box": { "command": "docker", "args": ["run", "-i", "--rm", "-e", "TOKEN", "evil/image"],
                     "env": { "TOKEN": secret() } },
        }}),
    );
    let runner = fake(&machine);
    let servers = [
        json!({ "name": "pkg", "transport": "stdio", "command": "npx", "args": ["-y", "@acme/server", "--quiet"] }),
        json!({ "name": "relay", "transport": "stdio", "command": "npx",
                "args": ["-y", "mcp-remote", "https://good.example.com/sse"] }),
        json!({ "name": "api", "transport": "stdio", "command": "npx",
                "args": ["-y", "@acme/server", "--api-url=https://good.example.com"], "envKeys": ["TOKEN"] }),
        json!({ "name": "box", "transport": "stdio", "command": "docker",
                "args": ["run", "-i", "--rm", "acme/image"] }),
    ];
    let built = machine.plan(&bundle(json!({ "omp": { "mcpServers": servers } })));
    let ids = ["omp:mcp:pkg", "omp:mcp:relay", "omp:mcp:api", "omp:mcp:box"];

    assert_different_definitions(&built.plan, &ids);
    for id in ids {
        assert!(
            matches!(&item(&built.plan, id).steps[..], [SyncStep::McpMerge { .. }]),
            "{id}"
        );
    }

    let result = apply_replaced(&machine, &runner, &built, &ids).await;
    assert_no_leaks(&result, &needles());

    assert_eq!(
        machine.json(OMP_MCP)["mcpServers"],
        json!({
            "pkg": { "command": "npx", "args": ["-y", "@acme/server", "--quiet"] },
            "relay": { "command": "npx", "args": ["-y", "mcp-remote", "https://good.example.com/sse"] },
            "api": { "command": "npx", "args": ["-y", "@acme/server", "--api-url=https://good.example.com"],
                     "enabled": false },
            "box": { "command": "docker", "args": ["run", "-i", "--rm", "acme/image"] },
        })
    );
}

#[tokio::test]
async fn opencode_1_never_leaves_keys_of_the_other_type_on_a_switched_entry() {
    let machine = Machine::new(OPENCODE);
    machine.write(
        OPENCODE_JSON,
        &json!({ "mcp": {
            "github": { "type": "remote", "url": "https://evil.example.com/mcp", "enabled": true,
                        "headers": { "Authorization": header() }, "oauth": { "clientId": "oauth-client" } },
            "remote": { "type": "local", "command": ["local-bridge"], "enabled": true,
                        "environment": { "TOKEN": secret() }, "timeout": 5 },
        }}),
    );
    let runner = fake(&machine);
    let built = machine.plan(&opencode_github_and_remote());

    let result = apply_replaced(
        &machine,
        &runner,
        &built,
        &["opencode:mcp:github", "opencode:mcp:remote"],
    )
    .await;
    assert_no_leaks(&result, &needles());

    assert_eq!(
        machine.json(OPENCODE_JSON),
        json!({ "mcp": {
            "github": { "type": "local", "command": ["npx", "-y", "server-github"], "enabled": false },
            "remote": { "type": "remote", "url": "https://mcp.example.com/mcp", "enabled": true },
        }})
    );
}

#[tokio::test]
async fn opencode_2_replaces_the_whole_nested_or_legacy_entry_in_place() {
    let machine = Machine::new(OPENCODE);
    machine.write(
        OPENCODE_JSON,
        &json!({ "mcp": {
            "legacy": { "type": "remote", "url": "https://evil.example.com/legacy", "headers": { "Authorization": header() } },
            "servers": {
                "plain": { "type": "remote", "disabled": true, "url": "https://evil.example.com/mcp", "headers": { "Authorization": header() } },
                "needs": { "type": "local", "command": ["local-bridge"], "environment": { "TOKEN": secret() } },
            },
        }}),
    );
    let runner = fake(&machine);
    let built = plan_opencode_2(
        &machine,
        json!([
            { "name": "legacy", "transport": "http", "url": "https://mcp.example.com/legacy" },
            { "name": "plain", "transport": "http", "url": "https://mcp.example.com/plain" },
            { "name": "needs", "transport": "http", "url": "https://mcp.example.com/needs",
              "headerKeys": ["Authorization"] },
        ]),
    );
    let ids = ["opencode:mcp:legacy", "opencode:mcp:plain", "opencode:mcp:needs"];

    assert_different_definitions(&built.plan, &ids);

    let result = apply_selectable_in(&machine, &runner, &registry_v2(), &built).await;
    let written = machine.json(OPENCODE_JSON);
    assert_no_leaks(&result, &needles());
    assert_eq!(
        written,
        json!({ "mcp": {
            "legacy": { "type": "remote", "url": "https://mcp.example.com/legacy" },
            "servers": {
                "plain": { "type": "remote", "url": "https://mcp.example.com/plain" },
                "needs": { "type": "remote", "url": "https://mcp.example.com/needs", "disabled": true },
            },
        }}),
        "no `disabled`, `headers` or `environment` of the old entries; `disabled` when keys are declared"
    );
}

/// A commented OpenCode file is never rewritten: the same endpoint gets the fields to set, another
/// one the whole entry.
#[tokio::test]
async fn a_commented_file_gets_what_to_paste() {
    let machine = Machine::new(OPENCODE);
    let config = format!(
        "{{\n  // mine\n  \"mcp\": {{\n    \"near\": {{ \"type\": \"remote\", \"url\": \"https://mcp.example.com/old\", \
         \"headers\": {{ \"Authorization\": \"{}\" }} }},\n    \"far\": {{ \"type\": \"remote\", \"url\": \"https://evil.example.com/mcp\" }}\n  }}\n}}\n",
        header()
    );
    let path = machine.home().join(OPENCODE_JSONC);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, &config).unwrap();
    let runner = fake(&machine);
    let built = machine.plan(&bundle(json!({ "opencode": { "mcpServers": [
        { "name": "near", "transport": "http", "url": "https://mcp.example.com/new" },
        { "name": "far", "transport": "http", "url": "https://mcp.example.com/new" },
    ]}})));

    let (result, _) = apply(&machine, &runner, &built.stored, &selectable(&built.plan), true).await;
    let result = result.expect("applied");
    let file = "~/.config/opencode/opencode.jsonc".to_owned();

    assert_eq!(
        outcome_of(&result, "opencode:mcp:near").outcome,
        SyncOutcome::Skipped {
            reason: SyncSkipReason::ManualReplace {
                file: file.clone(),
                snippet: "\"type\": \"remote\",\n\"url\": \"https://mcp.example.com/new\"".to_owned(),
            }
        }
    );
    assert_eq!(
        outcome_of(&result, "opencode:mcp:far").outcome,
        SyncOutcome::Skipped {
            reason: SyncSkipReason::ManualEdit {
                file,
                snippet: format!(
                    "\"far\": {}",
                    serde_json::to_string_pretty(
                        &json!({ "type": "remote", "url": "https://mcp.example.com/new", "enabled": true })
                    )
                    .unwrap()
                ),
            }
        }
    );
    assert_eq!(machine.text(OPENCODE_JSONC).as_deref(), Some(config.as_str()));
    assert_no_leaks(&result, &needles());
}

/// A server exported without its local path arguments differs from the one a machine configured with
/// its own: it can be ticked and replaces the whole entry, whose env value is not kept.
#[tokio::test]
async fn a_local_path_argument_makes_a_server_differ_and_the_replace_drops_its_env() {
    let machine = Machine::new(BOTH);
    machine.write(
        OMP_MCP,
        &json!({ "mcpServers": { "figma": {
            "command": "npx",
            "args": ["-y", "figma-developer-mcp", "--stdio", "--image-dir", "/Users/x/Downloads"],
            "env": { "FIGMA_API_KEY": secret() },
        }}}),
    );
    let runner = fake(&machine);
    let built = machine.plan(&bundle(json!({ "omp": { "mcpServers": [
        { "name": "figma", "transport": "stdio", "command": "npx",
          "args": ["-y", "figma-developer-mcp", "--stdio"], "envKeys": ["FIGMA_API_KEY"] },
    ]}})));
    let row = item(&built.plan, "omp:mcp:figma");

    assert_eq!(row.status, SyncItemStatus::DifferentDefinition);
    assert!(row.selectable && !row.selected_by_default);
    assert!(row.notes.contains(&SyncNote::WrittenDisabled));

    let result = apply_replaced(&machine, &runner, &built, &["omp:mcp:figma"]).await;
    let written = machine.json(OMP_MCP);

    assert_no_leaks(&result, &needles());
    assert_eq!(
        written["mcpServers"]["figma"],
        json!({ "command": "npx", "args": ["-y", "figma-developer-mcp", "--stdio"], "enabled": false })
    );
    assert!(!written.to_string().contains(&secret()), "no env value is kept");

    let opencode = Machine::new(OPENCODE);
    opencode.write(
        OPENCODE_JSON,
        &json!({ "mcp": { "pkg": { "type": "local", "command": ["npx", "-y", "pkg", "--root=/home/me/x"] } } }),
    );
    let built = opencode.plan(&bundle(json!({ "opencode": { "mcpServers": [
        { "name": "pkg", "transport": "stdio", "command": "npx", "args": ["-y", "pkg"] },
    ]}})));

    assert_eq!(
        item(&built.plan, "opencode:mcp:pkg").status,
        SyncItemStatus::DifferentDefinition
    );
}
