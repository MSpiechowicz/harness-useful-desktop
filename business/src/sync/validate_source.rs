//! Source validators, re-exported by [`super::validate`]: OMP marketplace sources and PI package
//! sources, classified the way the harness CLIs (and npm) will read them.
//!
//! - `npm:` specs: a registry name with an optional version, range or tag, or a `github:owner/repo`,
//!   `https://`, `git+https://` or `git+ssh://` location on a host name. Anything npm reads as a
//!   local path (starting with `.`, `/`, `~`, `\` or a drive letter; `file:`, `link:`, `git+file:`)
//!   or a tarball (`.tgz`, `.tar`, `.tar.gz`, `.zip`) is a `localPath`; `http:`, `git+http:`,
//!   `git:` and other transports are unsupported. Both the part after `npm:` and the part after
//!   `name@` are classified.
//! - `git:` sources: `host/owner/repo`, `user@host:owner/repo` or an `https://` URL; `file:` is a
//!   local path, other transports (`ssh://`, `git+ssh://`, `http://`) are unsupported.
//! - Every URL or git host must be a host name (`^[A-Za-z0-9]` labels with inner `-`), so it can
//!   never be read as an option (`https://-oProxyCommand=…`).

use std::sync::LazyLock;

use regex::Regex;

use super::validate::{Check, compile, target};
use crate::harness::TargetKind;
use crate::model::SyncInvalidReason;

/// OMP marketplace sources and `github:` npm specs: a GitHub `owner/repo`.
static GITHUB_REPO: LazyLock<Regex> =
    LazyLock::new(|| compile(r"^[A-Za-z0-9](?:[A-Za-z0-9-]{0,38})/[A-Za-z0-9_.-]{1,100}$"));

/// The registry package name of an npm spec (`name` or `@scope/name`; length is capped by the
/// text limit).
static NPM_PACKAGE: LazyLock<Regex> =
    LazyLock::new(|| compile(r"^(?:@[A-Za-z0-9][A-Za-z0-9._-]*/)?[A-Za-z0-9][A-Za-z0-9._-]*$"));

/// What may follow `name@` in a registry spec: a version, range or dist-tag (`1.2.3`, `1.x`,
/// `latest`, `=1.0.0`).
static NPM_VERSION: LazyLock<Regex> = LazyLock::new(|| compile(r"^=?[0-9A-Za-z][0-9A-Za-z._+-]*$"));

/// A host name: dot-separated labels of letters, digits and inner `-`.
static HOST: LazyLock<Regex> = LazyLock::new(|| {
    compile(r"^[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?(?:\.[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?)*$")
});

/// PI package source prefixes a bundle may carry.
const PACKAGE_PREFIXES: &[&str] = &["npm:", "git:", "https://"];

/// npm spec prefixes that install from this machine.
const NPM_LOCAL_SCHEMES: &[&str] = &["file:", "link:", "git+file:"];

/// npm spec prefixes of a remote location a bundle may carry.
const NPM_REMOTE_SCHEMES: &[&str] = &["https://", "git+https://", "git+ssh://"];

/// Endings npm reads as a tarball.
const ARCHIVES: &[&str] = &[".tgz", ".tar", ".tar.gz", ".zip"];

/// An OMP marketplace source: a GitHub `owner/repo` or an `https://` URL on a host name.
pub fn marketplace_source(source: &str) -> Check {
    target(TargetKind::MarketplaceSource, source)?;

    if GITHUB_REPO.is_match(source) {
        return Ok(());
    }

    if is_https_url(source) {
        return url_host(source);
    }

    Err(SyncInvalidReason::UnsupportedSource)
}

/// A PI package source: `npm:…`, `git:…` or `https://…` (any pin in it is kept). Neither the
/// part after the prefix nor an `@ref`/`#ref` may start with `-` (it would read as an option); the
/// rest is classified as described in the module docs.
pub fn package_source(source: &str) -> Check {
    target(TargetKind::PackageSource, source)?;

    let Some(rest) = PACKAGE_PREFIXES.iter().find_map(|prefix| source.strip_prefix(prefix)) else {
        return Err(SyncInvalidReason::UnsupportedSource);
    };

    if rest.starts_with('-') || source.contains("@-") || source.contains("#-") {
        return Err(SyncInvalidReason::UnsafeValue);
    }

    if let Some(spec) = source.strip_prefix("npm:") {
        return npm_spec(spec);
    }

    if let Some(location) = source.strip_prefix("git:") {
        return git_location(location);
    }

    url_host(source)
}

