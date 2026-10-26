//! Deny-list, `SafeReader`, path containment and `Env` helpers.

use std::fs;
use std::path::Path;

use harness_business::BusinessError;
use harness_business::env::Env;
use harness_business::security::{
    DenyReason, SafeReader, canonical_within, is_denied, is_safe_arg, validate_project_dir,
};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

fn write(dir: &Path, name: &str, contents: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, contents).unwrap();
    path
}

#[test]
fn deny_list_matches_extensions_names_prefixes_and_directories() {
    let denied = [
        "/h/.omp/agent/history.db",
        "/h/x.DB-WAL",
        "/h/x.db-shm",
        "/h/x.db-journal",
        "/h/x.sqlite",
        "/h/x.sqlite3",
        "/h/tool.log",
        "/h/cert.pem",
        "/h/server.key",
        "/h/a.p12",
        "/h/a.pfx",
        "/h/.pi/agent/auth.json",
        "/h/.pi/agent/MCP-AUTH.json",
        "/h/.omp/install-id",
        "/h/.config/harness-dashboard/auth-token",
        "/h/.pi/agent/models-store.json",
        "/h/.omp/gpu_cache.json",
        "/h/.aws/credentials",
        "/h/credentials.json",
        "/h/proj/.env",
        "/h/proj/.env.production",
        "/h/.ssh/id_rsa",
        "/h/.ssh/id_ed25519.pub",
        "/h/.omp/agent/blobs/abc",
        "/h/.omp/agent/terminal-sessions/1.json",
        "/h/.omp/agent/sessions/-p/x/url-search/r.json",
    ];

    for path in denied {
        assert!(is_denied(Path::new(path)).is_some(), "{path} must be denied");
    }

    let allowed = [
        "/h/.omp/agent/config.yml",
        "/h/.omp/agent/mcp.json",
        "/h/.pi/agent/settings.json",
        "/h/proj/blobs/data.json",
        "/h/proj/environment.json",
        "/h/.omp/agent/sessions/-p/2026_abc.jsonl",
    ];

    for path in allowed {
        assert_eq!(is_denied(Path::new(path)), None, "{path} must be allowed");
    }

    assert_eq!(
        is_denied(Path::new("/h/a.log")),
        Some(DenyReason::Extension("log".into()))
    );
}

#[test]
fn deny_list_covers_opencode_stores_without_catching_generic_project_folders() {
    let denied = [
        "/h/.local/share/opencode/auth.json",
        "/h/.local/share/opencode/mcp-auth.json",
        "/h/.local/share/opencode/opencode.db",
        "/h/.local/share/opencode/opencode-beta.db",
        "/h/.local/share/opencode/opencode.db-wal",
        "/h/.local/share/opencode/opencode.db-shm",
        "/h/.local/share/opencode/log/2026-10-16.log",
        "/h/.local/share/opencode/storage/session/x.json",
        "/h/.local/share/opencode/storage/part/msg_1/prt_1.json",
        "/h/.local/share/opencode/repos/github.com/acme/app/README.md",
        "/h/.local/share/opencode/snapshot/proj/config",
        "/h/.local/share/opencode/tool-output/tool_abc",
        "/h/.local/share/opencode/plans/1760600000000-brave-otter.md",
        "/h/.local/state/opencode/prompt-history.jsonl",
        "C:/Users/me/.local/state/opencode/Prompt-History.jsonl",
        "/h/.cache/opencode/node_modules/opencode-plugin/package.json",
        "C:/Users/me/.cache/opencode/node_modules/x/package.json",
        "/h/.config/ai.opencode.desktop/opencode.settings.dat",
        "/h/.config/ai.opencode.desktop/opencode.global.dat",
        "/h/.config/ai.opencode.desktop/opencode.workspace.abc123.dat",
        "/h/.config/ai.opencode.desktop/OpenCode.Workspace.ABC.DAT",
    ];

    for path in denied {
        assert!(is_denied(Path::new(path)).is_some(), "{path} must be denied");
    }

    let allowed = [
        "/h/.config/opencode/opencode.json",
        "/h/.config/opencode/opencode.jsonc",
        "/h/.config/opencode/tui.json",
        "/h/.config/opencode/package.json",
        "/h/proj/.opencode/agents/review.md",
        "/h/myclone/storage/x.json",
        "/h/myclone/repos/list.json",
        "/h/myclone/snapshot/state.json",
        "/h/myclone/node_modules/left-pad/package.json",
        "/h/.config/opencode/node_modules/plugin/package.json",
        "/h/myclone/opencode/notes/storage.json",
        "/h/myclone/settings.dat",
        "/h/myclone/tool-output/result.txt",
        "/h/myclone/plans/roadmap.md",
        "/h/proj/.opencode/plans/roadmap.md",
        "/h/myclone/prompt-history.jsonl",
        "/h/myclone/opencode/notes/prompt-history.jsonl",
    ];

    for path in allowed {
        assert_eq!(is_denied(Path::new(path)), None, "{path} must be allowed");
    }

    assert_eq!(
        is_denied(Path::new("/h/.local/share/opencode/storage/session/x.json")),
        Some(DenyReason::Directory("storage".into()))
    );
    assert_eq!(
        is_denied(Path::new("/h/.local/state/opencode/prompt-history.jsonl")),
        Some(DenyReason::FileName("prompt-history.jsonl".into()))
    );
}

