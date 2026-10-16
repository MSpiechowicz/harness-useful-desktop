//! PI package sources: the kind, display name and install directory of a `packages` entry.
//!
//! Follows PI's package manager (v1.1.0 `core/package-manager.js`, `utils/git.js`): git sources
//! are cloned to `<base>/git/<host>/<path>` and npm sources installed to
//! `<base>/npm/node_modules/<name>`, where `base` is the agent directory (user settings) or the
//! project `.pi` directory (project settings). Pins (`@ref`, `#ref`) never change the directory.
//! PI's fallback to the global npm root for user packages needs `npm root -g` and is not followed.
//!
//! Sources may carry credentials (URL userinfo, `?private_token=` queries), so neither the display
//! name nor a computed directory ever includes userinfo or a query.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::layers::{Layer, resolve};
use crate::env::Env;
use crate::model::ConfigValue;
use crate::security::{Redactor, canonical_within};

/// Hosts PI (through `hosted-git-info`) expands from `<shortcut>:<user>/<repo>`.
const HOSTED_SHORTCUTS: [(&str, &str); 3] = [
    ("github:", "github.com"),
    ("gitlab:", "gitlab.com"),
    ("bitbucket:", "bitbucket.org"),
];

/// `npm`, `git`, `local` or `unknown`, from the source prefix.
pub(super) fn package_kind(raw: &str) -> &'static str {
    let is_git = ["git:", "https://", "http://", "ssh://", "git@"]
        .iter()
        .any(|prefix| raw.starts_with(prefix));
    let is_local = ["./", "../", "/", "~", ".\\", "..\\"]
        .iter()
        .any(|prefix| raw.starts_with(prefix))
        || raw.as_bytes().get(1) == Some(&b':');

    if raw.starts_with("npm:") {
        "npm"
    } else if is_git {
        "git"
    } else if is_local {
        "local"
    } else {
        "unknown"
    }
}

/// The display name of a package: the npm package name, the git repository name or the last
/// path segment. Never includes credentials, query strings or pins: a name with characters other
/// than `A-Z a-z 0-9 . _ @ / -`, or one that looks like a secret, is replaced by the kind
/// (`git`, `npm`, `local`, else `package`).
pub(super) fn display_name(raw: &str, kind: &str) -> String {
    let parsed = match kind {
        "npm" => npm_name(raw).map(str::to_owned),
        "git" => git_source(raw).and_then(|git| git.path.rsplit('/').next().map(str::to_owned)),
        _ => None,
    };
    let name = parsed.unwrap_or_else(|| last_segment(raw));

    if is_safe_name(&name) {
        return name;
    }

    match kind {
        "git" | "npm" | "local" => kind.to_owned(),
        _ => "package".to_owned(),
    }
}

/// True for a non-empty name of plain characters that the redaction rules would show as is
/// (no credential format, nothing random-looking).
fn is_safe_name(name: &str) -> bool {
    let plain = |character: char| character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '@' | '/' | '-');
    if name.is_empty() || !name.chars().all(plain) {
        return false;
    }

    let shown = Redactor.redact(&Value::String(name.to_owned()));
    matches!(shown, ConfigValue::Text(text) if text == name)
}

/// The directory PI installed the package into (see the module docs), or the resolved local path.
/// Only an existing directory is returned, canonical; git and npm directories must resolve inside
/// their install root.
pub(super) fn install_dir(env: &Env, layer: &Layer, raw: &str, kind: &str) -> Option<PathBuf> {
    let (root, relative) = match kind {
        "git" => {
            let git = git_source(raw)?;
            (layer.base.join("git"), Path::new(&git.host).join(&git.path))
        }
        "npm" => (
            layer.base.join("npm").join("node_modules"),
            PathBuf::from(npm_name(raw)?),
        ),
        "local" => {
            let dir = dunce::canonicalize(resolve(env, layer, raw)).ok()?;
            return dir.is_dir().then_some(dir);
        }
        _ => return None,
    };

    let dir = canonical_within(&root, &relative).ok()?;
    dir.is_dir().then_some(dir)
}

/// A git source reduced to what selects its install directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GitSource {
    /// Lowercase host name (`a-z 0-9 . _ -` only), without credentials or port.
    pub host: String,
    /// Repository path below the host (`<user>/<repo>`), without `.git`, pin or query.
    pub path: String,
}

