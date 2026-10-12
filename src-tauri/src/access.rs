//! Access rules for the commands that hand something to the OS: links opened in the browser and
//! paths revealed in the file manager. Both refuse with a generic `Forbidden`.

use std::path::{Component, Path, PathBuf};

use harness_business::BusinessError;
use harness_business::harness::HarnessRegistry;
use harness_business::security::canonical_within;
use tauri::Url;

use crate::error::{CommandError, CommandResult};

/// Link prefixes allowed besides each harness's homepage and install docs.
const STATIC_LINKS: &[&str] = &[
    "https://github.com/MSpiechowicz/",
    "https://github.com/earendil-works/pi",
];

/// Every allowed link prefix: [`STATIC_LINKS`] plus the homepage and install documentation of each
/// registered harness.
pub fn link_allowlist(registry: &HarnessRegistry) -> Vec<String> {
    let mut prefixes: Vec<String> = STATIC_LINKS.iter().map(|&prefix| prefix.to_owned()).collect();

    for harness in registry.all() {
        prefixes.push(harness.info().homepage);
        prefixes.push(harness.install_hint().docs_url);
    }

    prefixes
}

/// Parses `raw` and returns it (normalized) when it starts with an allowed prefix; else `Forbidden`.
///
/// Only `https` without credentials is accepted. Scheme, host and port must equal the prefix's,
/// and the path must equal the prefix path or continue it at a `/` boundary, so
/// `https://github.com/earendil-works/pi` allows `.../pi`, `.../pi/issues` and `.../pi#readme`
/// but not `.../pi-evil`. Comparing parsed URLs (with `.`/`..` segments resolved) defeats tricks
/// such as `https://github.com.evil.com/` or `https://github.com@evil.com/`.
pub fn allowed_link(allowlist: &[String], raw: &str) -> CommandResult<Url> {
    let url = secure_link(raw)?;

    let allowed = allowlist
        .iter()
        .filter_map(|prefix| Url::parse(prefix).ok())
        .any(|prefix| starts_with_prefix(&url, &prefix));

    if allowed { Ok(url) } else { Err(refused()) }
}

/// Parses `raw` and returns it (normalized) when it is an `https` link without credentials, to any
/// site; else `Forbidden`. For when the user has chosen to open every web link.
pub fn secure_link(raw: &str) -> CommandResult<Url> {
    let url = Url::parse(raw).map_err(|_| refused())?;

    let has_credentials = !url.username().is_empty() || url.password().is_some();
    if url.scheme() != "https" || has_credentials {
        return Err(refused());
    }

    Ok(url)
}

fn starts_with_prefix(url: &Url, prefix: &Url) -> bool {
    let same_origin = url.scheme() == prefix.scheme()
        && url.host_str().is_some()
        && url.host_str() == prefix.host_str()
        && url.port() == prefix.port();

    if !same_origin {
        return false;
    }

    let path = url.path();
    let base = prefix.path();

    if path == base || (base.ends_with('/') && path.starts_with(base)) {
        return true;
    }

    path.strip_prefix(base).is_some_and(|rest| rest.starts_with('/'))
}

/// Resolves `raw` for revealing in the file manager: it must be absolute (`InvalidInput`) and,
/// after resolving symlinks, lie inside one of `roots` (`Forbidden`). Returns the canonical path.
///
/// `NotFound` is reported only for a missing path that is lexically inside a root; a missing path
/// anywhere else is `Forbidden` too, so the command cannot be used to probe which files exist.
/// Paths with `..` components are refused outright (the app never sends them).
pub fn reveal_target(roots: &[PathBuf], raw: &Path) -> CommandResult<PathBuf> {
    if !raw.is_absolute() {
        return Err(BusinessError::InvalidInput("the path must be absolute".to_owned()).into());
    }

    if raw.components().any(|component| component == Component::ParentDir) {
        return Err(refused());
    }

    if let Some(target) = roots.iter().find_map(|root| canonical_within(root, raw).ok()) {
        return Ok(target);
    }

    let lexically_inside = roots.iter().any(|root| raw.starts_with(root));

    if lexically_inside && !raw.exists() {
        Err(BusinessError::NotFound(raw.display().to_string()).into())
    } else {
        Err(refused())
    }
}

fn refused() -> CommandError {
    BusinessError::Forbidden("refused by the access rules".to_owned()).into()
}

#[cfg(test)]
mod tests {
    use harness_business::model::IpcErrorCode;

    use super::*;
    use crate::test_support::TempDir;

    fn allowlist() -> Vec<String> {
        link_allowlist(&HarnessRegistry::with_defaults())
    }

    fn assert_refused(raw: &str) {
        let error = allowed_link(&allowlist(), raw).expect_err(raw);
        assert_eq!(error.code(), IpcErrorCode::Forbidden, "{raw}");
    }