/// The package name of an npm spec and what follows its `@`, if anything: `@scope/pkg@1.0` gives
/// `("@scope/pkg", Some("1.0"))`.
pub(super) fn split_npm_spec(spec: &str) -> (&str, Option<&str>) {
    let version_at = match spec.strip_prefix('@') {
        Some(scoped) => scoped.find('@').map(|at| at + 1),
        None => spec.find('@'),
    };

    match version_at {
        Some(at) => (&spec[..at], Some(&spec[at + 1..])),
        None => (spec, None),
    }
}

/// An npm spec (after `npm:`): a registry name with an optional version, else a remote location
/// (see [`npm_location`]). A name ending like a tarball (`pkg.tgz`) is a local file to npm.
fn npm_spec(spec: &str) -> Check {
    let (name, wanted) = split_npm_spec(spec);

    if !NPM_PACKAGE.is_match(name) {
        return npm_location(spec);
    }

    if is_archive(name) {
        return Err(SyncInvalidReason::LocalPath);
    }

    match wanted {
        None => Ok(()),
        Some(wanted) if NPM_VERSION.is_match(wanted) && !is_archive(wanted) => Ok(()),
        Some(wanted) => npm_location(wanted),
    }
}

/// An npm spec that is not a registry version: `github:owner/repo`, or an `https://`,
/// `git+https://` or `git+ssh://` URL on a host name that is not a tarball.
fn npm_location(value: &str) -> Check {
    let lower = value.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let drive_letter = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    let local_scheme = NPM_LOCAL_SCHEMES.iter().any(|scheme| lower.starts_with(scheme));

    if lower.starts_with(['.', '/', '~', '\\']) || drive_letter || local_scheme {
        return Err(SyncInvalidReason::LocalPath);
    }

    if is_archive(&lower) && !lower.contains("://") {
        return Err(SyncInvalidReason::LocalPath);
    }

    if lower.starts_with("github:") {
        if GITHUB_REPO.is_match(&value["github:".len()..]) {
            return Ok(());
        }

        return Err(SyncInvalidReason::UnsupportedSource);
    }

    let remote = NPM_REMOTE_SCHEMES.iter().any(|scheme| lower.starts_with(scheme));
    if !remote || is_archive(&lower) {
        return Err(SyncInvalidReason::UnsupportedSource);
    }

    url_host(value)
}

/// A `git:` location: `host/owner/repo`, `user@host:owner/repo` or an `https://` URL, on a host
/// name.
fn git_location(location: &str) -> Check {
    let lower = location.to_ascii_lowercase();

    if lower.starts_with("file:") || lower.starts_with("git+file:") {
        return Err(SyncInvalidReason::LocalPath);
    }

    if lower.starts_with("https://") {
        return url_host(location);
    }

    if lower.contains("://") {
        return Err(SyncInvalidReason::UnsupportedSource);
    }

    // The host ends at the first `/` or `:`, after any `user@`.
    let authority = location.split('/').next().unwrap_or_default();
    let host = authority.rsplit_once('@').map_or(authority, |(_, host)| host);

    host_name(host.split(':').next().unwrap_or_default())
}

/// The host of `scheme://[user@]host[:port]/…` must be a host name, and any port a number.
fn url_host(url: &str) -> Check {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host_and_port = authority.rsplit_once('@').map_or(authority, |(_, host)| host);

    let (host, port) = match host_and_port.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (host_and_port, None),
    };

    if port.is_some_and(|port| port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit())) {
        return Err(SyncInvalidReason::UnsafeValue);
    }

    host_name(host)
}

fn host_name(host: &str) -> Check {
    if HOST.is_match(host) {
        Ok(())
    } else {
        Err(SyncInvalidReason::UnsafeValue)
    }
}

fn is_archive(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();

    ARCHIVES.iter().any(|ending| lower.ends_with(ending))
}

/// True for `https://<host>…` without whitespace.
fn is_https_url(value: &str) -> bool {
    value
        .strip_prefix("https://")
        .is_some_and(|rest| !rest.is_empty() && !rest.starts_with('/') && !value.chars().any(char::is_whitespace))
}
