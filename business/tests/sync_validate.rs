//! Validators and secret detection for shared items: hostile values, sources, MCP fields, key
//! names and display safety are refused (never echoed), and anything the redactor would hide is
//! never shareable.

#[path = "sync_support/servers.rs"]
mod servers;

use harness_business::model::SyncInvalidReason;
use harness_business::sync::codec::decode_input;
use harness_business::sync::schema::Plugin;
use harness_business::sync::secrets::{args_exportable, exportable};
use harness_business::sync::validate;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use servers::{http, stdio};

fn bundle_json(harnesses: Value) -> String {
    json!({ "format": "harness-useful-desktop/setup", "version": 1, "harnesses": harnesses }).to_string()
}

#[test]
fn hostile_sources_are_invalid() {
    let hostile = [
        "--foo",
        "npm:x;rm -rf ~",
        "$(id)",
        "npm:`id`",
        "npm:x\ny",
        "../x",
        "npm:../x",
        "/etc/passwd",
        "~/x",
        "file:..",
        "npm:file:../x",
        "link:x",
        "C:\\x",
        "https://u:t@host/x",
        "git:tok@host/x",
        "npm:\u{202E}x",
        "ftp://host/x",
        "",
    ];

    for value in hostile {
        assert!(validate::package_source(value).is_err(), "package source {value:?}");
        assert!(
            validate::marketplace_source(value).is_err(),
            "marketplace source {value:?}"
        );
        assert!(validate::plugin_id(value).is_err(), "plugin id {value:?}");
        assert!(validate::mcp_command(value).is_err(), "command {value:?}");
    }

    let packages: Vec<Value> = hostile.iter().map(|source| json!({ "source": source })).collect();
    let decoded = decode_input(&bundle_json(json!({ "pi": { "packages": packages } }))).unwrap();
    assert_eq!(decoded.invalid_count(), hostile.len());

    assert_eq!(validate::package_source("../x"), Err(SyncInvalidReason::LocalPath));
    assert_eq!(
        validate::package_source("https://u:t@host/x"),
        Err(SyncInvalidReason::Credentials)
    );
    assert_eq!(
        validate::package_source("git:tok@host/x"),
        Err(SyncInvalidReason::Credentials)
    );
    assert_eq!(validate::package_source("--foo"), Err(SyncInvalidReason::UnsafeValue));
    assert_eq!(
        validate::package_source("npm:x\ny"),
        Err(SyncInvalidReason::UnsafeCharacters)
    );
    assert_eq!(
        validate::package_source("ftp://host/x"),
        Err(SyncInvalidReason::UnsupportedSource)
    );
}

#[test]
fn paths_embedded_in_a_value_are_local_paths() {
    for value in [
        "--root=/home/alice/proj",
        "--config=~/x",
        "--dir=C:/Users/x",
        "--dir=c:\\Users\\x",
        "--data=/var/lib/x",
        "PATH=/usr/bin:/home/alice/bin",
        "--mount:/mnt/data",
        "dirs=a,/Users/alice/x",
    ] {
        assert!(validate::is_local_path(value), "{value:?}");
    }

    for value in [
        "https://mcp.example.com/mcp",
        "https://github.com/home/repo",
        "https://host:8080/root/x",
        "@modelcontextprotocol/server-github",
        "npm:@scope/pkg@1.2.0",
        "git:git@github.com:owner/repo",
        "--flag=value",
        "--port=8080",
        "--range=~1.2",
        "-y",
    ] {
        assert!(!validate::is_local_path(value), "{value:?}");
    }
}

#[test]
fn package_sources_never_smuggle_an_option_a_path_or_plain_http() {
    for source in [
        "npm:--registry=https://x",
        "npm:--foo",
        "git:--upload-pack=x",
        "git:github.com/a/b@--x",
        "https://github.com/a/b@-x",
        "https://github.com/a/b#-x",
    ] {
        assert_eq!(
            validate::package_source(source),
            Err(SyncInvalidReason::UnsafeValue),
            "{source}"
        );
    }

    for source in [
        "npm:pkg@file:x",
        "npm:pkg@FILE:x",
        "npm:pkg@link:x",
        "npm:pkg@./x",
        "npm:pkg@../x",
        "npm:pkg@/abs/x",
        "npm:./x",
    ] {
        assert_eq!(
            validate::package_source(source),
            Err(SyncInvalidReason::LocalPath),
            "{source}"
        );
    }
    assert!(validate::package_source("npm:pkg@~/x").is_err());

    assert_eq!(
        validate::package_source("npm:pkg@http://registry.example.com/pkg.tgz"),
        Err(SyncInvalidReason::UnsupportedSource)
    );
    for source in [
        "npm:pkg@1.2.0-beta.1",
        "npm:@scope/pkg@2.0.0-rc.1",
        "git:github.com/a/b@v1-x",
    ] {
        assert_eq!(validate::package_source(source), Ok(()), "{source}");
    }
}