#[test]
fn safe_reader_refuses_the_opencode_prompt_history_and_tool_outputs() {
    let dir = TempDir::new().unwrap();
    let history = write(dir.path(), ".local/state/opencode/prompt-history.jsonl", "{}");
    let tool_output = write(dir.path(), ".local/share/opencode/tool-output/tool_abc", "secret");
    let plan = write(dir.path(), ".local/share/opencode/plans/plan.md", "# plan");

    for path in [history, tool_output, plan] {
        assert!(
            matches!(
                SafeReader::read_to_string(&path, 1024),
                Err(BusinessError::Forbidden(_))
            ),
            "{path:?} must be refused"
        );
    }
}

#[test]
fn safe_reader_refuses_opencode_transcripts_but_reads_a_project_storage_folder() {
    let dir = TempDir::new().unwrap();
    let transcript = write(dir.path(), ".local/share/opencode/storage/session/x.json", "{}");
    let project = write(dir.path(), "myclone/storage/x.json", "{}");

    assert!(matches!(
        SafeReader::read_to_string(&transcript, 1024),
        Err(BusinessError::Forbidden(_))
    ));
    assert_eq!(SafeReader::read_to_string(&project, 1024).unwrap(), "{}");
}

#[test]
fn read_to_string_returns_allowed_files_and_refuses_denied_ones() {
    let dir = TempDir::new().unwrap();
    let config = write(dir.path(), "config.yml", "theme: dark\n");
    let secret = write(dir.path(), "auth.json", "{}");

    assert_eq!(SafeReader::read_to_string(&config, 1024).unwrap(), "theme: dark\n");
    assert!(matches!(
        SafeReader::read_to_string(&secret, 1024),
        Err(BusinessError::Forbidden(_))
    ));
}

#[test]
fn read_to_string_enforces_size_type_utf8_and_existence() {
    let dir = TempDir::new().unwrap();
    let big = write(dir.path(), "big.json", &"x".repeat(2048));
    let binary = dir.path().join("bin.json");
    fs::write(&binary, [0xff, 0xfe, 0x00]).unwrap();

    let too_big = SafeReader::read_to_string(&big, 1024);
    let not_file = SafeReader::read_to_string(dir.path(), 1024);
    let not_utf8 = SafeReader::read_to_string(&binary, 1024);
    let missing = SafeReader::read_to_string(&dir.path().join("nope.json"), 1024);

    assert!(matches!(too_big, Err(BusinessError::InvalidInput(_))));
    assert!(matches!(not_file, Err(BusinessError::InvalidInput(_))));
    assert!(matches!(not_utf8, Err(BusinessError::InvalidInput(_))));
    assert!(matches!(missing, Err(BusinessError::NotFound(_))));
}

#[cfg(unix)]
#[test]
fn symlinks_to_denied_files_are_refused() {
    use std::os::unix::fs::symlink;

    let dir = TempDir::new().unwrap();
    let target = write(dir.path(), "store/agent.db", "secret");
    let renamed = write(dir.path(), "store/plain.txt", "secret");
    let denied_hop = dir.path().join("store/auth.json");
    symlink(&renamed, &denied_hop).unwrap();

    let direct = dir.path().join("innocent.json");
    let chained = dir.path().join("chained.json");
    symlink(&target, &direct).unwrap();
    symlink(&denied_hop, &chained).unwrap();

    assert!(matches!(
        SafeReader::read_to_string(&direct, 1024),
        Err(BusinessError::Forbidden(_))
    ));
    assert!(matches!(
        SafeReader::read_to_string(&chained, 1024),
        Err(BusinessError::Forbidden(_))
    ));
}

#[cfg(unix)]
#[test]
fn fifos_are_refused_without_blocking() {
    let dir = TempDir::new().unwrap();
    let fifo = dir.path().join("pipe.json");
    nix::unistd::mkfifo(&fifo, nix::sys::stat::Mode::S_IRWXU).unwrap();

    assert!(matches!(
        SafeReader::read_to_string(&fifo, 1024),
        Err(BusinessError::InvalidInput(_))
    ));
}

