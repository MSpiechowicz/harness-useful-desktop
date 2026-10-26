//! Whether a registered marketplace source is the Harness Skills' GitHub repository, whichever
//! address form it was added with.

/// True when the marketplace source `registered` names the GitHub repository `repo` (`owner/repo`):
/// as `owner/repo` or as one of its GitHub addresses ([`github_path`]), with or without `.git` and
/// a trailing `/`, in any ASCII case. A marketplace of the same name from elsewhere does not count.
pub(super) fn same_repository(registered: &str, repo: &str) -> bool {
    let normalized = registered.trim().to_ascii_lowercase();
    let path = github_path(&normalized).unwrap_or(&normalized).trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);

    path == repo.to_ascii_lowercase()
}

/// The `owner/repo` part of a lowercase GitHub address: `github.com/owner/repo`, its `https://` or
/// `http://` URL (with or without `www.`) or its SSH remote (`git@github.com:owner/repo`,
/// `ssh://git@github.com/owner/repo`). `None` for any other source.
fn github_path(source: &str) -> Option<&str> {
    if let Some(path) = source.strip_prefix("git@github.com:") {
        return Some(path);
    }

    let address = ["ssh://git@", "https://", "http://"]
        .into_iter()
        .find_map(|scheme| source.strip_prefix(scheme))
        .unwrap_or(source);
    let address = address.strip_prefix("www.").unwrap_or(address);

    address.strip_prefix("github.com/")
}
