//! Amazon DocumentDB as the team database in Harness Memory's `config.toml`: a `mongodb://` URL is
//! DocumentDB when `database_kind = "documentdb"` (any case), MongoDB when it says `mongodb`, and
//! otherwise by its host, as the server picks it. Saving DocumentDB writes that key, only while the
//! installed server lists `documentdb` in `harness-memory kinds` (an older server refuses a file
//! with the key), saving MongoDB at a DocumentDB host writes `mongodb` on such a server, and any
//! other save removes the key. A saved URL is tested as the other of the two kinds only from a
//! file the server would use.

mod common;
mod memory_cli_support;
mod memory_remote_support;

use common::set_mode;
use harness_business::BusinessError;
use harness_business::ecosystem::{
    KindsSupport, MemoryRemoteChange, MemorySupport, ScriptsSupport, memory_remote_get, memory_remote_set,
};
use harness_business::model::{MemoryRemote, MemoryRemoteKind};
use memory_remote_support::{Home, postgres_change};
use pretty_assertions::assert_eq;

const CLUSTER: &str = "mongodb://team@memory.cluster-c1.eu-west-1.docdb.amazonaws.com:27017/memory?tls=true";
const MONGODB: &str = "mongodb+srv://team@cluster0.example.mongodb.net/memory";

fn support(kinds: KindsSupport) -> MemorySupport {
    MemorySupport {
        kinds,
        ..MemorySupport::from(ScriptsSupport::Unsupported)
    }
}

fn with_documentdb() -> MemorySupport {
    support(KindsSupport::Listed { documentdb: true })
}

fn change(kind: MemoryRemoteKind, url: Option<&str>) -> MemoryRemoteChange {
    MemoryRemoteChange {
        kind: Some(kind),
        url: url.map(str::to_owned),
        ..postgres_change()
    }
}

impl Home {
    fn write(&self, text: &str) {
        let path = self.config();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        set_mode(&path, 0o600);
    }

    fn get(&self) -> MemoryRemote {
        memory_remote_get(&self.env, with_documentdb())
    }

    fn set(&self, change: MemoryRemoteChange, support: MemorySupport) -> Result<MemoryRemote, BusinessError> {
        memory_remote_set(&self.env, change, support)
    }

    fn saved(&self) -> toml::Table {
        toml::from_str(&std::fs::read_to_string(self.config()).unwrap()).unwrap()
    }
}

#[test]
fn the_kind_of_a_saved_mongodb_url_follows_the_key_then_the_host() {
    let home = Home::new();
    let cases = [
        (format!("database_url = \"{CLUSTER}\"\n"), MemoryRemoteKind::Documentdb),
        (
            "database_url = \"mongodb://u@a.docdb-elastic.amazonaws.com/m\"\n".to_owned(),
            MemoryRemoteKind::Documentdb,
        ),
        (
            "database_url = \"mongodb://DB.Cluster-1.cn-north-1.DOCDB.amazonaws.com.cn./m\"\n".to_owned(),
            MemoryRemoteKind::Documentdb,
        ),
        (
            "database_url = \"mongodb://a.example.com:27017,b.cluster-1.docdb.amazonaws.com:27017/m\"\n".to_owned(),
            MemoryRemoteKind::Documentdb,
        ),
        (format!("database_url = \"{MONGODB}\"\n"), MemoryRemoteKind::Mongodb),
        (
            "database_url = \"mongodb://u@docdb.amazonaws.com.example.com/m?host=x.docdb.amazonaws.com\"\n".to_owned(),
            MemoryRemoteKind::Mongodb,
        ),
        (
            format!("database_url = \"{MONGODB}\"\ndatabase_kind = \"DocumentDB\"\n"),
            MemoryRemoteKind::Documentdb,
        ),
        (
            format!("database_url = \"{CLUSTER}\"\ndatabase_kind = \"mongodb\"\n"),
            MemoryRemoteKind::Mongodb,
        ),
    ];

    for (text, kind) in cases {
        home.write(&text);
        let remote = home.get();

        assert_eq!(remote.kind, Some(kind), "{text}");
        assert_eq!(remote.problem, None, "{text}");
    }
}

