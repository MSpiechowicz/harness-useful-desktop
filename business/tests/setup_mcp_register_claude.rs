//! The companion MCP servers in Claude Code's user scope: read from the `mcpServers` of its global
//! file, written only through `claude mcp add-json|remove --scope user` (a recording stand-in for
//! `claude`, never the real binary). The same decisions as for the other files: a user's entry is
//! never changed, a removed entry is not added back, an owned entry follows the program, and a
//! failed run is not retried.

mod claude_support;

use std::path::{Path, PathBuf};
use std::time::Duration;

use claude_support::{FakeClaude, binary, cli, read_object, stdio, write_json};
use harness_business::ecosystem::{
    Blocked, DesktopRegistration, McpFile, MemoryRegistration, Shape, mcp_target, register_dashboard_mcp_cli,
    register_desktop_mcp_cli, register_memory_mcp, register_memory_mcp_cli, unregister_memory_mcp_cli,
};
use harness_business::env::Env;
use harness_business::harness::HarnessCli;
use harness_business::harnesses::Claude;
use harness_business::model::{McpRefusal, McpState};
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ProcessRunner, ResolvedBinary};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// A home with Claude Code's global file at `~/.claude.json`.
struct Home {
    dir: tempfile::TempDir,
    env: Env,
}

impl Home {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let env = Env::for_tests(dir.path());

        Self { dir, env }
    }

    fn global(&self) -> PathBuf {
        self.dir.path().join(".claude.json")
    }

    fn file(&self) -> McpFile {
        mcp_target(&self.env, &Claude).expect("Claude Code has an MCP target")
    }

    fn memory(&self) -> PathBuf {
        self.dir.path().join(".local/bin/harness-memory")
    }

    fn servers(&self) -> Value {
        read_object(&self.global())
            .get("mcpServers")
            .cloned()
            .unwrap_or(Value::Null)
    }

    /// Writes an owned `harness-memory` entry of an older install and returns it.
    fn old_memory_entry(&self) -> Value {
        let old = stdio("/old/place/harness-memory");
        write_json(&self.global(), &json!({ "mcpServers": { "harness-memory": old } }));

        old
    }

    fn fake(&self) -> FakeClaude {
        FakeClaude::new(&self.global())
    }

    async fn memory_registration(&self, fake: &FakeClaude, recorded: bool) -> MemoryRegistration {
        let claude = binary(self.dir.path());
        register_memory_mcp_cli(cli(&self.env, fake, &claude), &self.file(), &self.memory(), recorded).await
    }
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn add_json(name: &str, definition: &Value) -> Vec<String> {
    ["mcp", "add-json", "--scope", "user", name]
        .iter()
        .map(|arg| (*arg).to_owned())
        .chain([definition.to_string()])
        .collect()
}

fn remove(name: &str) -> Vec<String> {
    ["mcp", "remove", "--scope", "user", name]
        .iter()
        .map(|arg| (*arg).to_owned())
        .collect()
}

#[test]
fn the_target_is_the_user_scope_of_the_global_file_also_under_claude_config_dir() {
    let home = Home::new();
    assert_eq!(
        home.file(),
        McpFile {
            path: home.global(),
            also_read: Vec::new(),
            shape: Shape::ClaudeCli,
        }
    );
    assert!(home.file().via_cli());

    let custom = home.dir.path().join("claude-work");
    let env = home.env.clone().with_var("CLAUDE_CONFIG_DIR", path_text(&custom));
    assert_eq!(mcp_target(&env, &Claude).unwrap().path, custom.join(".claude.json"));
}

#[tokio::test]
async fn the_dashboard_is_added_by_its_absolute_path_through_add_json() {
    let home = Home::new();
    // Joined part by part and with `.exe` on Windows, as the PATH search names the program it finds.
    let dashboard = home
        .dir
        .path()
        .join(".local")
        .join("bin")
        .join(format!("harness-dashboard{}", std::env::consts::EXE_SUFFIX));
    std::fs::create_dir_all(dashboard.parent().unwrap()).unwrap();
    std::fs::write(&dashboard, "never run").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dashboard, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let env = home
        .env
        .clone()
        .with_var("PATH", path_text(dashboard.parent().unwrap()));

    let fake = home.fake();
    let claude = binary(home.dir.path());
    let state = register_dashboard_mcp_cli(cli(&env, &fake, &claude), &home.file()).await;

    assert_eq!(
        state,
        McpState::Configured {
            file: "~/.claude.json".to_owned(),
            name: "harness-dashboard".to_owned()
        }
    );
    let entry = stdio(&path_text(&dashboard));
    assert_eq!(fake.argvs(), [add_json("harness-dashboard", &entry)]);
    assert_eq!(home.servers(), json!({ "harness-dashboard": entry }));

    let spec = &fake.specs()[0];
    assert_eq!(spec.program, claude.path);
    assert_eq!(spec.cwd.as_deref(), Some(home.dir.path()));
    assert_eq!(spec.timeout, Duration::from_secs(60));
    assert!(
        spec.env
            .iter()
            .any(|(name, value)| name == "DISABLE_UPDATES" && value == "1")
    );

    // Registered again: found, nothing runs.
    let again = register_dashboard_mcp_cli(cli(&env, &fake, &claude), &home.file()).await;
    assert!(matches!(again, McpState::Configured { .. }), "{again:?}");
    assert_eq!(fake.argvs().len(), 1);
}

