//! The MCP server names OpenCode 2 merges from all its config layers, asked with `opencode api
//! --standalone GET /api/config` (a private server, never the background service) and read from
//! the config sources it prints (a fake `opencode` script, never the real one).
#![cfg(unix)]

mod fake_cli_support;

use fake_cli_support::FakeCli;
use harness_business::BusinessError;
use harness_business::env::Env;
use harness_business::harness::OpenCodeGeneration;
use harness_business::harnesses::opencode::merged_mcp_names;
use harness_business::process::TokioRunner;
use pretty_assertions::assert_eq;

/// A fake `opencode` that records its arguments and working directory and prints `printed`.
fn fake(printed: &str, status: u8) -> FakeCli {
    let cli = FakeCli::script(
        "opencode",
        &format!(
            "printf '%s\\n' \"$@\" > \"$d/argv\"\npwd > \"$d/cwd\"\n\
             printf '%s' \"$OPENCODE_DISABLE_AUTOUPDATE\" > \"$d/autoupdate\"\n\
             printf '%s %s %s' \"$OPENCODE_DISABLE_MODELS_FETCH\" \"$OPENCODE_FILEWATCHER_DISABLE\" \
             \"$OPENCODE_DISABLE_FILEWATCHER\" > \"$d/quiet\"\n\
             printf '%s %s' \"$OPENCODE_CONFIG_PROJECT_DISABLE\" \"$OPENCODE_DISABLE_PROJECT_CONFIG\" > \"$d/project\"\n\
             cat \"$d/printed\"\nexit {status}\n"
        ),
    );
    cli.place("printed", printed);
    cli
}

async fn names(cli: &FakeCli) -> harness_business::Result<Vec<String>> {
    let env = Env::for_tests(cli.home());
    let found = merged_mcp_names(&env, &TokioRunner, &cli.binary, OpenCodeGeneration::V2).await?;

    Ok(found.into_iter().collect())
}

#[tokio::test]
async fn the_names_of_every_document_are_read_through_a_private_server() {
    let printed = r#"[
      {"type": "document", "path": "/h/.config/opencode/opencode.json", "info": {"mcp": {"servers": {
        "github": {"type": "local", "command": ["npx"], "environment": {"T": "x"}},
        "off": {"type": "local", "command": ["y"], "disabled": true}
      }}}},
      {"type": "directory", "path": "/h/.config/opencode"},
      {"type": "document", "info": {"mcp": {"servers": {"org": {"type": "remote", "url": "https://org.example.com"}}}}}
    ]"#;
    let cli = fake(printed, 0);

    assert_eq!(names(&cli).await.expect("read"), ["github", "off", "org"]);
    assert_eq!(
        cli.recorded("argv").as_deref(),
        Some("api\n--standalone\nGET\n/api/config\n")
    );
    assert_eq!(
        cli.recorded("cwd")
            .map(|cwd| std::fs::canonicalize(cwd.trim()).expect("cwd")),
        Some(std::fs::canonicalize(cli.home()).expect("home"))
    );
    assert_eq!(cli.recorded("autoupdate").as_deref(), Some("1"));
    assert_eq!(cli.recorded("quiet").as_deref(), Some("1 1 1"));
    assert_eq!(cli.recorded("project").as_deref(), Some("1 1"));
}

/// Set in the child run of [`an_inherited_project_config_switch_is_overridden`].
const CHILD_MARKER: &str = "HARNESS_PROBE_ENV_CHILD";

/// The app may not change its own environment (`unsafe` is forbidden), so the test runs itself
/// again with the switches turned back on in the inherited environment.
#[tokio::test]
async fn an_inherited_project_config_switch_is_overridden() {
    let name = "an_inherited_project_config_switch_is_overridden";

    if std::env::var_os(CHILD_MARKER).is_none() {
        let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .args(["--exact", name, "--test-threads=1"])
            .env(CHILD_MARKER, "1")
            .env("OPENCODE_CONFIG_PROJECT_DISABLE", "0")
            .env("OPENCODE_DISABLE_PROJECT_CONFIG", "false")
            .env("OPENCODE_FILEWATCHER_DISABLE", "")
            .output()
            .expect("run the test binary");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(output.status.success() && stdout.contains("1 passed"), "{stdout}");
        return;
    }

    let cli = fake("[]", 0);

    assert_eq!(names(&cli).await.expect("read"), Vec::<String>::new());
    assert_eq!(cli.recorded("project").as_deref(), Some("1 1"));
    assert_eq!(cli.recorded("quiet").as_deref(), Some("1 1 1"));
}

#[tokio::test]
async fn a_failed_request_or_another_answer_fails_without_repeating_it() {
    let failed = fake(r#"{"error": "secret-canary"}"#, 1);
    let error = names(&failed).await.expect_err("failed");
    assert!(matches!(error, BusinessError::Process(_)), "{error:?}");
    assert!(!error.to_string().contains("secret-canary"));

    let resolved_config = fake(r#"{"mcp": {"a": {}}, "token": "secret-canary"}"#, 0);
    let error = names(&resolved_config).await.expect_err("not sources");
    assert!(matches!(error, BusinessError::Protocol(_)), "{error:?}");
    assert!(!error.to_string().contains("secret-canary"));
}