#[test]
fn an_unknown_database_kind_is_reported_and_the_host_decides() {
    let home = Home::new();
    home.write(&format!("database_url = \"{CLUSTER}\"\ndatabase_kind = \"cosmos\"\n"));

    let remote = home.get();
    assert_eq!(remote.kind, Some(MemoryRemoteKind::Documentdb));
    assert_eq!(
        remote.problem.as_deref(),
        Some("database_kind must be \"mongodb\" or \"documentdb\"")
    );

    let saved = home
        .set(change(MemoryRemoteKind::Documentdb, None), with_documentdb())
        .unwrap();
    assert_eq!(saved.problem, None, "saving replaces the value");
    assert_eq!(home.saved()["database_kind"].as_str(), Some("documentdb"));
}

#[test]
fn documentdb_supported_follows_the_kinds_probe() {
    let home = Home::new();
    let cases = [
        (KindsSupport::Listed { documentdb: true }, true),
        (KindsSupport::Listed { documentdb: false }, false),
        (KindsSupport::Unsupported, false),
        (KindsSupport::NotInstalled, false),
        (KindsSupport::Unknown, false),
    ];

    for (kinds, supported) in cases {
        assert_eq!(
            memory_remote_get(&home.env, support(kinds)).documentdb_supported,
            supported,
            "{kinds:?}"
        );
    }

    assert!(!memory_remote_get(&home.env, ScriptsSupport::Approvals).documentdb_supported);
}

#[test]
fn saving_documentdb_writes_the_key_and_other_kinds_remove_it() {
    let home = Home::new();

    let remote = home
        .set(change(MemoryRemoteKind::Documentdb, Some(MONGODB)), with_documentdb())
        .unwrap();
    assert_eq!(remote.kind, Some(MemoryRemoteKind::Documentdb));
    let file = home.saved();
    assert_eq!(file["database_url"].as_str(), Some(MONGODB));
    assert_eq!(file["database_kind"].as_str(), Some("documentdb"));

    // MongoDB keeps the saved URL (same schemes) and removes the key.
    let remote = home
        .set(change(MemoryRemoteKind::Mongodb, None), with_documentdb())
        .unwrap();
    assert_eq!(remote.kind, Some(MemoryRemoteKind::Mongodb));
    assert!(!home.saved().contains_key("database_kind"));

    // And back: a kept MongoDB URL may be saved as DocumentDB.
    home.set(change(MemoryRemoteKind::Documentdb, None), with_documentdb())
        .unwrap();
    assert_eq!(home.saved()["database_kind"].as_str(), Some("documentdb"));

    // Another kind with a new URL removes it too, whatever the server supports.
    let remote = home.set(postgres_change(), support(KindsSupport::Unsupported)).unwrap();
    assert_eq!(remote.kind, Some(MemoryRemoteKind::Postgres));
    assert!(!home.saved().contains_key("database_kind"));

    // A kept URL of another scheme cannot become DocumentDB.
    let refused = home.set(change(MemoryRemoteKind::Documentdb, None), with_documentdb());
    match refused {
        Err(BusinessError::InvalidInput(message)) => {
            assert_eq!(
                message,
                "the saved URL is not an Amazon DocumentDB URL, so enter the new one"
            );
        }
        other => panic!("expected invalid input, got {other:?}"),
    }
}

#[test]
fn a_documentdb_url_must_be_a_mongodb_one() {
    let home = Home::new();

    let refused = home.set(
        change(MemoryRemoteKind::Documentdb, Some("postgres://h/db")),
        with_documentdb(),
    );

    match refused {
        Err(BusinessError::InvalidInput(message)) => assert_eq!(
            message,
            "an Amazon DocumentDB URL must start with mongodb:// or mongodb+srv://"
        ),
        other => panic!("expected invalid input, got {other:?}"),
    }
}

#[test]
fn documentdb_is_refused_while_the_server_cannot_use_it() {
    let home = Home::new();
    home.set(change(MemoryRemoteKind::Mongodb, Some(CLUSTER)), with_documentdb())
        .unwrap();
    let before = std::fs::read_to_string(home.config()).unwrap();

    let cases = [
        (
            KindsSupport::Listed { documentdb: false },
            "Update Harness Memory to use Amazon DocumentDB",
        ),
        (
            KindsSupport::Unsupported,
            "Update Harness Memory to use Amazon DocumentDB",
        ),
        (
            KindsSupport::NotInstalled,
            "Install Harness Memory to use Amazon DocumentDB",
        ),
        (
            KindsSupport::Unknown,
            "Harness Memory could not say whether it can use Amazon DocumentDB. Try again.",
        ),
    ];

    for (kinds, reason) in cases {
        match home.set(change(MemoryRemoteKind::Documentdb, Some(CLUSTER)), support(kinds)) {
            Err(BusinessError::Unsupported(message)) => assert_eq!(message, reason),
            other => panic!("expected unsupported, got {other:?}"),
        }
    }

    assert_eq!(
        std::fs::read_to_string(home.config()).unwrap(),
        before,
        "the file is untouched"
    );
}

