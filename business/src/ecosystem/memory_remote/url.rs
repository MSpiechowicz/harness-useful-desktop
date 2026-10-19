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

/// The host suffixes of Amazon DocumentDB clusters (any case, a trailing dot ignored), as the
/// server detects them.
const DOCUMENTDB_HOSTS: [&str; 4] = [
    ".docdb.amazonaws.com",
    ".docdb-elastic.amazonaws.com",
    ".docdb.amazonaws.com.cn",
    ".docdb-elastic.amazonaws.com.cn",
];

/// The `database_kind` value of Amazon DocumentDB (any case).
pub(super) const DOCUMENTDB_KIND: &str = "documentdb";

/// The `database_kind` value of MongoDB (any case).
pub(super) const MONGODB_KIND: &str = "mongodb";

/// The kind of a saved `url` whose `database_kind` key is `database_kind`, as the server picks it:
/// [`kind_of`] the scheme, and for a `mongodb://` or `mongodb+srv://` URL Amazon DocumentDB when
/// the key says `documentdb`, MongoDB when it says `mongodb`, else DocumentDB when a host is a
/// DocumentDB cluster's ([`is_documentdb_url`]). Any other key value counts as unset.
pub fn saved_kind(url: &str, database_kind: Option<&str>) -> Option<MemoryRemoteKind> {
    let kind = kind_of(url)?;

    if kind != MemoryRemoteKind::Mongodb {
        return Some(kind);
    }

    let named = database_kind.map(str::trim);

    if named.is_some_and(|name| name.eq_ignore_ascii_case(DOCUMENTDB_KIND)) {
        return Some(MemoryRemoteKind::Documentdb);
    }

    if named.is_some_and(|name| name.eq_ignore_ascii_case(MONGODB_KIND)) {
        return Some(MemoryRemoteKind::Mongodb);
    }

    if is_documentdb_url(url) {
        Some(MemoryRemoteKind::Documentdb)
    } else {
        Some(MemoryRemoteKind::Mongodb)
    }
}

/// The `database_kind` value that keeps a `url` of `kind` that kind: `documentdb` for Amazon
/// DocumentDB, `mongodb` for MongoDB at a host the server would otherwise take for a DocumentDB
/// cluster's ([`is_documentdb_url`]), else none (the scheme decides).
pub(super) fn kind_key(kind: MemoryRemoteKind, url: &str) -> Option<&'static str> {
    match kind {
        MemoryRemoteKind::Documentdb => Some(DOCUMENTDB_KIND),
        MemoryRemoteKind::Mongodb if is_documentdb_url(url) => Some(MONGODB_KIND),
        _ => None,
    }
}

/// True when a host of `url` (`scheme://[user@]host[:port][,host…]/…`) ends in one of
/// [`DOCUMENTDB_HOSTS`].
pub fn is_documentdb_url(url: &str) -> bool {
    let Some((_, rest)) = url.trim().split_once("://") else {
        return false;
    };

    // The user info ends at the last `@` before the query; the hosts end at the path.
    let head = rest.split(['?', '#']).next().unwrap_or_default();
    let after = head.rfind('@').map_or(head, |at| &head[at + 1..]);
    let hosts = after.split('/').next().unwrap_or_default();

    hosts.split(',').any(|host| {
        let name = host.trim().split(':').next().unwrap_or_default();
        let name = name.trim_end_matches('.').to_ascii_lowercase();

        DOCUMENTDB_HOSTS.iter().any(|suffix| name.ends_with(suffix))
    })
}

/// True when `url`'s scheme is one a URL of `kind` may start with (MongoDB and Amazon DocumentDB
/// share theirs).
pub(super) fn scheme_fits(url: &str, kind: MemoryRemoteKind) -> bool {
    let scheme_kind = match kind {
        MemoryRemoteKind::Documentdb => MemoryRemoteKind::Mongodb,
        other => other,
    };

    kind_of(url) == Some(scheme_kind)
}

/// The name of `kind` in messages.
pub(super) fn kind_name(kind: MemoryRemoteKind) -> &'static str {
    match kind {
        MemoryRemoteKind::Postgres => "PostgreSQL",
        MemoryRemoteKind::Libsql => "libSQL",
        MemoryRemoteKind::Mongodb => "MongoDB",
        MemoryRemoteKind::Documentdb => "Amazon DocumentDB",
    }
}

/// `kind`'s name with its article (`a PostgreSQL`, `an Amazon DocumentDB`).
pub(super) fn a_kind(kind: MemoryRemoteKind) -> String {
    match kind {
        MemoryRemoteKind::Documentdb => format!("an {}", kind_name(kind)),
        _ => format!("a {}", kind_name(kind)),
    }
}

/// `raw` trimmed, when it is a URL Harness Memory accepts for `kind`: at most
/// [`MAX_SECRET_BYTES`], no control characters (line breaks included), a scheme of `kind` (Amazon
/// DocumentDB takes MongoDB's); for
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

    if !scheme_fits(url, kind) {
        return Err(invalid(&format!(
            "{} URL must start with {}",
            a_kind(kind),
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
        MemoryRemoteKind::Mongodb | MemoryRemoteKind::Documentdb => "mongodb:// or mongodb+srv://",
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
