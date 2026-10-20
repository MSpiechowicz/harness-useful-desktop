//! Harness Memory's team database settings in its `config.toml`: where the file is on each OS,
//! what `memory_remote_get` shows (the URL redacted, the token as a flag), how `memory_remote_set`
//! saves, keeps, clears and removes values, what it refuses, and how the file is written.

mod common;
mod memory_remote_support;

use std::path::{Path, PathBuf};

use common::set_mode;
use harness_business::BusinessError;
use harness_business::ecosystem::{
    MemoryRemoteChange, ScriptsSupport, memory_config_path, memory_remote_get, memory_remote_set,
};
use harness_business::env::{Env, OsKind};
use harness_business::model::{MemoryNotesRouting, MemoryRemote, MemoryRemoteKind};
use memory_remote_support::postgres_change;
use pretty_assertions::assert_eq;

const PASSWORD: &str = "pa55-w0rd-secret";
const TOKEN: &str = "tok-e2e-secret";

/// A Linux environment rooted at a fresh home, and the config file in it.
fn home() -> (tempfile::TempDir, Env, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let env = Env::for_tests(dir.path()).with_os(OsKind::Linux);
    let path = dir.path().join(".config/harness-memory/config.toml");

    (dir, env, path)
}

fn change(kind: MemoryRemoteKind, url: Option<&str>, token: Option<&str>) -> MemoryRemoteChange {
    MemoryRemoteChange {
        kind: Some(kind),
        url: url.map(str::to_owned),
        token: token.map(str::to_owned),
        ..postgres_change()
    }
}

fn off() -> MemoryRemoteChange {
    MemoryRemoteChange {
        enabled: false,
        kind: None,
        url: None,
        ..change(MemoryRemoteKind::Postgres, None, None)
    }
}

/// The settings as an app sees them whose memory server predates scripts.
fn get(env: &Env) -> MemoryRemote {
    memory_remote_get(env, ScriptsSupport::Unsupported)
}

/// Saves `change` for a memory server that predates scripts.
fn set(env: &Env, change: MemoryRemoteChange) -> Result<MemoryRemote, BusinessError> {
    memory_remote_set(env, change, ScriptsSupport::Unsupported)
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
    set_mode(path, 0o600);
}