#[test]
fn versions_and_dates_follow_their_pattern() {
    for version in ["0.15.0", "1.0.0-beta.1+build.5", "v2.1", "3"] {
        assert_eq!(validate::version(Some(version)), Ok(()), "{version}");
    }
    assert_eq!(validate::version(None), Ok(()));
    for version in ["latest", "1.0 evil", "1.0.0\u{200B}", "", "--1"] {
        assert!(validate::version(Some(version)).is_err(), "{version:?}");
    }

    assert_eq!(validate::date("2026-10-15"), Ok(()));
    for date in ["Oct 15", "2026-10-15T00:00", "20261015", ""] {
        assert!(validate::date(date).is_err(), "{date:?}");
    }

    let decoded = decode_input(
        &json!({
            "format": "harness-useful-desktop/setup",
            "version": 1,
            "app": "click here to win",
            "createdAt": "yesterday",
            "harnesses": { "omp": { "plugins": [{ "id": "a@b", "version": "not a version" }] } },
        })
        .to_string(),
    )
    .unwrap();
    assert_eq!((&decoded.app, &decoded.created_at), (&None, &None));
    assert_eq!(decoded.invalid_count(), 1);
}

#[test]
fn a_marketplace_field_needs_a_marketplace_plugin_id() {
    let plugin = |id: &str, marketplace: &str| Plugin {
        id: id.to_owned(),
        marketplace: Some(marketplace.to_owned()),
        version: None,
    };

    assert_eq!(validate::plugin(&plugin("foo", "m")), Err(SyncInvalidReason::Malformed));
    assert_eq!(
        validate::plugin(&plugin("@scope/pkg", "m")),
        Err(SyncInvalidReason::Malformed)
    );
    assert_eq!(
        validate::plugin(&plugin("review@other", "m")),
        Err(SyncInvalidReason::Malformed)
    );
    assert_eq!(validate::plugin(&plugin("review@m", "m")), Ok(()));
}

#[test]
fn accepted_sources() {
    for source in [
        "npm:@scope/pkg@1.2.0",
        "npm:pkg",
        "git:github.com/owner/repo@v1",
        "git:git@github.com:owner/repo",
        "https://github.com/owner/repo",
    ] {
        assert_eq!(validate::package_source(source), Ok(()), "{source}");
    }

    for source in ["owner/repo", "https://github.com/owner/repo.git"] {
        assert_eq!(validate::marketplace_source(source), Ok(()), "{source}");
    }
    assert_eq!(
        validate::marketplace_source("git:github.com/owner/repo"),
        Err(SyncInvalidReason::UnsupportedSource)
    );

    for id in ["review@official", "pkg", "@scope/pkg"] {
        assert_eq!(validate::plugin_id(id), Ok(()), "{id}");
    }
}

#[test]
fn hostile_mcp_fields_are_invalid() {
    assert_eq!(
        validate::mcp_name("ok\u{202E}gnp"),
        Err(SyncInvalidReason::UnsafeCharacters)
    );
    assert_eq!(validate::mcp_name("-x"), Err(SyncInvalidReason::BadName));
    assert_eq!(validate::mcp_name(&"a".repeat(65)), Err(SyncInvalidReason::BadName));
    assert_eq!(validate::mcp_name("my.server_1-x"), Ok(()));

    // Shell interpreters as shared MCP commands.
    for shell in [
        "/bin/sh",                        // guard-allow: installer-shell
        "bash.exe",                       // guard-allow: installer-shell
        "BASH.EXE",                       // guard-allow: installer-shell
        "C:\\Windows\\System32\\cmd.exe", // guard-allow: installer-shell
        "pwsh",
        "powershell",
        "zsh",
        "fish",
        "dash",
    ] {
        assert_eq!(
            validate::mcp_command(shell),
            Err(SyncInvalidReason::ShellCommand),
            "{shell}"
        );
    }
    assert_eq!(validate::mcp_command("npx"), Ok(()));
    assert_eq!(
        validate::mcp_command("/usr/local/bin/server"),
        Err(SyncInvalidReason::LocalPath)
    );
    assert_eq!(validate::mcp_command(&"a".repeat(256)), Ok(()));
    assert_eq!(validate::mcp_command(&"a".repeat(257)), Err(SyncInvalidReason::TooLong));

    assert_eq!(
        validate::mcp_url("ftp://host/x"),
        Err(SyncInvalidReason::UnsupportedScheme)
    );
    assert_eq!(
        validate::mcp_url("https://u:t@host/x"),
        Err(SyncInvalidReason::Credentials)
    );
    assert_eq!(
        validate::mcp_url("https://host/x?api_key=abc"),
        Err(SyncInvalidReason::ContainsSecret)
    );
    assert_eq!(validate::mcp_url("http://localhost:3000/mcp"), Ok(()));
    let url = |length: usize| format!("https://h/{}", "a".repeat(length - "https://h/".len()));
    assert_eq!(validate::mcp_url(&url(2048)), Ok(()));
    assert_eq!(validate::mcp_url(&url(2049)), Err(SyncInvalidReason::TooLong));

    let args = |count: usize, length: usize| vec!["a".repeat(length); count];
    assert_eq!(validate::mcp_args(&args(64, 1024)), Ok(()));
    assert_eq!(validate::mcp_args(&args(65, 1)), Err(SyncInvalidReason::TooLong));
    assert_eq!(validate::mcp_args(&args(1, 1025)), Err(SyncInvalidReason::TooLong));
    assert_eq!(
        validate::mcp_args(&["a\u{2066}b".to_owned()]),
        Err(SyncInvalidReason::UnsafeCharacters)
    );
    assert_eq!(
        validate::mcp_args(&["--token".to_owned(), "sk-abcdefghijklmnopqrstu".to_owned()]),
        Err(SyncInvalidReason::ContainsSecret)
    );

    let mut both = stdio("x", "npx", &[]);
    both.url = Some("https://host".to_owned());
    assert_eq!(validate::mcp_server(&both), Err(SyncInvalidReason::Malformed));
    let mut no_url = http("x", "https://host");
    no_url.url = None;
    assert_eq!(validate::mcp_server(&no_url), Err(SyncInvalidReason::MissingField));
}