#[test]
fn read_first_lines_splits_on_newline_only_and_strips_one_carriage_return() {
    let dir = TempDir::new().unwrap();
    let path = write(
        dir.path(),
        "s.jsonl",
        "{\"a\":\"x\u{2028}y\"}\r\nsecond\r\r\nthird\nfourth",
    );

    let lines = SafeReader::read_first_lines(&path, 3, 64).unwrap();
    assert_eq!(lines, ["{\"a\":\"x\u{2028}y\"}", "second\r", "third"]);

    let all = SafeReader::read_first_lines(&path, 10, 64).unwrap();
    assert_eq!(all.len(), 4);
    assert_eq!(all[3], "fourth");
}

#[test]
fn read_first_lines_rejects_overlong_lines_and_denied_files() {
    let dir = TempDir::new().unwrap();
    let long = write(dir.path(), "s.jsonl", &format!("{}\nshort\n", "x".repeat(100)));
    let log = write(dir.path(), "tool.log", "line\n");

    assert!(matches!(
        SafeReader::read_first_lines(&long, 2, 10),
        Err(BusinessError::InvalidInput(_))
    ));
    assert!(matches!(
        SafeReader::read_first_lines(&log, 1, 10),
        Err(BusinessError::Forbidden(_))
    ));
    assert_eq!(SafeReader::read_first_lines(&long, 1, 100).unwrap()[0].len(), 100);
}

#[test]
fn canonical_within_accepts_inner_paths_and_rejects_dot_dot_escapes() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("root");
    write(&root, "sub/file.json", "{}");
    write(dir.path(), "outside.json", "{}");

    let inner = canonical_within(&root, Path::new("sub/../sub/file.json")).unwrap();
    assert!(inner.ends_with("sub/file.json"));

    let escape = canonical_within(&root, Path::new("sub/../../outside.json"));
    assert!(matches!(escape, Err(BusinessError::Forbidden(_))));

    let missing = canonical_within(&root, Path::new("nope.json"));
    assert!(matches!(missing, Err(BusinessError::NotFound(_))));
}

#[cfg(unix)]
#[test]
fn canonical_within_rejects_symlink_escapes() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("root");
    fs::create_dir_all(&root).unwrap();
    let outside = write(dir.path(), "outside/secret.json", "{}");
    std::os::unix::fs::symlink(outside.parent().unwrap(), root.join("link")).unwrap();

    let escape = canonical_within(&root, Path::new("link/secret.json"));
    assert!(matches!(escape, Err(BusinessError::Forbidden(_))));
}

#[test]
fn validate_project_dir_requires_an_existing_absolute_non_root_directory() {
    let home = TempDir::new().unwrap();
    let project = home.path().join("proj");
    fs::create_dir_all(&project).unwrap();
    let file = write(home.path(), "file.txt", "");
    let env = Env::for_tests(home.path());

    let expected = dunce::canonicalize(&project).unwrap();
    assert_eq!(validate_project_dir(&env, &project).unwrap(), expected);
    assert_eq!(validate_project_dir(&env, Path::new("~/proj")).unwrap(), expected);

    let relative = validate_project_dir(&env, Path::new("proj"));
    let not_dir = validate_project_dir(&env, &file);
    let missing = validate_project_dir(&env, &home.path().join("nope"));
    let root = validate_project_dir(&env, Path::new(if cfg!(windows) { "C:\\" } else { "/" }));

    assert!(matches!(relative, Err(BusinessError::InvalidInput(_))));
    assert!(matches!(not_dir, Err(BusinessError::InvalidInput(_))));
    assert!(matches!(missing, Err(BusinessError::NotFound(_))));
    assert!(matches!(root, Err(BusinessError::InvalidInput(_))));
}

#[test]
fn safe_args_use_a_restricted_charset_without_leading_dash() {
    for arg in [
        "harness-useful-sidebar@harness-useful-sidebar",
        "npm:@scope/pkg@1.2.3",
        "a=b,c+d",
        "user/repo",
    ] {
        assert!(is_safe_arg(arg), "{arg}");
    }

    for arg in [
        "", "-rf", "--scope", "a b", "a;b", "a&b", "a|b", "$(x)", "a%PATH%", "a\"b", "ä",
    ] {
        assert!(!is_safe_arg(arg), "{arg:?}");
    }
}

#[test]
fn env_expands_home_and_ignores_empty_variables() {
    let env = Env::for_tests("/home/test")
        .with_var("PATH", "")
        .with_var("APPDATA", "C:\\AppData");

    assert_eq!(env.expand_home("~"), Path::new("/home/test"));
    assert_eq!(env.expand_home("~/x/y"), Path::new("/home/test/x/y"));
    assert_eq!(env.expand_home("/abs"), Path::new("/abs"));
    assert_eq!(env.expand_home("~other"), Path::new("~other"));
    assert_eq!(env.var("PATH"), None);
    assert_eq!(env.var("APPDATA"), Some("C:\\AppData"));
    assert_eq!(env.var("MISSING"), None);
}
