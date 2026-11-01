//! What only the memory server knows about the team database URL: Amazon DocumentDB (its hosts,
//! the `database_kind` key that tells it from MongoDB). The URL and token checks are
//! [`sidecar_remote_url`](crate::ecosystem::sidecar_remote_url)'s.

use crate::model::MemoryRemoteKind;

pub use crate::ecosystem::sidecar_remote_url::{MAX_SECRET_BYTES, kind_of, redact_url, validate_token, validate_url};

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
