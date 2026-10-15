//! Validators for shared sources and displayed fields: npm specs are classified the way npm reads
//! them, git and URL sources need a real host, paths hide after any number of slashes, dates are
//! real dates, and invisible characters never reach the review.

use harness_business::model::SyncInvalidReason;
use harness_business::sync::validate;
use pretty_assertions::assert_eq;

/// `npm:<spec>` and `npm:pkg@<spec>`.
fn both_npm_forms(spec: &str) -> [String; 2] {
    [format!("npm:{spec}"), format!("npm:pkg@{spec}")]
}

fn assert_refused(sources: &[impl AsRef<str>], reason: SyncInvalidReason) {
    for source in sources.iter().map(AsRef::as_ref) {
        assert_eq!(validate::package_source(source), Err(reason), "{source}");
    }
}

#[test]
fn npm_specs_naming_a_path_or_an_archive_are_local_paths() {
    let local: Vec<String> = [
        "/dev/shm/x",
        "/etc/x",
        "/data/pkg",
        ".",
        ".hidden/x",
        ".local/x",
        "x.tgz",
        "x.TGZ",
        "x.tar",
        "x.tar.gz",
        "x.zip",
        "c:x",
        "git+file:///tmp/x",
        "git+file:/srv/x",
        "file:x",
        "link:x",
    ]
    .iter()
    .flat_map(|spec| both_npm_forms(spec))
    .collect();
    assert_refused(&local, SyncInvalidReason::LocalPath);

    for source in both_npm_forms("~/x").iter().chain(&both_npm_forms("~x")) {
        assert!(validate::package_source(source).is_err(), "{source}");
    }
}

#[test]
fn npm_specs_over_plain_http_or_the_git_protocol_are_unsupported() {
    let unsupported: Vec<String> = [
        "http://registry.example.com/x.tgz",
        "http://registry.example.com/x",
        "https://registry.example.com/x.tgz",
        "git+http://example.com/owner/repo",
        "git://example.com/owner/repo",
        "svn:example.com/x",
    ]
    .iter()
    .flat_map(|spec| both_npm_forms(spec))
    .collect();
    assert_refused(&unsupported, SyncInvalidReason::UnsupportedSource);

    for source in ["npm:https://-x/owner/repo", "npm:git+https://-x/owner/repo"] {
        assert_eq!(
            validate::package_source(source),
            Err(SyncInvalidReason::UnsafeValue),
            "{source}"
        );
    }
}

#[test]
fn registry_names_versions_and_remote_repositories_stay_valid() {
    for source in [
        "npm:pkg",
        "npm:fine@1.0.0",
        "npm:name@1.2.3",
        "npm:@scope/pkg",
        "npm:@scope/pkg@1.2.0",
        "npm:pkg@latest",
        "npm:pkg@1.2.0-beta.1",
        "npm:pkg@1.x",
        "npm:github:owner/repo",
        "npm:pkg@github:owner/repo",
        "npm:https://github.com/owner/repo",
        "npm:git+https://github.com/owner/repo.git",
        "npm:pkg@git+ssh://github.com/owner/repo.git",
        "git:github.com/owner/repo",
        "git:github.com/owner/repo@v1",
        "git:git@github.com:owner/repo",
        "git:https://github.com/owner/repo",
        "https://github.com/owner/repo",
        "https://gitlab.example.com:8443/owner/repo",
    ] {
        assert_eq!(validate::package_source(source), Ok(()), "{source}");
    }
}

#[test]
fn git_and_url_sources_need_a_host_that_is_not_an_option_or_a_file() {
    assert_refused(
        &["git:file:///tmp/x", "git:FILE:/srv/x", "git:file:x"],
        SyncInvalidReason::LocalPath,
    );
    assert_refused(
        &[
            "git:ssh://github.com/owner/repo",
            "git:git+ssh://github.com/owner/repo",
            "git:http://example.com/owner/repo",
        ],
        SyncInvalidReason::UnsupportedSource,
    );
    assert_refused(
        &[
            "https://-x/a/b",
            "git:https://-x/a/b",
            "git:git@-oProxyCommand=x:a/b",
            "git:-oProxyCommand=x/a/b",
            "https://_x/a/b",
        ],
        SyncInvalidReason::UnsafeValue,
    );
    assert!(validate::package_source("git:ssh://-oProxyCommand=x/a/b").is_err());

    assert_eq!(
        validate::marketplace_source("https://-x/a/b"),
        Err(SyncInvalidReason::UnsafeValue)
    );
    assert_eq!(validate::marketplace_source("https://github.com/owner/repo"), Ok(()));
}

#[test]
fn paths_after_several_slashes_or_an_at_sign_are_local_paths() {
    for value in [
        "sqlite:///home/alice/x.db",
        "--db=sqlite:///home/alice/x.db",
        "file:////Users/alice/x",
        "@/home/alice/x",
        "--config=@/etc/x",
        "dirs=a,//home/alice/x",
    ] {
        assert!(validate::is_local_path(value), "{value:?}");
    }

    for value in [
        "@scope/pkg",
        "npm:@scope/pkg@1.2.0",
        "https://github.com/home/repo",
        "https://home.example.com/x",
        "http://media/mcp",
        "user@host.example.com",
    ] {
        assert!(!validate::is_local_path(value), "{value:?}");
    }
}

#[test]
fn dates_need_a_real_month_and_day() {
    for date in ["2026-01-01", "2026-12-31", "2026-10-15"] {
        assert_eq!(validate::date(date), Ok(()), "{date}");
    }

    for date in ["2026-00-10", "2026-13-01", "2026-99-99", "2026-10-00", "2026-10-32"] {
        assert_eq!(validate::date(date), Err(SyncInvalidReason::Malformed), "{date}");
    }
}

#[test]
fn invisible_fillers_and_variation_selectors_are_not_display_safe() {
    for character in [
        '\u{034F}',
        '\u{115F}',
        '\u{1160}',
        '\u{17B4}',
        '\u{17B5}',
        '\u{180B}',
        '\u{180F}',
        '\u{2800}',
        '\u{3164}',
        '\u{FE00}',
        '\u{FE0F}',
        '\u{FFA0}',
        '\u{E0100}',
        '\u{E01EF}',
    ] {
        let value = format!("ok{character}name");
        assert!(!validate::display_safe(&value), "U+{:04X}", u32::from(character));
        assert_eq!(validate::mcp_name(&value), Err(SyncInvalidReason::UnsafeCharacters));
    }

    assert!(validate::display_safe("한국어 server"));
    assert!(validate::display_safe("emoji 👍 and 日本語"));
}
