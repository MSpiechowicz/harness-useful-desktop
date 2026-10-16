//! The team database URL and token: which kind a URL names, what may be saved, and the redacted
//! form shown in the UI. Error messages never quote the URL or the token.

use crate::error::{BusinessError, Result};
use crate::model::MemoryRemoteKind;

/// The longest URL or token accepted (4 KiB).
pub const MAX_SECRET_BYTES: usize = 4 * 1024;

/// What a masked secret looks like.
pub(super) const MASK: &str = "••••";

/// The kind of database `url`'s scheme (any case) names; `None` for any other scheme.
pub fn kind_of(url: &str) -> Option<MemoryRemoteKind> {
    let (scheme, _) = url.trim().split_once("://")?;

    match scheme.to_ascii_lowercase().as_str() {
        "postgres" | "postgresql" => Some(MemoryRemoteKind::Postgres),
        "libsql" | "https" | "http" => Some(MemoryRemoteKind::Libsql),
        "mongodb" | "mongodb+srv" => Some(MemoryRemoteKind::Mongodb),
        _ => None,
    }
}

/// The name of `kind` in messages.
pub(super) fn kind_name(kind: MemoryRemoteKind) -> &'static str {
    match kind {
        MemoryRemoteKind::Postgres => "PostgreSQL",
        MemoryRemoteKind::Libsql => "libSQL",
        MemoryRemoteKind::Mongodb => "MongoDB",
    }
}

/// `raw` trimmed, when it is a URL Harness Memory accepts for `kind`: at most
/// [`MAX_SECRET_BYTES`], no control characters (line breaks included), a scheme of `kind`; for
/// libSQL also no user, password, query or fragment, and `http://` only for this machine.
pub fn validate_url(raw: &str, kind: MemoryRemoteKind) -> Result<String> {
    let url = raw.trim();

    if url.is_empty() {
        return Err(invalid("enter the database URL"));
    }

    if url.len() > MAX_SECRET_BYTES {
        return Err(invalid("the database URL is longer than 4 KiB"));
    }

    if url.chars().any(char::is_control) {
        return Err(invalid(
            "the database URL may not contain line breaks or control characters",
        ));
    }

    if kind_of(url) != Some(kind) {
        return Err(invalid(&format!(
            "a {} URL must start with {}",
            kind_name(kind),
            schemes(kind)
        )));
    }

    if kind == MemoryRemoteKind::Libsql {
        check_libsql(url)?;
    }

    Ok(url.to_owned())
}

/// The schemes a URL of `kind` may start with.
fn schemes(kind: MemoryRemoteKind) -> &'static str {
    match kind {
        MemoryRemoteKind::Postgres => "postgres:// or postgresql://",
        MemoryRemoteKind::Libsql => "libsql://, https:// or http:// (this machine only)",
        MemoryRemoteKind::Mongodb => "mongodb:// or mongodb+srv://",
    }
}

/// A libSQL URL carries no credentials (the token is kept apart), and `http://` is for this
/// machine only.
fn check_libsql(url: &str) -> Result<()> {
    let (scheme, rest) = url.split_once("://").unwrap_or(("", url));
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();

    if authority.contains('@') || rest.contains(['?', '#']) {
        return Err(invalid(
            "a libSQL URL takes no user, password, query or fragment; enter the token in its own field",
        ));
    }

    if scheme.eq_ignore_ascii_case("http") && !is_local_host(authority) {
        return Err(invalid(
            "an http:// URL is only allowed for this machine (localhost); use https:// or libsql://",
        ));
    }

    Ok(())
}

/// True when `authority` (`host[:port]`) is `localhost` or a loopback address.
fn is_local_host(authority: &str) -> bool {
    let host = match authority.strip_prefix('[') {
        Some(bracketed) => bracketed.split(']').next().unwrap_or_default(),
        None => authority.split(':').next().unwrap_or_default(),
    };

    host.eq_ignore_ascii_case("localhost") || host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// The token to save: `raw` trimmed, `None` when that is empty (no token). At most
/// [`MAX_SECRET_BYTES`] and no control characters.
pub fn validate_token(raw: &str) -> Result<Option<String>> {
    let token = raw.trim();

    if token.is_empty() {
        return Ok(None);
    }

    if token.len() > MAX_SECRET_BYTES {
        return Err(invalid("the token is longer than 4 KiB"));
    }

    if token.chars().any(char::is_control) {
        return Err(invalid("the token may not contain line breaks or control characters"));
    }

    Ok(Some(token.to_owned()))
}

/// `url` as `scheme://user@host:port/path`: no password, query or fragment. Only the part after
/// the last `@` counts as the host, so a password with an unescaped `@`, `/` or `?` is never
/// shown. A string without `://` is masked whole.
pub fn redact_url(url: &str) -> String {
    let Some((scheme, rest)) = url.trim().split_once("://") else {
        return MASK.to_owned();
    };

    let (user, after) = match rest.rfind('@') {
        Some(at) => (rest[..at].split(':').next(), &rest[at + 1..]),
        None => (None, rest),
    };

    let end = after.find(['?', '#']).unwrap_or(after.len());
    let location = &after[..end];

    match user.filter(|user| !user.is_empty()) {
        Some(user) => format!("{scheme}://{user}@{location}"),
        None => format!("{scheme}://{location}"),
    }
}

/// The secret parts of `url` and `token`, longest first: the whole URL, its password, its query
/// and fragment, and the token. Masked out of the server's answers by [`scrub`].
pub(super) fn secrets_of(url: Option<&str>, token: Option<&str>) -> Vec<String> {
    let mut secrets: Vec<String> = Vec::new();

    if let Some(url) = url {
        secrets.push(url.to_owned());

        if let Some((_, rest)) = url.split_once("://") {
            if let Some(at) = rest.rfind('@')
                && let Some((_, password)) = rest[..at].split_once(':')
            {
                secrets.push(password.to_owned());
            }

            if let Some(start) = rest.find(['?', '#']) {
                secrets.push(rest[start + 1..].to_owned());
            }
        }
    }

    secrets.extend(token.map(str::to_owned));
    secrets.retain(|secret| !secret.is_empty());
    secrets.sort_by_key(|secret| std::cmp::Reverse(secret.len()));
    secrets.dedup();
    secrets
}

/// `text` with every one of `secrets` replaced by [`MASK`].
pub(super) fn scrub(text: &str, secrets: &[String]) -> String {
    secrets
        .iter()
        .fold(text.to_owned(), |out, secret| out.replace(secret.as_str(), MASK))
}

fn invalid(message: &str) -> BusinessError {
    BusinessError::InvalidInput(message.to_owned())
}
