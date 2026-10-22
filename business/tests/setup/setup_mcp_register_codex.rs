//! The companion MCP servers in Codex's `config.toml`: read from its `mcp_servers` table, written
//! only through `codex mcp add|remove` (a recording stand-in for `codex`, never the real binary).
//! The same decisions as for the other files: a user's entry is never changed, a removed entry is
//! not added back, an owned entry follows the program, a failed run is not retried, and an entry
//! the CLI cannot write as it is (its `env` values, its headers) is never rewritten.

use crate::codex_support;
use crate::setup_support::dashboard_on_path;

use std::path::{Path, PathBuf};
use std::time::Duration;

use codex_support::{FakeCodex, add_stdio, binary, cli, read_toml, remove, stdio, write_config};
use harness_business::ecosystem::{
    Blocked, DesktopRegistration, McpFile, MemoryRegistration, Shape, mcp_state, mcp_target,
    register_dashboard_mcp_cli, register_desktop_mcp_cli, register_memory_mcp_cli, unregister_memory_mcp_cli,
};
use harness_business::env::Env;
use harness_business::harnesses::Codex;
use harness_business::model::{McpRefusal, McpState};
use harness_business::process::ResolvedBinary;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// A home with Codex's settings at `~/.codex/config.toml`, its detected `codex` and the recording
/// stand-in that answers for it.
struct Home {
    dir: tempfile::TempDir,
    env: Env,
    codex: ResolvedBinary,
    fake: FakeCodex,
}

impl Home {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let fake = FakeCodex::new(&dir.path().join(".codex/config.toml"));

        Self {
            env: Env::for_tests(dir.path()),
            codex: binary(dir.path()),
            fake,
            dir,
        }
    }

    fn config(&self) -> PathBuf {
        self.dir.path().join(".codex/config.toml")
    }

    /// Writes `text` as the settings file.
    fn write(&self, text: &str) {
        std::fs::create_dir_all(self.config().parent().unwrap()).unwrap();
        std::fs::write(self.config(), text).unwrap();
    }

    fn file(&self) -> McpFile {
        mcp_target(&self.env, &Codex).expect("Codex has an MCP target")
    }

    fn memory(&self) -> String {
        text(&self.dir.path().join(".local/bin/harness-memory"))
    }

    fn servers(&self) -> Value {
        read_toml(&self.config())
            .get("mcp_servers")
            .cloned()
            .unwrap_or(Value::Null)
    }

    /// The commands the stand-in ran so far.
    fn ran(&self) -> Vec<Vec<String>> {
        self.fake.argvs()
    }

    async fn dashboard(&self, env: &Env) -> McpState {
        register_dashboard_mcp_cli(cli(env, &self.fake, &self.codex), &self.file()).await
    }

    async fn register_memory(&self, recorded: bool) -> MemoryRegistration {
        let memory = PathBuf::from(self.memory());

        register_memory_mcp_cli(cli(&self.env, &self.fake, &self.codex), &self.file(), &memory, recorded).await
    }

    async fn unregister_memory(&self) -> Result<bool, Blocked> {
        unregister_memory_mcp_cli(cli(&self.env, &self.fake, &self.codex), &self.file()).await
    }

    async fn desktop(&self, program: &Path, recorded: bool) -> DesktopRegistration {
        register_desktop_mcp_cli(
            cli(&self.env, &self.fake, &self.codex),
            &self.file(),
            program,
            recorded,
            None,
        )
        .await
    }
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[test]
fn the_target_is_config_toml_in_the_codex_home_also_under_codex_home() {
    let home = Home::new();
    let expected = McpFile {
        path: home.config(),
        also_read: Vec::new(),
        shape: Shape::CodexCli,
    };
    assert_eq!(home.file(), expected);
    assert!(home.file().via_cli());

    let custom = home.dir.path().join("codex-work");
    let env = home.env.clone().with_var("CODEX_HOME", text(&custom));
    assert_eq!(mcp_target(&env, &Codex).unwrap().path, custom.join("config.toml"));
}

#[tokio::test]
async fn the_dashboard_is_added_by_its_absolute_path_through_mcp_add() {
    let home = Home::new();
    let (dashboard, env) = dashboard_on_path(home.dir.path(), &home.env);
    write_config(&home.config(), &json!({ "model": "gpt-5" }));

    let configured = McpState::Configured {
        file: "~/.codex/config.toml".to_owned(),
        name: "harness-dashboard".to_owned(),
    };
    assert_eq!(home.dashboard(&env).await, configured);
    assert_eq!(home.ran(), [add_stdio("harness-dashboard", &text(&dashboard))]);
    assert_eq!(home.servers(), json!({ "harness-dashboard": stdio(&text(&dashboard)) }));
    assert_eq!(
        read_toml(&home.config())["model"],
        "gpt-5",
        "the rest of the file is the CLI's"
    );

    let spec = &home.fake.specs()[0];
    assert_eq!(
        (&spec.program, spec.cwd.as_deref(), spec.timeout),
        (&home.codex.path, Some(home.dir.path()), Duration::from_secs(60))
    );

    // Registered again: found, nothing runs.
    assert_eq!(home.dashboard(&env).await, configured);
    assert_eq!(home.ran().len(), 1);
}