/// The parsed file.
fn saved(path: &Path) -> toml::Table {
    toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn invalid_input(result: Result<MemoryRemote, BusinessError>) -> String {
    match result {
        Err(BusinessError::InvalidInput(message)) => message,
        other => panic!("expected invalid input, got {other:?}"),
    }
}

#[test]
fn the_config_file_is_in_each_os_config_folder() {
    let linux = Env::for_tests("/home/u").with_os(OsKind::Linux);
    let file = |dir: &str| Some(Path::new(dir).join("harness-memory").join("config.toml"));

    assert_eq!(memory_config_path(&linux), file("/home/u/.config"));
    assert_eq!(
        memory_config_path(&linux.clone().with_var("XDG_CONFIG_HOME", "/xdg")),
        file("/xdg")
    );
    assert_eq!(
        memory_config_path(&linux.with_var("XDG_CONFIG_HOME", "relative/dir")),
        file("/home/u/.config"),
        "a relative XDG_CONFIG_HOME does not count"
    );

    let mac = Env::for_tests("/Users/u").with_os(OsKind::MacOs);
    assert_eq!(memory_config_path(&mac), file("/Users/u/Library/Application Support"));

    let windows = Env::for_tests("/Users/u").with_os(OsKind::Windows);
    assert_eq!(memory_config_path(&windows), None);
    assert_eq!(
        memory_config_path(&windows.with_var("APPDATA", "/Users/u/AppData/Roaming")),
        file("/Users/u/AppData/Roaming")
    );
}

#[test]
fn without_a_file_team_memory_is_off() {
    let (_dir, env, _) = home();

    assert_eq!(
        get(&env),
        MemoryRemote {
            enabled: false,
            kind: None,
            url_redacted: None,
            has_token: false,
            repository_notes: MemoryNotesRouting::Remote,
            global_notes: MemoryNotesRouting::Remote,
            scripts: MemoryNotesRouting::Remote,
            scripts_supported: false,
            trust_team_scripts: false,
            trust_team_scripts_supported: false,
            hooks_supported: false,
            share_hooks: false,
            trust_team_hooks: false,
            routines_supported: false,
            share_routines: false,
            trust_team_routines: false,
            documentdb_supported: false,
            team_and_local_supported: false,
            scripts_team_and_local_supported: false,
            config_path: "~/.config/harness-memory/config.toml".to_owned(),
            problem: None,
        }
    );
}

#[test]
fn windows_without_appdata_reports_the_unknown_folder() {
    let dir = tempfile::tempdir().unwrap();
    let env = Env::for_tests(dir.path()).with_os(OsKind::Windows);

    let remote = get(&env);
    assert!(!remote.enabled);
    assert!(remote.problem.unwrap().contains("APPDATA"));
    assert!(matches!(
        set(&env, change(MemoryRemoteKind::Postgres, Some("postgres://h/db"), None)),
        Err(BusinessError::Unsupported(_))
    ));
}

#[test]
fn a_saved_url_is_shown_without_password_query_or_token() {
    let (_dir, env, path) = home();
    write(
        &path,
        &format!(
            "database_url = \"postgresql://team:{PASSWORD}@db.example.com:5432/memory?sslmode=require&password={PASSWORD}\"\n\
             database_token = \"{TOKEN}\"\nrepository_notes = \"local\"\n"
        ),
    );

    let remote = get(&env);

    assert!(remote.enabled);
    assert_eq!(remote.kind, Some(MemoryRemoteKind::Postgres));
    assert_eq!(
        remote.url_redacted.as_deref(),
        Some("postgresql://team@db.example.com:5432/memory")
    );
    assert!(remote.has_token);
    assert_eq!(remote.repository_notes, MemoryNotesRouting::Local);
    assert_eq!(remote.global_notes, MemoryNotesRouting::Remote);
    assert_eq!(remote.problem, None);

    let wire = serde_json::to_string(&remote).unwrap();
    assert!(!wire.contains(PASSWORD) && !wire.contains(TOKEN), "{wire}");
}

#[test]
fn every_scheme_names_its_kind() {
    let (_dir, env, path) = home();
    let cases = [
        ("POSTGRES://h/db", Some(MemoryRemoteKind::Postgres)),
        ("libsql://team.turso.io", Some(MemoryRemoteKind::Libsql)),
        ("https://sqld.example.com", Some(MemoryRemoteKind::Libsql)),
        (
            "mongodb+srv://u:p@cluster.example.com/memory",
            Some(MemoryRemoteKind::Mongodb),
        ),
        ("mysql://h/db", None),
    ];

    for (url, kind) in cases {
        write(&path, &format!("database_url = \"{url}\"\n"));
        let remote = get(&env);

        assert_eq!(remote.kind, kind, "{url}");
        assert_eq!(remote.problem.is_some(), kind.is_none(), "{url}");
    }
}

#[test]
fn saving_keeping_clearing_and_removing() {
    let (_dir, env, path) = home();
    let url = "libsql://team-memory.turso.io";

    let remote = set(&env, change(MemoryRemoteKind::Libsql, Some(url), Some(TOKEN))).unwrap();
    assert!(remote.enabled && remote.has_token);
    assert_eq!(remote.url_redacted.as_deref(), Some(url));
    assert_eq!(saved(&path)["database_token"].as_str(), Some(TOKEN));

    // Null URL and token keep the saved ones; the routing is written every time.
    let mut keep = change(MemoryRemoteKind::Libsql, None, None);
    keep.global_notes = MemoryNotesRouting::Local;
    let remote = set(&env, keep).unwrap();
    assert!(remote.has_token);
    assert_eq!(remote.global_notes, MemoryNotesRouting::Local);
    let file = saved(&path);
    assert_eq!(file["database_url"].as_str(), Some(url));
    assert_eq!(file["repository_notes"].as_str(), Some("remote"));
    assert_eq!(file["global_notes"].as_str(), Some("local"));

    // An empty token clears it.
    let remote = set(&env, change(MemoryRemoteKind::Libsql, None, Some(""))).unwrap();
    assert!(!remote.has_token);
    assert!(!saved(&path).contains_key("database_token"));

    // Off removes the file.
    let remote = set(&env, off()).unwrap();
    assert!(!remote.enabled);
    assert!(!path.exists());
    assert!(set(&env, off()).is_ok(), "off without a file is fine");
}

#[test]
fn a_kept_url_must_exist_and_match_the_kind() {
    let (_dir, env, path) = home();

    let message = invalid_input(set(&env, change(MemoryRemoteKind::Postgres, None, None)));
    assert!(message.contains("enter the database URL"), "{message}");

    set(&env, change(MemoryRemoteKind::Postgres, Some("postgres://h/db"), None)).unwrap();
    let message = invalid_input(set(&env, change(MemoryRemoteKind::Mongodb, None, None)));
    assert!(message.contains("not a MongoDB URL"), "{message}");
    assert_eq!(saved(&path)["database_url"].as_str(), Some("postgres://h/db"));
}

#[test]
fn postgres_and_mongodb_save_no_token() {
    let (_dir, env, path) = home();
    set(
        &env,
        change(MemoryRemoteKind::Libsql, Some("libsql://t.turso.io"), Some(TOKEN)),
    )
    .unwrap();

    let remote = set(
        &env,
        change(MemoryRemoteKind::Postgres, Some("postgres://u@h/db"), Some(TOKEN)),
    )
    .unwrap();

    assert!(!remote.has_token);
    assert!(!saved(&path).contains_key("database_token"));
}

#[test]
fn urls_and_tokens_are_validated_without_echoing_them() {
    let (_dir, env, path) = home();
    let too_long = format!("postgres://h/{}", "d".repeat(4096));
    let cases = [
        (
            MemoryRemoteKind::Libsql,
            "postgres://h/db".to_owned(),
            "must start with libsql://",
        ),
        (
            MemoryRemoteKind::Postgres,
            "mongodb://h/db".to_owned(),
            "must start with postgres://",
        ),
        (
            MemoryRemoteKind::Mongodb,
            "h/db".to_owned(),
            "must start with mongodb://",
        ),
        (
            MemoryRemoteKind::Libsql,
            format!("libsql://u:{PASSWORD}@t.turso.io"),
            "no user, password",
        ),
        (
            MemoryRemoteKind::Libsql,
            format!("https://t.turso.io?authToken={PASSWORD}"),
            "query",
        ),
        (
            MemoryRemoteKind::Libsql,
            format!("libsql://t.turso.io#{PASSWORD}"),
            "fragment",
        ),
        (
            MemoryRemoteKind::Libsql,
            "http://sqld.example.com".to_owned(),
            "only allowed for this machine",
        ),
        (
            MemoryRemoteKind::Postgres,
            format!("postgres://u:{PASSWORD}@h/db\nx = 1"),
            "control characters",
        ),
        (
            MemoryRemoteKind::Postgres,
            format!("postgres://u:{PASSWORD}\u{7}@h/db"),
            "control characters",
        ),
        (MemoryRemoteKind::Postgres, too_long, "longer than 4 KiB"),
        (MemoryRemoteKind::Postgres, "   ".to_owned(), "enter the database URL"),
    ];

    for (kind, url, expected) in cases {
        let message = invalid_input(set(&env, change(kind, Some(&url), None)));

        assert!(message.contains(expected), "{url:?}: {message}");
        assert!(!message.contains(PASSWORD), "{message}");
    }

    let long_token = "t".repeat(4097);
    for (token, expected) in [
        (long_token.as_str(), "longer than 4 KiB"),
        ("a\nb", "control characters"),
    ] {
        let message = invalid_input(set(
            &env,
            change(MemoryRemoteKind::Libsql, Some("libsql://t.turso.io"), Some(token)),
        ));
        assert!(message.contains(expected), "{message}");
    }

    assert!(!path.exists(), "nothing was written");

    for url in [
        "  HTTP://localhost:8080  ",
        "http://127.0.0.1:8080/",
        "http://[::1]:8080",
        "LIBSQL://t.turso.io",
    ] {
        let remote = set(&env, change(MemoryRemoteKind::Libsql, Some(url), None)).unwrap();
        assert_eq!(saved(&path)["database_url"].as_str(), Some(url.trim()));
        assert_eq!(remote.kind, Some(MemoryRemoteKind::Libsql));
    }
}

#[test]
fn escaping_round_trips_quotes_backslashes_and_unicode() {
    let (_dir, env, path) = home();
    let password = r#"p"a\ss'w=ó🔑\"#;
    let url = format!("postgres://team:{password}@db.example.com/memory");
    let token = r#"t"o\k€n"#;

    set(&env, change(MemoryRemoteKind::Postgres, Some(&url), None)).unwrap();
    assert_eq!(saved(&path)["database_url"].as_str(), Some(url.as_str()));

    set(
        &env,
        change(MemoryRemoteKind::Libsql, Some("libsql://ł.turso.io"), Some(token)),
    )
    .unwrap();
    let file = saved(&path);
    assert_eq!(file["database_url"].as_str(), Some("libsql://ł.turso.io"));
    assert_eq!(file["database_token"].as_str(), Some(token));
}

#[test]
fn other_keys_are_reported_and_the_file_is_left_alone() {
    let (_dir, env, path) = home();
    let text = "database_url = \"postgres://h/db\"\ndatabse_token = \"x\"\n[extra]\nkey = 1\n";
    write(&path, text);

    let problem = get(&env).problem.unwrap();
    assert!(
        problem.contains("databse_token") && problem.contains("extra"),
        "{problem}"
    );

    let message = invalid_input(set(&env, change(MemoryRemoteKind::Postgres, None, None)));
    assert!(message.contains("does not know"), "{message}");
    invalid_input(set(&env, off()));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
}

#[test]
fn a_file_that_is_not_toml_is_reported_without_its_content_and_left_alone() {
    let (_dir, env, path) = home();
    let text = format!("database_url = \"postgres://u:{PASSWORD}@h/db\n");
    write(&path, &text);

    let problem = get(&env).problem.unwrap();
    assert!(problem.contains("not valid TOML (line 1)"), "{problem}");
    assert!(!problem.contains(PASSWORD));

    invalid_input(set(
        &env,
        change(MemoryRemoteKind::Postgres, Some("postgres://h/db"), None),
    ));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
}

#[test]
fn wrong_values_are_reported_and_replaced_on_save() {
    let (_dir, env, path) = home();
    write(&path, "database_url = \"postgres://h/db\"\nglobal_notes = \"team\"\n");

    let remote = get(&env);
    assert_eq!(remote.global_notes, MemoryNotesRouting::Remote);
    assert!(remote.problem.unwrap().contains("global_notes must be"));

    write(&path, "database_url = 5\n");
    let remote = get(&env);
    assert!(!remote.enabled);
    assert!(remote.problem.unwrap().contains("database_url must be a string"));

    let remote = set(&env, change(MemoryRemoteKind::Postgres, Some("postgres://h/db"), None)).unwrap();
    assert_eq!(remote.problem, None);
}

#[cfg(unix)]
mod unix {
    use std::os::unix::fs::{PermissionsExt, symlink};

    use pretty_assertions::assert_eq;

    use super::*;

    fn mode(path: &Path) -> u32 {
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn the_file_is_private_and_its_folder_too() {
        let (dir, env, path) = home();

        set(&env, change(MemoryRemoteKind::Postgres, Some("postgres://h/db"), None)).unwrap();

        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(path.parent().unwrap()), 0o700);
        let leftovers: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().collect();
        assert_eq!(leftovers.len(), 1, "no temporary file is left");
        drop(dir);
    }

    #[test]
    fn loose_permissions_are_reported_and_fixed_by_saving() {
        let (_dir, env, path) = home();
        write(&path, "database_url = \"postgres://h/db\"\n");

        set_mode(&path, 0o644);
        assert!(get(&env).problem.unwrap().contains("can read"));

        set_mode(&path, 0o666);
        assert!(get(&env).problem.unwrap().contains("can change"));

        set(&env, change(MemoryRemoteKind::Postgres, None, None)).unwrap();
        assert_eq!(mode(&path), 0o600);
        assert_eq!(get(&env).problem, None);
    }

    #[test]
    fn a_symlinked_file_or_folder_is_never_written_through() {
        let (dir, env, path) = home();
        let elsewhere = dir.path().join("elsewhere.toml");
        write(&elsewhere, "database_url = \"postgres://h/db\"\n");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        symlink(&elsewhere, &path).unwrap();

        let remote = get(&env);
        assert!(remote.enabled, "the linked file is still read");
        assert!(remote.problem.unwrap().contains("symbolic link"));

        let message = invalid_input(set(
            &env,
            change(MemoryRemoteKind::Postgres, Some("postgres://other/db"), None),
        ));
        assert!(message.contains("symbolic link"), "{message}");
        invalid_input(set(&env, off()));
        assert!(std::fs::read_to_string(&elsewhere).unwrap().contains("postgres://h/db"));

        // A linked folder.
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(path.parent().unwrap()).unwrap();
        let real = dir.path().join("real-folder");
        std::fs::create_dir_all(&real).unwrap();
        symlink(&real, path.parent().unwrap()).unwrap();

        let error = set(&env, change(MemoryRemoteKind::Postgres, Some("postgres://h/db"), None));
        assert!(invalid_input(error).contains("symbolic link"));
        assert!(!real.join("config.toml").exists());
    }
}