#[cfg(unix)]
#[test]
fn the_file_with_the_key_stays_private() {
    use std::os::unix::fs::PermissionsExt;

    let home = Home::new();
    home.set(change(MemoryRemoteKind::Documentdb, Some(CLUSTER)), with_documentdb())
        .unwrap();

    let mode = std::fs::metadata(home.config()).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn mongodb_at_a_documentdb_host_stays_mongodb_on_a_server_that_lists_documentdb() {
    let home = Home::new();

    home.set(change(MemoryRemoteKind::Mongodb, Some(CLUSTER)), with_documentdb())
        .unwrap();
    assert_eq!(home.saved()["database_kind"].as_str(), Some("mongodb"));
    assert_eq!(home.get().kind, Some(MemoryRemoteKind::Mongodb));

    // A kept URL too: to DocumentDB and back.
    home.set(change(MemoryRemoteKind::Documentdb, None), with_documentdb())
        .unwrap();
    assert_eq!(home.saved()["database_kind"].as_str(), Some("documentdb"));
    let remote = home
        .set(change(MemoryRemoteKind::Mongodb, None), with_documentdb())
        .unwrap();
    assert_eq!(remote.kind, Some(MemoryRemoteKind::Mongodb));
    assert_eq!(home.saved()["database_kind"].as_str(), Some("mongodb"));

    // A server that does not list DocumentDB refuses the key (and does not detect the host).
    for kinds in [
        KindsSupport::Listed { documentdb: false },
        KindsSupport::Unsupported,
        KindsSupport::NotInstalled,
    ] {
        home.set(change(MemoryRemoteKind::Mongodb, Some(CLUSTER)), support(kinds))
            .unwrap();
        assert!(!home.saved().contains_key("database_kind"), "{kinds:?}");
    }

    // Without an answer the file is left alone: a current server would read DocumentDB.
    let before = home.saved();
    let refused = home.set(
        change(MemoryRemoteKind::Mongodb, Some(CLUSTER)),
        support(KindsSupport::Unknown),
    );
    assert!(refused.is_err());
    assert_eq!(home.saved(), before);
}

/// A saved DocumentDB URL tested as MongoDB without a typed one, from a config file the server
/// refuses: the server is never run with the file's URL.
#[cfg(unix)]
#[test]
fn a_saved_url_is_not_switched_out_of_a_file_the_server_refuses() {
    use harness_business::ecosystem::{memory_config_path, memory_remote_test};
    use harness_business::model::MemoryRemoteTest;
    use harness_business::process::ExecOutput;
    use memory_cli_support::{install, installed_at, stub};

    let dir = tempfile::tempdir().unwrap();
    let (env, server) = installed_at(dir.path());
    install(&server, None);
    let runner = stub(ExecOutput {
        status: Some(0),
        stdout: "ok: MongoDB\n".to_owned(),
        ..ExecOutput::default()
    });
    let check = || {
        let future = memory_remote_test(
            &env,
            &runner,
            MemoryRemoteKind::Mongodb,
            KindsSupport::Unknown,
            None,
            None,
        );
        tokio::runtime::Runtime::new().unwrap().block_on(future)
    };
    let refused = MemoryRemoteTest {
        ok: false,
        message: "the saved URL is not a MongoDB URL, so enter one to test it".to_owned(),
    };

    let config = memory_config_path(&env).unwrap();
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, format!("database_url = \"{CLUSTER}\"\n")).unwrap();

    set_mode(&config, 0o664);
    assert_eq!(check(), refused, "other users can change the file");

    let target = dir.path().join("elsewhere.toml");
    std::fs::rename(&config, &target).unwrap();
    set_mode(&target, 0o600);
    std::os::unix::fs::symlink(&target, &config).unwrap();
    assert_eq!(check(), refused, "the file is a symbolic link");

    assert!(runner.runs.lock().unwrap().is_empty(), "the server is never run");

    // The same file, private and in place, is switched.
    std::fs::remove_file(&config).unwrap();
    std::fs::rename(&target, &config).unwrap();
    assert!(check().ok);
    assert_eq!(runner.runs.lock().unwrap().len(), 1);
}
