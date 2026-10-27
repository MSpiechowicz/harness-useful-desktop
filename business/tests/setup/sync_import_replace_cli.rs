//! Replacing an MCP server of "Share setup" that exists with another definition, through the
//! harness's CLI (Claude Code's global file and Codex's `config.toml` are never written by the app):
//! `remove`, then `add` carrying the old entry's other settings over, the old entry added back when
//! the add fails, what a changed entry ends as, and no value read from the file in the plan or the
//! result (the values only go to the CLI). A recording fake answers for `claude` and `codex`.

use crate::import_support;

use harness_business::model::{HarnessId, SyncItemStatus, SyncOutcome, SyncStep};
use import_support::{
    CLAUDE_GLOBAL, CODEX_CONFIG, FakeCli, Machine, apply, apply_replaced, argv, assert_no_leaks, bundle, item,
    outcome_of, selectable,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const CLAUDE: &[HarnessId] = &[HarnessId::Claude];
const CODEX: &[HarnessId] = &[HarnessId::Codex];

const SEARCH: &str = "claude:mcp:search";
const FILES: &str = "codex:mcp:files";
const WEB: &str = "codex:mcp:web";

/// Values built at run time, so no source line looks like a credential.
fn secret() -> String {
    ["env", "-value-", "Z9mK"].concat()
}

fn header() -> String {
    ["Bearer ", "header", "-value-", "L3xT"].concat()
}

/// What of the existing entries must not show in a plan or a result.
fn needles() -> Vec<String> {
    ["/old", "--old", "evil", "old-cmd"]
        .map(str::to_owned)
        .into_iter()
        .chain([secret(), header()])
        .collect()
}

fn claude_bundle() -> Value {
    bundle(json!({ "claude": { "mcpServers": [
        { "name": "search", "transport": "http", "url": "https://search.example.com/mcp" },
    ]}}))
}

/// Claude Code's `search` server at another URL, with a header value and an `oauth` block.
fn claude_machine() -> Machine {
    let machine = Machine::new(CLAUDE);
    machine.write(
        CLAUDE_GLOBAL,
        &json!({ "numStartups": 3, "mcpServers": {
            "search": {
                "type": "http", "url": "https://search.example.com/old",
                "headers": { "Authorization": header() }, "oauth": { "clientId": "abc" },
            },
            "other": { "type": "stdio", "command": "other", "args": [] },
        }}),
    );

    machine
}

fn remove(name: &str) -> Vec<String> {
    argv(&["claude", "mcp", "remove", "--scope", "user", name])
}

fn add_json(name: &str, definition: &Value) -> Vec<String> {
    argv(&[
        "claude",
        "mcp",
        "add-json",
        "--scope",
        "user",
        name,
        &definition.to_string(),
    ])
}

fn user_servers(machine: &Machine) -> Value {
    machine.json(CLAUDE_GLOBAL)["mcpServers"].clone()
}

#[tokio::test]
async fn claude_removes_then_adds_the_entry_with_its_other_settings() {
    let machine = claude_machine();
    let runner = FakeCli::new(&machine);
    let built = machine.plan(&claude_bundle());
    let row = item(&built.plan, SEARCH);

    assert_eq!(row.status, SyncItemStatus::DifferentDefinition);
    assert!(row.selectable && !row.selected_by_default);
    assert_eq!(
        row.steps,
        [
            SyncStep::Cli {
                command: remove("search")
            },
            SyncStep::Cli {
                command: add_json(
                    "search",
                    &json!({ "type": "http", "url": "https://search.example.com/mcp" })
                )
            },
        ]
    );
    assert_no_leaks(&built.plan, &needles());

    let (result, _) = apply(&machine, &runner, &built.stored, &[SEARCH.to_owned()], true).await;
    let result = result.expect("applied");
    let done = outcome_of(&result, SEARCH);

    assert!(result.ok, "{result:?}");
    assert_eq!(done.outcome, SyncOutcome::Replaced);
    assert_eq!(
        done.argv,
        add_json(
            "search",
            &json!({ "type": "http", "url": "https://search.example.com/mcp" })
        ),
        "the result shows the reviewed command, with none of the entry's values"
    );
    assert_no_leaks(&result, &needles());

    let kept = json!({
        "type": "http", "url": "https://search.example.com/mcp",
        "headers": { "Authorization": header() }, "oauth": { "clientId": "abc" },
    });
    assert_eq!(runner.seen(), [remove("search"), add_json("search", &kept)]);
    assert_eq!(user_servers(&machine)["search"], kept);
    assert_eq!(
        user_servers(&machine)["other"],
        json!({ "type": "stdio", "command": "other", "args": [] })
    );
    assert_eq!(machine.json(CLAUDE_GLOBAL)["numStartups"], json!(3));
}

#[tokio::test]
async fn claude_puts_the_old_entry_back_when_the_add_fails() {
    let machine = claude_machine();
    let before = machine.json(CLAUDE_GLOBAL);
    let new = json!({
        "type": "http", "url": "https://search.example.com/mcp",
        "headers": { "Authorization": header() }, "oauth": { "clientId": "abc" },
    });
    let new_add = add_json("search", &new);
    let words: Vec<&str> = new_add.iter().map(String::as_str).collect();
    let runner = FakeCli::new(&machine).failing(&words);
    let built = machine.plan(&claude_bundle());

    let (result, _) = apply(&machine, &runner, &built.stored, &[SEARCH.to_owned()], true).await;
    let result = result.expect("applied");
    let failed = outcome_of(&result, SEARCH);

    assert!(!result.ok);
    assert_eq!(failed.outcome, SyncOutcome::Failed);
    assert_eq!(
        failed.argv,
        add_json(
            "search",
            &json!({ "type": "http", "url": "https://search.example.com/mcp" })
        )
    );
    assert_no_leaks(&result, &needles());

    let old = before["mcpServers"]["search"].clone();
    assert_eq!(runner.seen(), [remove("search"), new_add, add_json("search", &old)]);
    assert_eq!(user_servers(&machine)["search"], old, "the old entry is back");
}

#[tokio::test]
async fn claude_resolves_a_changed_entry_when_it_applies() {
    let machine = claude_machine();
    let runner = FakeCli::new(&machine);
    let built = machine.plan(&claude_bundle());

    // The shared definition now: nothing runs.
    machine.write(
        CLAUDE_GLOBAL,
        &json!({ "mcpServers": { "search": { "type": "http", "url": "https://search.example.com/mcp" } } }),
    );
    let (result, _) = apply(&machine, &runner, &built.stored, &[SEARCH.to_owned()], true).await;
    assert_eq!(
        outcome_of(&result.expect("applied"), SEARCH).outcome,
        SyncOutcome::AlreadyPresent
    );
    assert!(runner.seen().is_empty());

    // Gone meanwhile: a plain add.
    machine.write(CLAUDE_GLOBAL, &json!({ "mcpServers": {} }));
    let (result, _) = apply(&machine, &runner, &built.stored, &[SEARCH.to_owned()], true).await;
    assert_eq!(outcome_of(&result.expect("applied"), SEARCH).outcome, SyncOutcome::Done);
    assert_eq!(
        runner.seen(),
        [add_json(
            "search",
            &json!({ "type": "http", "url": "https://search.example.com/mcp" })
        )]
    );
}

/// Codex: the stdio `files` server has an `env`, the HTTP `web` one a bearer token variable, and
/// `docs` a field `codex mcp add` cannot carry over.
fn codex_machine() -> Machine {
    let machine = Machine::new(CODEX);
    let config = format!(
        "[mcp_servers.files]\ncommand = \"npx\"\nargs = [\"files-pkg\", \"--old\"]\nenv = {{ FILES_TOKEN = \"{}\" }}\n\n\
         [mcp_servers.web]\nurl = \"https://web.example.com/old\"\nbearer_token_env_var = \"WEB_TOKEN\"\n\n\
         [mcp_servers.docs]\ncommand = \"npx\"\nargs = [\"docs-pkg\"]\nstartup_timeout_sec = 30\n",
        secret()
    );
    std::fs::create_dir_all(machine.home().join(".codex")).unwrap();
    std::fs::write(machine.home().join(CODEX_CONFIG), config).unwrap();

    machine
}

fn codex_bundle() -> Value {
    bundle(json!({ "codex": { "mcpServers": [
        { "name": "files", "transport": "stdio", "command": "npx", "args": ["files-pkg", "--new"],
          "envKeys": ["FILES_TOKEN"] },
        { "name": "web", "transport": "http", "url": "https://web.example.com/mcp" },
        { "name": "docs", "transport": "stdio", "command": "npx", "args": ["docs-pkg", "--new"] },
    ]}}))
}

#[tokio::test]
async fn codex_carries_the_bearer_variable_of_a_url_and_adds_a_stdio_server_plain() {
    let machine = codex_machine();
    let runner = FakeCli::new(&machine);
    let built = machine.plan(&codex_bundle());

    assert_eq!(item(&built.plan, FILES).status, SyncItemStatus::DifferentDefinition);
    assert_eq!(item(&built.plan, WEB).status, SyncItemStatus::DifferentDefinition);
    assert_eq!(
        item(&built.plan, "codex:mcp:docs").status,
        SyncItemStatus::Conflict,
        "a timeout cannot be carried over by `codex mcp add`"
    );
    assert_eq!(selectable(&built.plan), [FILES, WEB]);
    assert!(
        built
            .plan
            .sections
            .iter()
            .flat_map(|section| &section.items)
            .all(|item| !item.selected_by_default)
    );
    assert_eq!(
        item(&built.plan, FILES).steps,
        [
            SyncStep::Cli {
                command: argv(&["codex", "mcp", "remove", "files"])
            },
            SyncStep::Cli {
                command: argv(&["codex", "mcp", "add", "files", "--", "npx", "files-pkg", "--new"])
            },
        ]
    );
    assert_no_leaks(&built.plan, &needles());

    let result = apply_replaced(&machine, &runner, &built, &[FILES, WEB]).await;
    assert_eq!(
        outcome_of(&result, FILES).argv,
        argv(&["codex", "mcp", "add", "files", "--", "npx", "files-pkg", "--new"]),
        "the reviewed command, without any value"
    );
    assert_no_leaks(&result, &needles());

    assert_eq!(
        runner.seen(),
        [
            argv(&["codex", "mcp", "remove", "files"]),
            argv(&["codex", "mcp", "add", "files", "--", "npx", "files-pkg", "--new"]),
            argv(&["codex", "mcp", "remove", "web"]),
            argv(&[
                "codex",
                "mcp",
                "add",
                "web",
                "--url",
                "https://web.example.com/mcp",
                "--bearer-token-env-var",
                "WEB_TOKEN"
            ]),
        ]
    );

    let servers = machine.toml(CODEX_CONFIG)["mcp_servers"].clone();
    assert_eq!(
        servers["files"],
        json!({ "command": "npx", "args": ["files-pkg", "--new"] }),
        "a stdio server never keeps the env of the old entry"
    );
    assert_eq!(
        servers["web"],
        json!({ "url": "https://web.example.com/mcp", "bearer_token_env_var": "WEB_TOKEN" })
    );
    assert_eq!(
        servers["docs"],
        json!({ "command": "npx", "args": ["docs-pkg"], "startup_timeout_sec": 30 }),
        "an entry that cannot be replaced is left alone"
    );
}

/// An entry `codex mcp add` could not write back is never replaced, whatever it points to: a failed
/// add could not be rolled back.
#[tokio::test]
async fn codex_will_not_replace_what_it_could_not_put_back() {
    let machine = codex_machine();
    let runner = FakeCli::new(&machine);
    let built = machine.plan(&codex_bundle());

    for config in [
        "[mcp_servers.web]\nurl = \"https://web.example.com/old\"\nenabled = false\n",
        "[mcp_servers.web]\nurl = \"https://evil.example.com/mcp\"\nstartup_timeout_sec = 30\n",
    ] {
        std::fs::write(machine.home().join(CODEX_CONFIG), config).unwrap();
        let (result, _) = apply(&machine, &runner, &built.stored, &[WEB.to_owned()], true).await;

        assert_eq!(
            outcome_of(&result.expect("applied"), WEB).outcome,
            SyncOutcome::Conflict
        );
        assert!(runner.seen().is_empty());
        assert_eq!(machine.text(CODEX_CONFIG).as_deref(), Some(config));
    }
}

#[tokio::test]
async fn claude_adds_the_fresh_entry_for_another_host_and_keeps_none_of_the_old_one() {
    let machine = Machine::new(CLAUDE);
    let old = json!({
        "type": "http", "url": "https://evil.example.com/mcp",
        "headers": { "Authorization": header() }, "oauth": { "clientId": "abc" },
    });
    machine.write(CLAUDE_GLOBAL, &json!({ "mcpServers": { "search": old } }));
    let fresh = json!({ "type": "http", "url": "https://search.example.com/mcp" });
    let fresh_add = add_json("search", &fresh);
    let words: Vec<&str> = fresh_add.iter().map(String::as_str).collect();
    let built = machine.plan(&claude_bundle());

    assert_eq!(item(&built.plan, SEARCH).status, SyncItemStatus::DifferentDefinition);

    let runner = FakeCli::new(&machine);
    let result = apply_replaced(&machine, &runner, &built, &[SEARCH]).await;

    assert_eq!(runner.seen(), [remove("search"), fresh_add.clone()]);
    assert_eq!(user_servers(&machine)["search"], fresh);
    assert_no_leaks(&result, &needles());

    // The add fails: the old entry is put back, and only then.
    machine.write(CLAUDE_GLOBAL, &json!({ "mcpServers": { "search": old } }));
    let failing = FakeCli::new(&machine).failing(&words);
    let (result, _) = apply(&machine, &failing, &built.stored, &[SEARCH.to_owned()], true).await;
    let result = result.expect("applied");

    assert_eq!(outcome_of(&result, SEARCH).outcome, SyncOutcome::Failed);
    assert_eq!(failing.seen(), [remove("search"), fresh_add, add_json("search", &old)]);
    assert_eq!(user_servers(&machine)["search"], old);
    assert_no_leaks(&result, &needles());
}

#[tokio::test]
async fn codex_replaces_the_whole_entry_for_another_host_or_command_it_can_write_back() {
    let machine = Machine::new(CODEX);
    let config = format!(
        "[mcp_servers.files]\ncommand = \"evil-files\"\nenv = {{ FILES_TOKEN = \"{}\" }}\n\n\
         [mcp_servers.web]\nurl = \"https://evil.example.com/mcp\"\nbearer_token_env_var = \"WEB_TOKEN\"\n\n\
         [mcp_servers.docs]\ncommand = \"evil-docs\"\nstartup_timeout_sec = 30\n",
        secret()
    );
    std::fs::create_dir_all(machine.home().join(".codex")).unwrap();
    std::fs::write(machine.home().join(CODEX_CONFIG), config).unwrap();
    let runner = FakeCli::new(&machine);
    let built = machine.plan(&codex_bundle());

    assert_eq!(
        selectable(&built.plan),
        [FILES, WEB],
        "docs holds a field `codex mcp add` cannot write back, so a failed add could not be rolled back"
    );
    assert_no_leaks(&built.plan, &needles());

    let ids = [FILES, WEB];
    let result = apply_replaced(&machine, &runner, &built, &ids).await;

    assert_no_leaks(&result, &needles());
    let seen: Vec<String> = runner.seen().iter().map(|command| command.join(" ")).collect();
    assert_eq!(
        seen,
        [
            "codex mcp remove files",
            "codex mcp add files -- npx files-pkg --new",
            "codex mcp remove web",
            "codex mcp add web --url https://web.example.com/mcp",
        ],
        "no --env, no bearer variable"
    );

    let servers = machine.toml(CODEX_CONFIG)["mcp_servers"].clone();
    assert_eq!(
        servers["files"],
        json!({ "command": "npx", "args": ["files-pkg", "--new"] })
    );
    assert_eq!(servers["web"], json!({ "url": "https://web.example.com/mcp" }));
    assert_eq!(
        servers["docs"],
        json!({ "command": "evil-docs", "startup_timeout_sec": 30 }),
        "left alone"
    );
}