    #[test]
    fn harness_and_project_links_are_allowed() {
        let registry = HarnessRegistry::with_defaults();
        let mut allowed: Vec<String> = registry.all().map(|harness| harness.info().homepage).collect();
        allowed.extend(registry.all().map(|harness| harness.install_hint().docs_url));
        allowed.extend(
            [
                "https://github.com/MSpiechowicz/harness-useful-dashboard",
                "https://github.com/MSpiechowicz/harness-useful-desktop/releases?tab=latest#v1",
                "https://github.com/earendil-works/pi",
                "https://github.com/earendil-works/pi/",
                "https://github.com/earendil-works/pi/tree/main/docs",
                "https://github.com/earendil-works/pi#readme",
            ]
            .map(str::to_owned),
        );

        for raw in allowed {
            let url = allowed_link(&allowlist(), &raw).expect(&raw);
            assert!(url.as_str().starts_with("https://"), "{raw}");
        }
    }

    #[test]
    fn insecure_and_foreign_links_are_refused() {
        for raw in [
            "http://github.com/MSpiechowicz/harness-useful-dashboard",
            "http://omp.sh",
            "https://github.com/MSpiechowicz.evil.com",
            "https://github.com.evil.com/MSpiechowicz/",
            "https://github.com@evil.com/MSpiechowicz/",
            "https://user:pass@github.com/MSpiechowicz/",
            "https://github.com:8443/MSpiechowicz/",
            "https://github.com/earendil-works/pi-evil",
            "https://github.com/earendil-works/pi.evil",
            "https://github.com/MSpiechowicz/../attacker/repo",
            "https://github.com/MSpiechowicz/%2e%2e/attacker",
            "https://github.com/mspiechowicz/",
            "https://github.com/",
            "https://omp.sh.evil.com/",
            "javascript:alert(1)",
            "file:///etc/passwd",
            "data:text/html,hi",
            "github.com/MSpiechowicz/",
            "",
        ] {
            assert_refused(raw);
        }
    }

    #[test]
    fn any_secure_link_opens_only_over_https_without_credentials() {
        let url = secure_link("https://github.com/someone/repo/pull/12").expect("allowed");
        assert_eq!(url.as_str(), "https://github.com/someone/repo/pull/12");

        for raw in [
            "http://github.com/someone/repo/pull/12",
            "https://user:pass@github.com/",
            "javascript:alert(1)",
            "file:///etc/passwd",
            "data:text/html,hi",
            "github.com/someone/",
            "",
        ] {
            let error = secure_link(raw).expect_err(raw);
            assert_eq!(error.code(), IpcErrorCode::Forbidden, "{raw}");
        }
    }

    #[test]
    fn opened_links_are_the_normalized_form() {
        let url = allowed_link(&allowlist(), "HTTPS://GitHub.com/MSpiechowicz/x/./y").expect("allowed");

        assert_eq!(url.as_str(), "https://github.com/MSpiechowicz/x/y");
    }

    #[test]
    fn reveal_accepts_existing_paths_inside_a_root() {
        let dir = TempDir::new("reveal-ok");
        let root = dir.path().join("root");
        std::fs::create_dir_all(root.join("skills")).expect("create");
        std::fs::write(root.join("skills/SKILL.md"), "x").expect("write");

        let roots = [dir.path().join("missing-root"), root.clone()];

        let file = reveal_target(&roots, &root.join("skills/SKILL.md")).expect("inside");
        let itself = reveal_target(&roots, &root).expect("the root itself");

        assert!(file.ends_with("skills/SKILL.md"));
        assert!(itself.ends_with("root"));
    }

    #[test]
    fn reveal_refuses_paths_outside_every_root() {
        let dir = TempDir::new("reveal-outside");
        let root = dir.path().join("root");
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&root).expect("create");
        std::fs::create_dir_all(&outside).expect("create");

        let roots = [root.clone()];

        let sibling = reveal_target(&roots, &outside).expect_err("sibling");
        let dotdot = reveal_target(&roots, &root.join("../outside")).expect_err("dot-dot");
        let missing_outside = reveal_target(&roots, &outside.join("missing")).expect_err("missing outside");
        let missing_inside = reveal_target(&roots, &root.join("missing")).expect_err("missing inside");
        let relative = reveal_target(&roots, Path::new("root")).expect_err("relative");

        assert_eq!(sibling.code(), IpcErrorCode::Forbidden);
        assert_eq!(dotdot.code(), IpcErrorCode::Forbidden);
        assert_eq!(missing_outside.code(), IpcErrorCode::Forbidden);
        assert_eq!(missing_inside.code(), IpcErrorCode::NotFound);
        assert_eq!(relative.code(), IpcErrorCode::InvalidInput);
    }

    #[cfg(unix)]
    #[test]
    fn reveal_refuses_symlink_escapes() {
        let dir = TempDir::new("reveal-symlink");
        let root = dir.path().join("root");
        let secret = dir.path().join("secret");
        std::fs::create_dir_all(&root).expect("create");
        std::fs::create_dir_all(&secret).expect("create");
        std::os::unix::fs::symlink(&secret, root.join("link")).expect("symlink");

        let error = reveal_target(std::slice::from_ref(&root), &root.join("link")).expect_err("escape");

        assert_eq!(error.code(), IpcErrorCode::Forbidden);
    }
}
