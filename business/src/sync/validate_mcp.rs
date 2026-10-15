//! MCP server validators, re-exported by [`super::validate`]: names, stdio commands and arguments,
//! URLs, env and header key names, and whole servers.
//!
//! A harness expands `$NAME` / `${NAME}` in a server's fields from its own environment, so a
//! received reference could send this machine's secrets to the bundle's author: URLs may hold none,
//! and arguments only reference the env keys the server declares (it is then written switched off
//! until the user fills them in).

use std::sync::LazyLock;

use regex::Regex;

use super::schema::{
    MAX_ARG_CHARS, MAX_ARGS, MAX_COMMAND_CHARS, MAX_KEY_CHARS, MAX_KEYS, MAX_URL_CHARS, McpServer, Transport,
};
use super::secrets::{args_exportable, exportable};
use super::validate::{Check, compile, has_credentials, is_local_path, name, text};
use crate::ecosystem::catalog::DASHBOARD_MCP_SERVER;
use crate::model::SyncInvalidReason;

/// Stdio MCP commands: a bare program name.
static PROGRAM: LazyLock<Regex> = LazyLock::new(|| compile(r"^[A-Za-z0-9][A-Za-z0-9._+-]*$"));

/// Env and header key names.
static KEY_NAME: LazyLock<Regex> = LazyLock::new(|| compile(r"^[A-Za-z_][A-Za-z0-9_-]{0,127}$"));

/// An env reference: `${NAME}` (group 1), `$NAME` (group 2), or any other `${` form such as a
/// default (`${NAME:-x}`) or an unclosed brace (group 3). A `$` before anything else (`5$`, `$1`)
/// is not one.
static ENV_REFERENCE: LazyLock<Regex> =
    LazyLock::new(|| compile(r"\$(?:\{([A-Za-z_][A-Za-z0-9_]*)\}|([A-Za-z_][A-Za-z0-9_]*)|(\{))"));

/// Programs an MCP command may never be (compared without directory, case and `.exe`).
const SHELLS: &[&str] = &["sh", "bash", "zsh", "dash", "fish", "cmd", "pwsh", "powershell"];

/// An MCP server name (see [`name`]), except the dashboard's own entry ([`DASHBOARD_MCP_SERVER`],
/// in any case): Recommended setup writes that one and reports the dashboard configured when it
/// finds it.
pub fn mcp_name(value: &str) -> Check {
    name(value)?;

    if value.eq_ignore_ascii_case(DASHBOARD_MCP_SERVER) {
        return Err(SyncInvalidReason::BadName);
    }

    Ok(())
}

/// True when `command` is a shell interpreter (`sh`, `bash`, `zsh`, `dash`, `fish`, `cmd`, `pwsh`,
/// `powershell`), with or without a directory, `.exe` or upper case.
pub fn is_shell(command: &str) -> bool {
    let base = command
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(command)
        .to_ascii_lowercase();
    let base = base.strip_suffix(".exe").unwrap_or(&base);

    SHELLS.contains(&base)
}

/// A stdio MCP command: 1–256 characters, a bare program name looked up on `PATH`
/// (`^[A-Za-z0-9][A-Za-z0-9._+-]*$`: no directory, no flag, no scheme, no env reference), never a
/// shell interpreter, nothing secret.
pub fn mcp_command(command: &str) -> Check {
    if command.is_empty() {
        return Err(SyncInvalidReason::MissingField);
    }

    text(command, MAX_COMMAND_CHARS)?;

    if is_shell(command) {
        return Err(SyncInvalidReason::ShellCommand);
    }

    if is_local_path(command) {
        return Err(SyncInvalidReason::LocalPath);
    }

    if !PROGRAM.is_match(command) {
        return Err(SyncInvalidReason::UnsafeValue);
    }

    if !exportable(command) {
        return Err(SyncInvalidReason::ContainsSecret);
    }

    Ok(())
}

/// Stdio MCP arguments: at most 64, each at most 1024 characters, [`display_safe`], and nothing
/// secret (including the value after a secret-named flag such as `--token`). Env references are
/// checked by [`mcp_server`], which knows the declared keys.
///
/// [`display_safe`]: super::validate::display_safe
pub fn mcp_args(args: &[String]) -> Check {
    if args.len() > MAX_ARGS {
        return Err(SyncInvalidReason::TooLong);
    }

    for arg in args {
        text(arg, MAX_ARG_CHARS)?;
    }

    if !args_exportable(args) {
        return Err(SyncInvalidReason::ContainsSecret);
    }

    Ok(())
}

/// An MCP server URL: at most 2048 characters, `http://` or `https://` with a host, no whitespace,
/// no env reference (`unsafeValue`), no userinfo and nothing secret (such as `?api_key=`).
pub fn mcp_url(url: &str) -> Check {
    text(url, MAX_URL_CHARS)?;

    let lower = url.to_ascii_lowercase();
    let Some(rest) = lower.strip_prefix("https://").or_else(|| lower.strip_prefix("http://")) else {
        return Err(SyncInvalidReason::UnsupportedScheme);
    };

    if rest.is_empty() || rest.starts_with(['/', '?', '#']) || url.chars().any(char::is_whitespace) {
        return Err(SyncInvalidReason::Malformed);
    }

    if ENV_REFERENCE.is_match(url) {
        return Err(SyncInvalidReason::UnsafeValue);
    }

    if has_credentials(url) {
        return Err(SyncInvalidReason::Credentials);
    }

    if !exportable(url) {
        return Err(SyncInvalidReason::ContainsSecret);
    }

    Ok(())
}

/// True when every env reference in `value` is a plain `$NAME` or `${NAME}` naming one of
/// `declared`.
fn references_only(value: &str, declared: &[String]) -> bool {
    ENV_REFERENCE.captures_iter(value).all(|reference| {
        let named = reference.get(1).or_else(|| reference.get(2));

        named.is_some_and(|name| declared.iter().any(|key| key == name.as_str()))
    })
}

/// Env or header key names: at most 64, each `^[A-Za-z_][A-Za-z0-9_-]{0,127}$`.
pub fn key_names(keys: &[String]) -> Check {
    if keys.len() > MAX_KEYS {
        return Err(SyncInvalidReason::TooLong);
    }

    for key in keys {
        text(key, MAX_KEY_CHARS)?;

        if !KEY_NAME.is_match(key) {
            return Err(SyncInvalidReason::BadKeyName);
        }
    }

    Ok(())
}

/// A whole shared MCP server. Stdio servers need `command` and may not carry a `url`; their
/// arguments may only reference the declared env keys. Http/sse servers need `url` and may not carry
/// `command` or `args`.
pub fn mcp_server(item: &McpServer) -> Check {
    mcp_name(&item.name)?;

    match item.transport {
        Transport::Stdio => {
            if item.url.is_some() {
                return Err(SyncInvalidReason::Malformed);
            }

            mcp_command(item.command.as_deref().unwrap_or_default())?;
            mcp_args(&item.args)?;

            if !item.args.iter().all(|arg| references_only(arg, &item.env_keys)) {
                return Err(SyncInvalidReason::UnsafeValue);
            }
        }
        Transport::Http | Transport::Sse => {
            if item.command.is_some() || !item.args.is_empty() {
                return Err(SyncInvalidReason::Malformed);
            }

            let Some(url) = item.url.as_deref() else {
                return Err(SyncInvalidReason::MissingField);
            };

            mcp_url(url)?;
        }
    }

    key_names(&item.env_keys)?;
    key_names(&item.header_keys)
}