#[tokio::test]
async fn the_memory_server_is_added_and_a_removed_one_stays_out() {
    let home = Home::new();
    let fake = home.fake();

    assert_eq!(home.memory_registration(&fake, false).await, MemoryRegistration::Added);
    let entry = stdio(&path_text(&home.memory()));
    assert_eq!(fake.argvs(), [add_json("harness-memory", &entry)]);
    assert_eq!(home.servers(), json!({ "harness-memory": entry }));

    write_json(&home.global(), &json!({ "mcpServers": {}, "numStartups": 3 }));
    let after_removal = home.fake();
    assert_eq!(
        home.memory_registration(&after_removal, true).await,
        MemoryRegistration::SkippedRemoved
    );
    assert!(after_removal.argvs().is_empty(), "nothing runs");
}

#[tokio::test]
async fn a_users_harness_memory_entry_is_never_changed() {
    let home = Home::new();
    let theirs = json!({ "command": "/opt/tools/memory-proxy", "args": ["serve"], "env": { "MODE": "x" } });
    let original = json!({ "mcpServers": { "harness-memory": theirs }, "projects": {} });
    write_json(&home.global(), &original);
    let text = std::fs::read_to_string(home.global()).unwrap();

    let fake = home.fake();
    assert_eq!(
        home.memory_registration(&fake, false).await,
        MemoryRegistration::Present
    );

    let claude = binary(home.dir.path());
    let removed = unregister_memory_mcp_cli(cli(&home.env, &fake, &claude), &home.file()).await;
    assert_eq!(removed, Ok(false));

    assert!(fake.argvs().is_empty(), "nothing runs");
    assert_eq!(std::fs::read_to_string(home.global()).unwrap(), text);
}

#[tokio::test]
async fn a_server_running_harness_memory_under_another_name_counts() {
    let home = Home::new();
    write_json(
        &home.global(),
        &json!({ "mcpServers": { "notes": { "command": "/somewhere/harness-memory", "args": ["mcp"] } } }),
    );

    let fake = home.fake();
    assert_eq!(
        home.memory_registration(&fake, false).await,
        MemoryRegistration::Present
    );
    assert!(fake.argvs().is_empty());
}

#[tokio::test]
async fn the_owned_entry_follows_the_server_and_is_removed_on_request() {
    let home = Home::new();
    let old = json!({ "type": "stdio", "command": "/old/place/harness-memory", "args": ["mcp"], "env": {} });
    write_json(&home.global(), &json!({ "mcpServers": { "harness-memory": old } }));

    let fake = home.fake();
    assert_eq!(home.memory_registration(&fake, true).await, MemoryRegistration::Updated);

    let new = json!({ "type": "stdio", "command": path_text(&home.memory()), "args": ["mcp"], "env": {} });
    assert_eq!(
        fake.argvs(),
        [remove("harness-memory"), add_json("harness-memory", &new)]
    );
    assert_eq!(home.servers(), json!({ "harness-memory": new }));

    let claude = binary(home.dir.path());
    let removed = unregister_memory_mcp_cli(cli(&home.env, &fake, &claude), &home.file()).await;
    assert_eq!(removed, Ok(true));
    assert_eq!(fake.argvs().last().unwrap(), &remove("harness-memory"));
    assert_eq!(home.servers(), json!({}));
}

#[tokio::test]
async fn a_failed_add_is_refused_once_without_a_retry() {
    let home = Home::new();
    let fake = home.fake();
    fake.fail("add-json");

    assert_eq!(
        home.memory_registration(&fake, false).await,
        MemoryRegistration::Refused(McpRefusal::CliFailed)
    );
    assert_eq!(fake.argvs().len(), 1, "one run, no retry");
    assert!(!home.global().exists(), "the app never writes the file");
}

#[tokio::test]
async fn a_failed_remove_is_refused_as_a_failed_cli_run_and_not_a_write_failure() {
    let home = Home::new();
    let entry = stdio(&path_text(&home.memory()));
    write_json(&home.global(), &json!({ "mcpServers": { "harness-memory": entry } }));

    let fake = home.fake();
    fake.fail("remove");

    let claude = binary(home.dir.path());
    let removed = unregister_memory_mcp_cli(cli(&home.env, &fake, &claude), &home.file()).await;

    assert_eq!(removed, Err(Blocked::Refused(McpRefusal::CliFailed)));
    assert_eq!(fake.argvs(), [remove("harness-memory")], "one run, no retry");
}

#[tokio::test]
async fn a_failed_change_puts_the_old_entry_back() {
    let home = Home::new();
    let old = home.old_memory_entry();

    let fake = home.fake();
    fake.fail("add-json");
    assert_eq!(
        home.memory_registration(&fake, true).await,
        MemoryRegistration::Refused(McpRefusal::CliFailed)
    );

    let new = stdio(&path_text(&home.memory()));
    assert_eq!(
        fake.argvs(),
        [
            remove("harness-memory"),
            add_json("harness-memory", &new),
            add_json("harness-memory", &old)
        ]
    );
    assert_eq!(home.servers(), json!({ "harness-memory": old }));
}

