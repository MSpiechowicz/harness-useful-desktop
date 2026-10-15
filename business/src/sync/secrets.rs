//! Secret detection for shared values, built on the app's redactor ([`crate::security`]).
//!
//! A value is shareable only when redaction would leave it exactly as it is: no credential formats
//! (`sk-…`, `ghp_…`, JWTs, `Bearer …`), no secret-named assignments (`api_key=…`), no random-looking
//! tokens, no URL userinfo or secret query values (`?api_key=…`), and no `!command`. Export leaves
//! out MCP servers and sources that fail this; import marks them `invalid`.
//!
//! `$NAME` / `${NAME}` references are not secret: they name a variable, not its value. MCP fields
//! limit them further (see [`super::validate_mcp`]): the harness expands them on the importing
//! machine.

use serde_json::Value;

use super::validate::has_credentials;
use crate::harnesses::omp::{GITHUB_REPO, is_word_like};
use crate::model::{ConfigValue, SecretKind};
use crate::security::{RedactionContext, Redactor, redact_text};

/// True when `value` holds nothing the redactor would hide and carries no URL credentials.
pub fn exportable(value: &str) -> bool {
    if has_credentials(value) {
        return false;
    }

    // A repository on GitHub with word-like names (`MSpiechowicz/harness-useful-skills`) is a public name,
    // although the redactor's entropy check may take its mixed-case owner for a token.
    if is_word_like_repo(value) {
        return true;
    }

    if redact_text(value) != value {
        return false;
    }

    match Redactor.redact(&Value::String(value.to_owned())) {
        ConfigValue::Text(shown) => shown == value,
        ConfigValue::Secret(SecretKind::EnvRef { .. }) => true,
        _ => false,
    }
}

/// `owner/repo`, `git:github.com/owner/repo` or `https://github.com/owner/repo(.git)`, both names word-like.
fn is_word_like_repo(value: &str) -> bool {
    let path = ["git:github.com/", "https://github.com/", ""]
        .iter()
        .find_map(|prefix| value.strip_prefix(prefix))
        .unwrap_or(value);
    let path = path.strip_suffix(".git").unwrap_or(path);

    GITHUB_REPO
        .captures(path)
        .is_some_and(|parts| is_word_like(&parts[1]) && is_word_like(&parts[2]))
}

/// True when every argument is [`exportable`] and none passes a secret (see [`passes_secret`]):
/// neither the argument after a flag (a value may itself start with `-`), nor a flag's `=value`,
/// nor a value attached to `-p`/`-P` (`-phunter2`; a port number such as `-p8080` is fine).
///
/// `--token=value` is also caught by [`exportable`] itself (a secret assignment).
pub fn args_exportable(args: &[String]) -> bool {
    if !args.iter().all(|arg| exportable(arg)) {
        return false;
    }

    if args.iter().any(|arg| attached_secret(arg)) {
        return false;
    }

    args.windows(2).all(|pair| {
        let [flag, value] = pair else {
            return true;
        };

        !passes_secret(flag, value)
    })
}

/// True for `-p<value>`/`-P<value>` with a value that is not a number, or `--flag=value` where
/// the flag passes the value as a secret.
fn attached_secret(arg: &str) -> bool {
    let short = arg.strip_prefix("-p").or_else(|| arg.strip_prefix("-P"));

    match short {
        Some(value) if !value.is_empty() && !value.starts_with('=') => !is_number(value),
        _ => arg
            .split_once('=')
            .is_some_and(|(flag, value)| passes_secret(flag, value)),
    }
}

/// True when `flag` passes `value` as a secret: a secret flag ([`is_secret_flag`]) followed by
/// anything but a `$NAME` reference (`-p`/`-P` may also take a port number, `--password`/`--pass`
/// may not), or a user flag ([`USER_FLAGS`]) followed by `user:password`.
fn passes_secret(flag: &str, value: &str) -> bool {
    if is_secret_flag(flag) {
        let port = matches!(flag, "-p" | "-P") && is_number(value);

        return !port && !is_env_ref(value);
    }

    USER_FLAGS.contains(&flag) && is_user_password(value)
}

/// Flags that take a `user[:password]` value (curl, wget, proxies).
const USER_FLAGS: &[&str] = &["-u", "--user", "--auth", "--basic-auth", "--proxy-user"];

/// True for `user:password` with a password that is not a `$NAME` reference.
fn is_user_password(value: &str) -> bool {
    value
        .split_once(':')
        .is_some_and(|(user, password)| !user.is_empty() && !password.is_empty() && !is_env_ref(password))
}

fn is_number(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

/// Short or abbreviated flag names that pass a secret (`-p`, `--pat`, …), which the redactor's key
/// names do not cover.
const SECRET_FLAG_NAMES: &[&str] = &["p", "P", "key", "pat", "pass", "pwd"];

/// True for a flag (`-x`, `--name`) whose name is in [`SECRET_FLAG_NAMES`] or that the redactor
/// treats as a secret key.
fn is_secret_flag(arg: &str) -> bool {
    let name = arg.trim_start_matches('-');
    if name.len() == arg.len() || name.is_empty() {
        return false;
    }

    if SECRET_FLAG_NAMES.contains(&name) {
        return true;
    }

    let probe = Value::String("value".to_owned());
    let mut context = RedactionContext::at([name]);

    matches!(Redactor.redact_at(&probe, &mut context), ConfigValue::Secret(_))
}

/// True for `$NAME` or `${NAME}`.
fn is_env_ref(value: &str) -> bool {
    matches!(
        Redactor.redact(&Value::String(value.to_owned())),
        ConfigValue::Secret(SecretKind::EnvRef { .. })
    )
}