/// Parses a git source the way PI does: with a `git:` prefix any form (URL, `git@host:path`,
/// `host/path`, `<user>/<repo>` on GitHub, `github:<user>/<repo>`); without it only
/// `http(s)://`, `ssh://` and `git://` URLs. `None` for anything PI would refuse to install,
/// including paths with `..`, backslashes, percent escapes or fewer than two segments.
///
/// A URL's query and fragment are dropped, as PI's URL parser does. In the other forms PI keeps a
/// `?query` in the clone path; such a source is refused rather than followed, so a credential in
/// the query never becomes part of a path the view shows. Userinfo before a bare host
/// (`git:user:pass@host/path`) is dropped too.
pub(super) fn git_source(raw: &str) -> Option<GitSource> {
    let trimmed = raw.trim();
    let prefixed = trimmed.strip_prefix("git:").map(str::trim);
    let url = prefixed.unwrap_or(trimmed);

    let url = if protocol_rest(url).is_some() {
        url.split(['?', '#']).next().unwrap_or(url)
    } else if prefixed.is_some() {
        url.split('#').next().unwrap_or(url)
    } else {
        return None;
    };

    if url.contains('?') {
        return None;
    }

    let (host, path) = split_host(url)?;

    let path = path.split('@').next().unwrap_or(path);
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);

    let host = host.to_ascii_lowercase();
    let host_char = |character: char| character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-');
    let safe_host = !host.is_empty() && host.chars().all(host_char) && host != "." && host != "..";

    let segments: Vec<&str> = path.split('/').collect();
    let unsafe_part = |part: &str| part.is_empty() || part.contains(['\\', '\0', '%']) || part == "." || part == "..";

    if !safe_host || segments.len() < 2 || segments.iter().any(|part| unsafe_part(part)) {
        return None;
    }

    Some(GitSource {
        host,
        path: path.to_owned(),
    })
}

/// `(host, path)` of a git URL in any of the forms [`git_source`] accepts.
fn split_host(url: &str) -> Option<(&str, &str)> {
    if let Some(rest) = url.strip_prefix("git@") {
        return rest.split_once(':');
    }

    if let Some(rest) = protocol_rest(url) {
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        let host = authority.rsplit('@').next().unwrap_or(authority);
        let host = host.split(':').next().unwrap_or(host);

        return Some((host, path));
    }

    if let Some((_, host)) = HOSTED_SHORTCUTS.iter().find(|(shortcut, _)| url.starts_with(shortcut)) {
        return Some((host, url.split_once(':')?.1));
    }

    let (host, path) = url.split_once('/')?;
    let host = host.rsplit('@').next().unwrap_or(host);
    let is_host = host.contains('.') || host == "localhost";

    if is_host {
        Some((host, path))
    } else if !path.contains('/') {
        Some(("github.com", url))
    } else {
        None
    }
}

/// The part after `<scheme>://` for the protocols PI treats as git without a `git:` prefix.
fn protocol_rest(url: &str) -> Option<&str> {
    let (scheme, rest) = url.split_once("://")?;
    let is_git_protocol = ["http", "https", "ssh", "git"]
        .iter()
        .any(|protocol| scheme.eq_ignore_ascii_case(protocol));

    is_git_protocol.then_some(rest)
}

/// The package name of an `npm:` source (`@scope/name` or `name`, without the version), when it
/// is safe as a `node_modules` path.
pub(super) fn npm_name(raw: &str) -> Option<&str> {
    let spec = raw.strip_prefix("npm:")?.trim();
    let version_at = match spec.strip_prefix('@') {
        Some(rest) => rest.find('@').map(|index| index + 1),
        None => spec.find('@'),
    };
    let name = &spec[..version_at.unwrap_or(spec.len())];

    let segments: Vec<&str> = name.split('/').collect();
    let scoped = segments.len() == 2 && segments[0].starts_with('@');
    let safe = segments
        .iter()
        .all(|part| !part.is_empty() && *part != "." && *part != ".." && !part.contains(['\\', '\0', ':']));

    (safe && (segments.len() == 1 || scoped)).then_some(name)
}