#[tokio::test]
async fn the_memory_server_is_added_and_a_removed_one_stays_out() {
    let home = Home::new();

    assert_eq!(home.register_memory(false).await, MemoryRegistration::Added);
    assert_eq!(home.ran(), [add_stdio("harness-memory", &home.memory())]);
    assert_eq!(home.servers(), json!({ "harness-memory": stdio(&home.memory()) }));

    // The user took it out again: a launch leaves it out.
    write_config(&home.config(), &json!({ "model": "gpt-5" }));
    assert_eq!(home.register_memory(true).await, MemoryRegistration::SkippedRemoved);
    assert_eq!(home.ran().len(), 1, "nothing more runs");
}

#[tokio::test]
async fn a_users_harness_memory_entry_is_never_changed() {
    let home = Home::new();
    let config = "# My servers\n[mcp_servers.harness-memory]\ncommand = \"/opt/tools/memory-proxy\"\nargs = [\"serve\"]\nenv = { MODE = \"x\" }\n";
    home.write(config);

    assert_eq!(home.register_memory(false).await, MemoryRegistration::Present);
    assert_eq!(home.unregister_memory().await, Ok(false));

    assert!(home.ran().is_empty(), "nothing runs");
    assert_eq!(std::fs::read_to_string(home.config()).unwrap(), config);
}

#[tokio::test]
async fn the_owned_entry_follows_the_server_and_is_removed_on_request() {
    let home = Home::new();
    let old = stdio("/old/place/harness-memory");
    write_config(&home.config(), &json!({ "mcp_servers": { "harness-memory": old } }));

    assert_eq!(home.register_memory(true).await, MemoryRegistration::Updated);
    assert_eq!(
        home.ran(),
        [remove("harness-memory"), add_stdio("harness-memory", &home.memory())]
    );
    assert_eq!(home.servers(), json!({ "harness-memory": stdio(&home.memory()) }));

    assert_eq!(home.unregister_memory().await, Ok(true));
    assert_eq!(home.ran().last().unwrap(), &remove("harness-memory"));
    assert_eq!(home.servers(), json!({}));
}

#[tokio::test]
async fn an_owned_entry_holding_env_values_is_never_rewritten() {
    let home = Home::new();
    let token = format!("{}{}", "sk-proj-", "a".repeat(24));
    let config = format!(
        "[mcp_servers.harness-memory]\ncommand = \"/old/place/harness-memory\"\nargs = [\"mcp\"]\nenv = {{ API_TOKEN = \"{token}\" }}\n"
    );
    home.write(&config);

    assert_eq!(
        home.register_memory(true).await,
        MemoryRegistration::Refused(McpRefusal::CliFailed)
    );

    assert!(
        home.ran().is_empty(),
        "nothing runs, so the secret never reaches a command line"
    );
    assert_eq!(std::fs::read_to_string(home.config()).unwrap(), config);
}

#[tokio::test]
async fn a_failed_add_is_refused_once_without_a_retry() {
    let home = Home::new();
    home.fake.fail("add");

    assert_eq!(
        home.register_memory(false).await,
        MemoryRegistration::Refused(McpRefusal::CliFailed)
    );
    assert_eq!(home.ran().len(), 1, "one run, no retry");
    assert!(!home.config().exists(), "the app never writes the file");
}

#[tokio::test]
async fn a_failed_remove_is_refused_as_a_failed_cli_run() {
    let home = Home::new();
    let entry = stdio(&home.memory());
    write_config(&home.config(), &json!({ "mcp_servers": { "harness-memory": entry } }));
    home.fake.fail("remove");

    assert_eq!(
        home.unregister_memory().await,
        Err(Blocked::Refused(McpRefusal::CliFailed))
    );
    assert_eq!(home.ran(), [remove("harness-memory")], "one run, no retry");
}

#[tokio::test]
async fn the_app_is_added_once_and_a_removed_entry_stays_out_at_launch() {
    let home = Home::new();
    let program = home.dir.path().join("apps/harness-desktop");

    assert_eq!(home.desktop(&program, false).await, DesktopRegistration::Added);
    assert_eq!(home.ran(), [add_stdio("harness-desktop", &text(&program))]);

    assert_eq!(home.desktop(&program, true).await, DesktopRegistration::Present);

    write_config(&home.config(), &json!({ "mcp_servers": {} }));
    assert_eq!(home.desktop(&program, true).await, DesktopRegistration::SkippedRemoved);
    assert_eq!(home.ran().len(), 1, "only the first registration ran");
}

#[test]
fn a_disabled_dashboard_server_is_reported_as_disabled() {
    let home = Home::new();
    home.write("[mcp_servers.harness-dashboard]\ncommand = \"harness-dashboard\"\nargs = [\"mcp\"]\nenabled = false\n");

    let state = mcp_state(&home.env, &home.file());

    assert!(matches!(state, McpState::Disabled { .. }), "{state:?}");
}

#[test]
fn an_unparsable_config_is_refused() {
    let home = Home::new();
    home.write("model = \n");

    assert_eq!(
        mcp_state(&home.env, &home.file()),
        McpState::Refused {
            file: "~/.codex/config.toml".to_owned(),
            reason: McpRefusal::Unparsable
        }
    );
}
