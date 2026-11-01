//! The team database URL and token of a Harness family server: which kind a URL names, what may be
//! saved, and the redacted form shown in the UI. Error messages never quote the URL or the token.
//! [`memory_remote`](super::memory_remote) adds what only the memory server knows (Amazon
//! DocumentDB hosts).

use crate::error::{BusinessError, Result};
use crate::model::MemoryRemoteKind;

/// The longest URL or token accepted (4 KiB).
pub const MAX_SECRET_BYTES: usize = 4 * 1024;

/// What a masked secret looks like.
pub(crate) const MASK: &str = "••••";

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

/// True when `url`'s scheme is one a URL of `kind` may start with (MongoDB and Amazon DocumentDB
/// share theirs).
pub(crate) fn scheme_fits(url: &str, kind: MemoryRemoteKind) -> bool {
    let scheme_kind = match kind {
        MemoryRemoteKind::Documentdb => MemoryRemoteKind::Mongodb,
        other => other,
    };

    kind_of(url) == Some(scheme_kind)
}

/// The name of `kind` in messages.
pub(crate) fn kind_name(kind: MemoryRemoteKind) -> &'static str {
    match kind {
        MemoryRemoteKind::Postgres => "PostgreSQL",
        MemoryRemoteKind::Libsql => "libSQL",
        MemoryRemoteKind::Mongodb => "MongoDB",
        MemoryRemoteKind::Documentdb => "Amazon DocumentDB",
    }
}

/// `kind`'s name with its article (`a PostgreSQL`, `an Amazon DocumentDB`).
pub(crate) fn a_kind(kind: MemoryRemoteKind) -> String {
    match kind {
        MemoryRemoteKind::Documentdb => format!("an {}", kind_name(kind)),
        _ => format!("a {}", kind_name(kind)),
    }
}

/// `raw` trimmed, when it is a URL Harness Memory accepts for `kind`: at most
/// [`MAX_SECRET_BYTES`], no control characters (line breaks included), a scheme of `kind` (Amazon
/// DocumentDB takes MongoDB's); for
/// libSQL also no user, password, query or fragment, and `http://` only for this machine. For
/// MongoDB and Amazon DocumentDB the user and password are percent-encoded when they hold reserved
/// characters ([`encode_credentials`]), so a pasted raw password works.
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

    if matches!(kind, MemoryRemoteKind::Mongodb | MemoryRemoteKind::Documentdb) {
        return Ok(encode_credentials(url));
    }

    Ok(url.to_owned())
}

/// `url` with the user and password of `scheme://user:password@host…` percent-encoded. As in
/// [`redact_url`], the last `@` ends the credentials, so a password may hold a raw `@`, `/`, `?`
/// or `#`. A `%` followed by two hex digits is kept, so an already encoded URL is unchanged
/// (a password that literally contains such a sequence must be written `%25xx`). A URL without
/// credentials is returned as is.
fn encode_credentials(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_owned();
    };

    let Some(at) = rest.rfind('@') else {
        return url.to_owned();
    };

    let (credentials, host) = (&rest[..at], &rest[at..]);
    let encoded = match credentials.split_once(':') {
        Some((user, password)) => format!("{}:{}", percent_encode(user), percent_encode(password)),
        None => percent_encode(credentials),
    };

    format!("{scheme}://{encoded}{host}")
}

/// `text` with every byte outside `A-Z a-z 0-9 - . _ ~` written as `%XX`; an existing `%XX` stays.
fn percent_encode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());

    for (index, &byte) in bytes.iter().enumerate() {
        let escaped = byte == b'%'
            && bytes.get(index + 1).is_some_and(u8::is_ascii_hexdigit)
            && bytes.get(index + 2).is_some_and(u8::is_ascii_hexdigit);

        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') || escaped {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }

    out
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
pub(crate) fn secrets_of(url: Option<&str>, token: Option<&str>) -> Vec<String> {
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
pub(crate) fn scrub(text: &str, secrets: &[String]) -> String {
    secrets
        .iter()
        .fold(text.to_owned(), |out, secret| out.replace(secret.as_str(), MASK))
}

/// The URL and the token a save writes. `wanted_url` is validated for `kind`, else `saved_url` is
/// kept ([`kept_url`]). A libSQL token is the `wanted_token` validated (empty removes it), else the
/// `saved_token`; PostgreSQL and MongoDB take their credentials in the URL, so they save none.
pub(crate) fn url_and_token(
    kind: MemoryRemoteKind,
    wanted_url: Option<&str>,
    saved_url: Option<&str>,
    wanted_token: Option<&str>,
    saved_token: Option<String>,
) -> Result<(String, Option<String>)> {
    let url = match wanted_url {
        Some(raw) => validate_url(raw, kind)?,
        None => kept_url(saved_url, kind)?,
    };

    let token = match (kind, wanted_token) {
        (MemoryRemoteKind::Libsql, Some(raw)) => validate_token(raw)?,
        (MemoryRemoteKind::Libsql, None) => saved_token,
        _ => None,
    };

    Ok((url, token))
}

/// The saved URL, re-validated, when it is one of `kind`.
fn kept_url(saved: Option<&str>, kind: MemoryRemoteKind) -> Result<String> {
    let Some(saved) = saved.filter(|url| !url.trim().is_empty()) else {
        return Err(invalid("enter the database URL"));
    };

    if !scheme_fits(saved, kind) {
        return Err(BusinessError::InvalidInput(format!(
            "the saved URL is not {} URL, so enter the new one",
            a_kind(kind)
        )));
    }

    validate_url(saved, kind).map_err(|error| match error {
        BusinessError::InvalidInput(reason) => {
            BusinessError::InvalidInput(format!("the saved URL cannot be kept ({reason}), so enter it again"))
        }
        other => other,
    })
}

/// Every secret a check may quote, longest first: those of the typed URL and token and of the saved
/// ones (see [`secrets_of`]).
pub(crate) fn check_secrets(
    typed_url: Option<&str>,
    typed_token: Option<&Option<String>>,
    saved_url: Option<&str>,
    saved_token: Option<&str>,
) -> Vec<String> {
    let mut secrets = secrets_of(typed_url, typed_token.and_then(|token| token.as_deref()));
    secrets.extend(secrets_of(saved_url, saved_token));
    secrets.sort_by_key(|secret| std::cmp::Reverse(secret.len()));

    secrets
}

/// Why the saved URL cannot be tested as `kind`, if it cannot: none is saved, or `fits` says it is
/// not one of `kind`.
pub(crate) fn saved_url_problem(
    saved: Option<&str>,
    fits: impl FnOnce(&str) -> bool,
    kind: MemoryRemoteKind,
) -> Option<String> {
    match saved.filter(|url| !url.trim().is_empty()) {
        None => Some("no database URL is saved, so enter one to test it".to_owned()),
        Some(url) if !fits(url) => Some(format!(
            "the saved URL is not {} URL, so enter one to test it",
            a_kind(kind)
        )),
        Some(_) => None,
    }
}

/// The sentence of a validation error.
pub(crate) fn reason(error: BusinessError) -> String {
    match error {
        BusinessError::InvalidInput(reason) => reason,
        other => other.to_string(),
    }
}

fn invalid(message: &str) -> BusinessError {
    BusinessError::InvalidInput(message.to_owned())
}