#[test]
fn key_names_follow_the_allowlist() {
    let keys = |names: &[&str]| names.iter().map(|name| (*name).to_owned()).collect::<Vec<_>>();

    assert_eq!(
        validate::key_names(&keys(&["GITHUB_TOKEN", "_x", "X-Api-Key", &"K".repeat(128)])),
        Ok(())
    );
    for bad in ["1ABC", "A B", "A=B", "", "Ä", "A;B", "-A", &"K".repeat(129)] {
        assert!(validate::key_names(&keys(&[bad])).is_err(), "{bad:?}");
    }
    assert_eq!(
        validate::key_names(&keys(&["A\u{202E}"])),
        Err(SyncInvalidReason::UnsafeCharacters)
    );
    assert_eq!(validate::key_names(&vec!["K".to_owned(); 64]), Ok(()));
    assert_eq!(
        validate::key_names(&vec!["K".to_owned(); 65]),
        Err(SyncInvalidReason::TooLong)
    );
}

#[test]
fn text_fields_are_capped_at_512_characters() {
    let source = |length: usize| format!("npm:{}", "a".repeat(length - 4));

    assert_eq!(validate::package_source(&source(512)), Ok(()));
    assert_eq!(validate::package_source(&source(513)), Err(SyncInvalidReason::TooLong));
}

#[test]
fn secrets_are_never_exportable() {
    let secret = [
        "ghp_abcdefghijklmnopqrstuvwxyz0123456789",
        "sk-abcdefghijklmnopqrstu",
        "Bearer abcdefgh12345678",
        "https://x-access-token:abc@github.com/o/r",
        "git:tok@host/x",
        "https://api.example.com/mcp?api_key=abc",
        "token=abc",
        "!pass show github",
        "aZ3$kq9!Lw2#xR7@mN5%vB8^cT1&yH4*",
    ];
    for value in secret {
        assert!(!exportable(value), "{value}");
    }

    for value in [
        "npx",
        "-y",
        "@modelcontextprotocol/server-github",
        "https://mcp.example.com/sse",
        "${GITHUB_TOKEN}",
        "$KEY",
        "npm:@scope/pkg@1.2.0",
        "git:git@github.com:o/r",
    ] {
        assert!(exportable(value), "{value}");
    }

    let args = |values: &[&str]| values.iter().map(|value| (*value).to_owned()).collect::<Vec<_>>();
    assert!(!args_exportable(&args(&["--token", "abc"])));
    assert!(!args_exportable(&args(&["--api-key", "abc"])));
    assert!(!args_exportable(&args(&["--token=abc"])));
    assert!(args_exportable(&args(&["--api-key", "${KEY}"])));
    assert!(args_exportable(&args(&["-y", "pkg", "--port", "3000"])));
    assert!(
        args_exportable(&args(&["server", "--token"])),
        "a trailing flag has no value"
    );

    // Short and abbreviated secret flags, and a value that only looks like a flag.
    for secret in [
        &["-p", "hunter2"][..],
        &["-P", "hunter2"],
        &["--key", "abc"],
        &["--pat", "abc"],
        &["--pass", "abc"],
        &["--pwd", "abc"],
        &["-p=hunter2"],
        &["--pat=abc"],
        &["--token", "-abc"],
        &["--token", "--verbose"],
    ] {
        assert!(!args_exportable(&args(secret)), "{secret:?}");
    }
    assert!(args_exportable(&args(&["-p", "$PASSWORD"])));
}

#[test]
fn display_safety_rejects_control_and_bidi_characters() {
    for value in [
        "a\u{0}",
        "a\u{1b}[31m",
        "a\u{7f}",
        "a\u{202A}",
        "a\u{202E}",
        "a\u{2066}",
        "a\u{2069}",
        "a\u{200F}",
        "a\u{200B}b",
        "a\u{2060}",
        "a\u{2028}",
        "a\u{2029}",
        "a\u{FEFF}",
        "a\u{00AD}b",
        "a\u{E0041}",
    ] {
        assert!(!validate::display_safe(value), "{value:?}");
    }
    assert!(validate::display_safe("zażółć server — ok"));
    assert!(validate::display_safe("emoji 👍 and 日本語"));
}