/// The last segment of `raw`, cut at any query, fragment, credential or pin separator.
fn last_segment(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches(['/', '\\']);
    let trimmed = trimmed.split(['?', '#']).next().unwrap_or(trimmed);
    let segment = trimmed.rsplit(['/', '\\', ':', '@']).next().unwrap_or(trimmed);

    segment.strip_suffix(".git").unwrap_or(segment).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(raw: &str) -> Option<(String, String)> {
        git_source(raw).map(|git| (git.host, git.path))
    }

    fn pair(host: &str, path: &str) -> Option<(String, String)> {
        Some((host.to_owned(), path.to_owned()))
    }

    #[test]
    fn git_sources_resolve_to_host_and_repository_path() {
        let repo = pair("github.com", "acme/tools");

        assert_eq!(git("git:github.com/acme/tools"), repo);
        assert_eq!(git("git:github.com/acme/tools@v1"), repo);
        assert_eq!(git("git:acme/tools"), repo);
        assert_eq!(git("git:github:acme/tools#main"), repo);
        assert_eq!(git("git:git@github.com:acme/tools.git@v2"), repo);
        assert_eq!(git("https://github.com/acme/tools.git"), repo);
        assert_eq!(git("ssh://git@GitHub.com:22/acme/tools"), repo);
        assert_eq!(
            git("https://ci:secret@git.example.com/group/sub/tools.git"),
            pair("git.example.com", "group/sub/tools")
        );
    }

    #[test]
    fn unsafe_or_non_git_sources_are_refused() {
        assert_eq!(git("github.com/acme/tools"), None);
        assert_eq!(git("git@github.com:acme/tools"), None);
        assert_eq!(git("git:github.com/acme/../../escape"), None);
        assert_eq!(git("git:github.com/acme/%2e%2e"), None);
        assert_eq!(git("git:github.com/acme"), None);
        assert_eq!(git("https://host"), None);
    }

    #[test]
    fn npm_names_drop_the_version_and_refuse_paths() {
        assert_eq!(npm_name("npm:@demo/pi-review-kit@1.0.0"), Some("@demo/pi-review-kit"));
        assert_eq!(npm_name("npm:pi-tools@^2"), Some("pi-tools"));
        assert_eq!(npm_name("npm:pi-tools"), Some("pi-tools"));
        assert_eq!(npm_name("npm:../escape"), None);
        assert_eq!(npm_name("npm:a/b"), None);
    }

    #[test]
    fn display_names_never_carry_credentials_or_pins() {
        assert_eq!(display_name("git:github.com/acme/tools@v1", "git"), "tools");
        assert_eq!(
            display_name("https://ci:secret@git.example.com/acme/private.git", "git"),
            "private"
        );
        assert_eq!(display_name("https://ci:secret@host", "git"), "host");
        assert_eq!(display_name("npm:@demo/kit@1.0.0", "npm"), "@demo/kit");
        assert_eq!(display_name("../tools/pi-local/", "local"), "pi-local");
    }

    #[test]
    fn queries_and_userinfo_never_reach_the_name_or_the_clone_path() {
        // Built at run time: a token-shaped literal in the source trips secret scanning on push.
        let gitlab = format!(
            "https://gitlab.example.com/acme/tools.git?private_token={}-AbCdEf0123456789xyzQ",
            "glpat"
        );
        let sources = [
            gitlab.as_str(),
            "git:host.example/acme/tools?access_token=s3cr3tV4lu3xQ9z",
            "https://host.example/acme/tools/?token=abc",
            "git:ci:hunter2@host.example/acme/tools",
            "git:git@host.example:acme/tools?token=abc",
        ];

        for raw in sources {
            let name = display_name(raw, package_kind(raw));
            assert_eq!(name, "tools", "{raw}");

            for leak in ["?", "token", "glpat-", "hunter2"] {
                assert!(!name.contains(leak), "{raw} -> {name}");
            }
        }

        // URL queries are not part of the path; elsewhere PI would keep them, so the source is
        // refused rather than followed.
        assert_eq!(
            git("https://gitlab.example.com/acme/tools.git?private_token=glpat-x"),
            pair("gitlab.example.com", "acme/tools")
        );
        assert_eq!(
            git("https://host.example/acme/tools/?token=abc"),
            pair("host.example", "acme/tools")
        );
        assert_eq!(git("git:host.example/acme/tools?access_token=abc"), None);
        assert_eq!(git("git:git@host.example:acme/tools?token=abc"), None);
        assert_eq!(
            git("git:ci:hunter2@host.example/acme/tools"),
            pair("host.example", "acme/tools")
        );
        assert_eq!(git("git:git@ci:pw@host.example:acme/tools"), None);
    }

    #[test]
    fn unsafe_or_secret_looking_names_fall_back_to_the_kind() {
        assert_eq!(
            display_name(
                "https://host.example/acme/ghp_AbCdEfGhIjKlMnOpQrStUvWxYz0123456789",
                "git"
            ),
            "git"
        );
        assert_eq!(
            display_name("../tools/Zx8Qp2Lm9Rt4Vb7Nc1Kd6Hf3Jg5Ws0Ye", "local"),
            "local"
        );
        assert_eq!(display_name("../tools/name with spaces", "local"), "local");
        assert_eq!(display_name("weird?thing", "unknown"), "weird");
        assert_eq!(display_name("?", "unknown"), "package");
        assert_eq!(display_name("npm:@demo/kit", "npm"), "@demo/kit");
    }
}
