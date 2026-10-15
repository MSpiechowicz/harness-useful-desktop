//! MCP validators of "Share setup": env references the importer's harness would expand, secret
//! flags and `user:password` values in arguments, and the name the dashboard's own entry uses.

#[path = "sync_support/servers.rs"]
mod servers;

use harness_business::model::SyncInvalidReason;
use harness_business::sync::schema::McpServer;
use harness_business::sync::secrets::args_exportable;
use harness_business::sync::validate;
use pretty_assertions::assert_eq;
use servers::{http, stdio};

fn declaring(mut server: McpServer, env_keys: &[&str]) -> McpServer {
    server.env_keys = env_keys.iter().map(|key| (*key).to_owned()).collect();
    server
}

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn urls_never_carry_an_env_reference_even_a_declared_one() {
    for url in [
        "https://c.example.com/mcp?k=${GITHUB_TOKEN}",
        "https://${ANTHROPIC_API_KEY}.c.example.com/mcp",
        "https://c.example.com/${GITHUB_TOKEN}/mcp",
        "https://c.example.com/mcp?k=$GITHUB_TOKEN",
        "https://c.example.com/mcp#${X",
    ] {
        assert_eq!(validate::mcp_url(url), Err(SyncInvalidReason::UnsafeValue), "{url}");

        let server = declaring(http("remote", url), &["GITHUB_TOKEN", "ANTHROPIC_API_KEY"]);
        assert_eq!(
            validate::mcp_server(&server),
            Err(SyncInvalidReason::UnsafeValue),
            "{url}"
        );
    }

    assert_eq!(validate::mcp_url("https://c.example.com/mcp?price=5$"), Ok(()));
}

#[test]
fn arguments_may_only_reference_declared_env_keys() {
    let server = |values: &[&str]| stdio("tool", "my-server", values);

    for values in [
        &["$HOME"][..],
        &["${HOME}"],
        &["--root=${WORKSPACE}"],
        &["--api-key", "${API_KEY}", "$OTHER"],
        &["${HOME:-x}"],
        &["${HOME"],
        &["${}"],
    ] {
        assert_eq!(
            validate::mcp_server(&server(values)),
            Err(SyncInvalidReason::UnsafeValue),
            "{values:?}"
        );
    }
    for value in ["${HOME:-x}", "${HOME"] {
        assert_eq!(
            validate::mcp_server(&declaring(server(&[value]), &["HOME"])),
            Err(SyncInvalidReason::UnsafeValue),
            "{value}"
        );
    }

    for values in [
        &["$HOME"][..],
        &["${HOME}"],
        &["--root=${WORKSPACE}"],
        &["--api-key", "${API_KEY}"],
        &["--home=$HOME/x"],
    ] {
        let declared = declaring(server(values), &["HOME", "WORKSPACE", "API_KEY"]);
        assert_eq!(validate::mcp_server(&declared), Ok(()), "{values:?}");
    }

    assert_eq!(validate::mcp_server(&server(&["--price=5$", "$1"])), Ok(()));
}

#[test]
fn attached_short_passwords_and_user_password_values_are_secret() {
    for secret in [
        &["-phunter2"][..],
        &["-Phunter2"],
        &["-p", "hunter2"],
        &["-p=hunter2"],
        &["-u", "alice:hunter2"],
        &["--user", "alice:hunter2"],
        &["--user=alice:hunter2"],
        &["--auth", "alice:hunter2"],
        &["--basic-auth", "alice:hunter2"],
        &["--proxy-user", "alice:hunter2"],
        &["--password", "1234"],
        &["--pass", "1234"],
        &["--password=1234"],
    ] {
        assert!(!args_exportable(&args(secret)), "{secret:?}");
    }

    for fine in [
        &["-p", "8080"][..],
        &["-p8080"],
        &["-p=8080"],
        &["-P", "2222"],
        &["-p", "$PASSWORD"],
        &["-u", "alice"],
        &["--user", "alice"],
        &["-u", "alice:$PASSWORD"],
        &["--port", "3000"],
        &["-y", "pkg"],
    ] {
        assert!(args_exportable(&args(fine)), "{fine:?}");
    }
}

#[test]
fn the_dashboard_entry_name_is_reserved() {
    assert_eq!(validate::mcp_name("harness-dashboard"), Err(SyncInvalidReason::BadName));
    assert_eq!(validate::mcp_name("Harness-Dashboard"), Err(SyncInvalidReason::BadName));
    assert_eq!(
        validate::mcp_server(&http("harness-dashboard", "https://c.example.com/mcp")),
        Err(SyncInvalidReason::BadName)
    );
    assert_eq!(validate::mcp_name("harness-dashboard-2"), Ok(()));
}