#[tokio::test]
async fn an_entry_that_appeared_meanwhile_is_reported_after_a_failed_add() {
    let home = Home::new();
    let fake = home.fake();

    // Another Claude Code adds the server between the read and the run.
    let claude = binary(home.dir.path());
    let racing = Racing {
        inner: &fake,
        global: home.global(),
        memory: home.memory(),
    };
    let outcome = register_memory_mcp_cli(
        cli_racing(&home.env, &racing, &claude),
        &home.file(),
        &home.memory(),
        false,
    )
    .await;

    assert_eq!(outcome, MemoryRegistration::Present);
}

/// Writes the server into the file before every run, then lets the fake run it (which refuses the
/// existing name).
struct Racing<'a> {
    inner: &'a FakeClaude,
    global: PathBuf,
    memory: PathBuf,
}

impl ProcessRunner for Racing<'_> {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, harness_business::Result<ExecOutput>> {
        write_json(
            &self.global,
            &json!({ "mcpServers": { "harness-memory": stdio(&path_text(&self.memory)) } }),
        );
        self.inner.run(spec)
    }
}

fn cli_racing<'a>(env: &'a Env, runner: &'a Racing<'_>, binary: &'a ResolvedBinary) -> HarnessCli<'a> {
    HarnessCli {
        harness: &Claude,
        env,
        runner,
        binary,
    }
}

#[tokio::test]
async fn the_app_is_added_once_and_a_removed_entry_stays_out_at_launch() {
    let home = Home::new();
    let program = home.dir.path().join("apps/harness-desktop");
    let fake = home.fake();
    let claude = binary(home.dir.path());

    let added = register_desktop_mcp_cli(cli(&home.env, &fake, &claude), &home.file(), &program, false, None).await;
    assert_eq!(added, DesktopRegistration::Added);
    let entry = stdio(&path_text(&program));
    assert_eq!(fake.argvs(), [add_json("harness-desktop", &entry)]);

    let present = register_desktop_mcp_cli(cli(&home.env, &fake, &claude), &home.file(), &program, true, None).await;
    assert_eq!(present, DesktopRegistration::Present);
    assert_eq!(fake.argvs().len(), 1);

    write_json(&home.global(), &json!({ "mcpServers": {} }));
    let skipped = register_desktop_mcp_cli(cli(&home.env, &fake, &claude), &home.file(), &program, true, None).await;
    assert_eq!(skipped, DesktopRegistration::SkippedRemoved);
    assert_eq!(fake.argvs().len(), 1);
}

/// A batch shim (`claude.cmd`) never gets the JSON argument, and an entry it could not add back
/// is never removed: nothing runs.
#[tokio::test]
async fn a_batch_script_claude_never_gets_the_json_argument() {
    let shim = |home: &Home| ResolvedBinary {
        path: home.dir.path().join("claude.cmd"),
        is_batch_script: true,
    };

    let home = Home::new();
    let fake = home.fake();
    let outcome =
        register_memory_mcp_cli(cli(&home.env, &fake, &shim(&home)), &home.file(), &home.memory(), false).await;

    assert_eq!(outcome, MemoryRegistration::Refused(McpRefusal::CliBatchShim));
    assert!(fake.argvs().is_empty(), "nothing runs");

    let home = Home::new();
    let old = home.old_memory_entry();
    let fake = home.fake();
    let outcome =
        register_memory_mcp_cli(cli(&home.env, &fake, &shim(&home)), &home.file(), &home.memory(), true).await;

    assert_eq!(outcome, MemoryRegistration::Refused(McpRefusal::CliBatchShim));
    assert!(fake.argvs().is_empty(), "not even the remove runs");
    assert_eq!(home.servers(), json!({ "harness-memory": old }));
}

#[tokio::test]
async fn an_entry_removed_and_not_added_back_is_refused_as_removed() {
    let home = Home::new();
    home.old_memory_entry();

    let fake = home.fake();
    fake.fail("add-json");
    fake.fail("add-json");

    assert_eq!(
        home.memory_registration(&fake, true).await,
        MemoryRegistration::Refused(McpRefusal::CliRemovedEntry)
    );
    assert_eq!(fake.argvs().len(), 3, "remove, add, add back; no retry");
    assert_eq!(home.servers(), json!({}), "the entry is gone");
}

#[test]
fn the_file_path_never_writes_claudes_global_file() {
    let home = Home::new();
    let text = json!({ "mcpServers": { "other": stdio("/x/other") }, "userID": "u" }).to_string();
    std::fs::write(home.global(), &text).unwrap();

    let outcome = register_memory_mcp(&home.file(), &home.memory(), false);

    assert_eq!(outcome, MemoryRegistration::Refused(McpRefusal::WriteFailed));
    assert_eq!(std::fs::read_to_string(home.global()).unwrap(), text);
}
