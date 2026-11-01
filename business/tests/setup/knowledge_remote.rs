//! The team database of Harness Knowledge: the `[database]` table of the server's own config file
//! (read, saved, removed, never overwritten when it holds anything the server would refuse), the
//! kinds it takes (no Amazon DocumentDB), and `check --json` read into a typed problem with the
//! secrets masked out of every sentence.

use crate::knowledge_stand_in;
use crate::team_support;

use harness_business::BusinessError;
use harness_business::ecosystem::{
    KnowledgeRemoteChange, KnowledgeSupport, knowledge_config_path, knowledge_remote_get, knowledge_remote_set,
    knowledge_remote_test, read_check,
};
use harness_business::model::{KnowledgeIndexState, KnowledgeProblem, MemoryRemoteKind};
use knowledge_stand_in::{ALL_FEATURES, cli, installed};
use pretty_assertions::assert_eq;
use serde_json::json;
use team_support::{answers, exits, wait};

const POSTGRES: &str = "postgres://ana:s3cr3t-pw@db.example.com:5432/knowledge";
const SUPPORT: KnowledgeSupport = KnowledgeSupport::Unknown;

fn change(kind: MemoryRemoteKind, url: Option<&str>, token: Option<&str>) -> KnowledgeRemoteChange {
    KnowledgeRemoteChange {
        enabled: true,
        kind: Some(kind),
        url: url.map(str::to_owned),
        token: token.map(str::to_owned),
        repository_documents: None,
        global_documents: None,
    }
}

#[test]
fn a_saved_database_is_a_table_with_the_url_and_reads_back_redacted() {
    let (_dir, env) = installed();

    let empty = knowledge_remote_get(&env, SUPPORT);
    assert!(!empty.enabled && empty.problem.is_none() && empty.url_redacted.is_none());

    let saved = knowledge_remote_set(
        &env,
        change(MemoryRemoteKind::Postgres, Some(POSTGRES), Some("ignored")),
        SUPPORT,
    )
    .unwrap();

    assert!(saved.enabled && !saved.has_token, "PostgreSQL takes no token");
    assert_eq!(saved.kind, Some(MemoryRemoteKind::Postgres));
    assert_eq!(
        saved.url_redacted.as_deref(),
        Some("postgres://ana@db.example.com:5432/knowledge")
    );
    assert!(
        saved.config_path.ends_with("harness-knowledge/config.toml")
            || saved.config_path.ends_with("harness-knowledge\\config.toml")
    );

    let text = std::fs::read_to_string(knowledge_config_path(&env).unwrap()).unwrap();
    assert!(text.starts_with("# Harness Knowledge team database"));
    assert!(text.contains(&format!("[database]\nurl = \"{POSTGRES}\"\n")), "{text}");
    assert!(!text.contains("token"));

    let removed = knowledge_remote_set(
        &env,
        KnowledgeRemoteChange {
            enabled: false,
            kind: None,
            url: None,
            token: None,
            repository_documents: None,
            global_documents: None,
        },
        SUPPORT,
    )
    .unwrap();
    assert!(!removed.enabled);
    assert!(!knowledge_config_path(&env).unwrap().exists());
}

#[test]
fn a_libsql_token_is_saved_kept_and_removed() {
    let (_dir, env) = installed();
    let url = "libsql://knowledge-acme.turso.io";

    let saved = knowledge_remote_set(
        &env,
        change(MemoryRemoteKind::Libsql, Some(url), Some(" tok-123 ")),
        SUPPORT,
    )
    .unwrap();
    assert!(saved.has_token);
    let text = std::fs::read_to_string(knowledge_config_path(&env).unwrap()).unwrap();
    assert!(text.contains("token = \"tok-123\""), "{text}");

    let kept = knowledge_remote_set(&env, change(MemoryRemoteKind::Libsql, None, None), SUPPORT).unwrap();
    assert!(kept.has_token);

    let cleared = knowledge_remote_set(&env, change(MemoryRemoteKind::Libsql, None, Some("")), SUPPORT).unwrap();
    assert!(!cleared.has_token);
    assert_eq!(cleared.url_redacted.as_deref(), Some(url));
}

#[test]
fn amazon_documentdb_and_urls_the_server_refuses_are_not_saved() {
    let (_dir, env) = installed();
    let docdb = "mongodb://u:p@cluster.cluster-abc.eu-west-1.docdb.amazonaws.com:27017/knowledge";

    for refused in [
        change(MemoryRemoteKind::Documentdb, Some(docdb), None),
        change(MemoryRemoteKind::Mongodb, Some(docdb), None),
    ] {
        assert!(matches!(
            knowledge_remote_set(&env, refused, SUPPORT),
            Err(BusinessError::Unsupported(message)) if message.contains("DocumentDB")
        ));
    }

    for bad in [
        change(MemoryRemoteKind::Postgres, Some("mysql://x/y"), None),
        change(MemoryRemoteKind::Postgres, None, None),
        change(MemoryRemoteKind::Libsql, Some("libsql://user:pw@host"), None),
    ] {
        assert!(matches!(
            knowledge_remote_set(&env, bad, SUPPORT),
            Err(BusinessError::InvalidInput(_))
        ));
    }

    assert!(!knowledge_config_path(&env).unwrap().exists());
}

#[test]
fn a_file_with_keys_the_server_refuses_is_reported_and_left_alone() {
    let (_dir, env) = installed();
    let path = knowledge_config_path(&env).unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();

    for text in [
        "[database]\nurl = \"postgres://h/k\"\ndatabase_kind = \"mongodb\"\n",
        "database_url = \"postgres://h/k\"\n",
        "database = \"postgres://h/k\"\n",
    ] {
        std::fs::write(&path, text).unwrap();
        #[cfg(unix)]
        std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();

        assert!(knowledge_remote_get(&env, SUPPORT).problem.is_some(), "{text}");
        assert!(matches!(
            knowledge_remote_set(&env, change(MemoryRemoteKind::Postgres, Some(POSTGRES), None), SUPPORT),
            Err(BusinessError::InvalidInput(_))
        ));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text, "never overwritten");
    }
}

#[test]
fn a_saved_documentdb_host_and_an_unknown_scheme_are_reported_as_problems() {
    let (_dir, env) = installed();
    let path = knowledge_config_path(&env).unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();

    std::fs::write(&path, "[database]\nurl = \"mongodb://h.docdb.amazonaws.com/k\"\n").unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
    assert!(
        knowledge_remote_get(&env, SUPPORT)
            .problem
            .unwrap()
            .contains("DocumentDB")
    );

    std::fs::write(&path, "[database]\nurl = \"ftp://h/k\"\n").unwrap();
    assert!(knowledge_remote_get(&env, SUPPORT).problem.unwrap().contains("scheme"));
}

#[test]
fn every_problem_code_is_typed_and_the_secrets_are_masked() {
    let codes = [
        ("no-mongot", KnowledgeProblem::NoMongot),
        ("no-pgvector", KnowledgeProblem::NoPgvector),
        ("pgvector-no-privilege", KnowledgeProblem::PgvectorNoPrivilege),
        ("pgvector-too-old", KnowledgeProblem::PgvectorTooOld),
        ("libsql-no-vector", KnowledgeProblem::LibsqlNoVector),
        ("no-fts5", KnowledgeProblem::NoFts5),
        ("index-building", KnowledgeProblem::IndexBuilding),
        ("unreachable", KnowledgeProblem::Unreachable),
        ("auth", KnowledgeProblem::Auth),
        ("something-newer", KnowledgeProblem::Other),
    ];
    let secrets = vec![POSTGRES.to_owned(), "s3cr3t-pw".to_owned()];

    for (code, problem) in codes {
        let stdout = json!({
            "ok": false, "backend": "postgres", "schema": 1,
            "capabilities": {"keyword": true, "vector": false, "vectorIndex": "building"},
            "model": {"dims": 384}, "problem": code, "message": "could not use s3cr3t-pw here"
        })
        .to_string();

        let check = read_check(&stdout, "", Some(3), &secrets);

        assert!(!check.ok, "{code}");
        assert_eq!(check.problem, Some(problem), "{code}");
        assert_eq!(check.message.as_deref(), Some("could not use •••• here"), "{code}");
        let capabilities = check.capabilities.unwrap();
        assert!(capabilities.keyword && !capabilities.vector);
        assert_eq!(capabilities.vector_index, KnowledgeIndexState::Building);
    }
}

#[test]
fn a_ready_database_and_the_other_shapes_of_an_answer() {
    let ready = json!({
        "ok": true, "backend": "sqlite", "schema": 1,
        "capabilities": {"keyword": true, "vector": true, "vectorIndex": "ready"},
        "model": {"dims": 384, "id": "multilingual-e5-small"}
    })
    .to_string();
    let check = read_check(&ready, "", Some(0), &[]);
    assert!(check.ok && check.problem.is_none());
    assert_eq!(check.backend.as_deref(), Some("sqlite"));
    assert_eq!(check.model.as_deref(), Some("multilingual-e5-small"));
    assert_eq!(check.capabilities.unwrap().vector_index, KnowledgeIndexState::Ready);

    // A capability missing before the check ran its own probes: the generic missing shape.
    let missing = read_check(
        "{\"error\":\"missing\",\"code\":\"no-fts5\",\"message\":\"no FTS5\"}\n",
        "",
        Some(3),
        &[],
    );
    assert!(!missing.ok);
    assert_eq!(missing.problem, Some(KnowledgeProblem::NoFts5));
    assert!(missing.capabilities.is_none());

    // A failure and an unsupported database print their own object, or nothing on stdout.
    let failed = read_check(
        "{\"error\":\"failed\",\"message\":\"bad config\"}\n",
        "harness-knowledge: bad config\n",
        Some(1),
        &[],
    );
    assert!(!failed.ok && failed.problem.is_none());
    assert_eq!(failed.message.as_deref(), Some("bad config"));

    let silent = read_check("", "warning\nharness-knowledge: no home folder\n", Some(1), &[]);
    assert_eq!(silent.message.as_deref(), Some("harness-knowledge: no home folder"));
    assert!(
        read_check("", "", Some(2), &[])
            .message
            .unwrap()
            .contains("exit code 2")
    );
}

#[test]
fn the_test_runs_check_json_with_the_typed_url_in_the_environment() {
    let (_dir, env) = installed();
    let unreachable = json!({"ok": false, "backend": "postgres", "schema": 1,
        "capabilities": {"keyword": false, "vector": false, "vectorIndex": "missing"},
        "model": {"dims": 384}, "problem": "unreachable", "message": format!("cannot reach {POSTGRES}")})
    .to_string();
    let server_cli = cli(ALL_FEATURES).starting(&["check"], exits(3, &unreachable, ""));

    let check = wait(knowledge_remote_test(
        &env,
        &server_cli,
        MemoryRemoteKind::Postgres,
        Some(POSTGRES.to_owned()),
        None,
    ));

    assert_eq!(check.problem, Some(KnowledgeProblem::Unreachable));
    assert_eq!(check.message.as_deref(), Some("cannot reach ••••"));

    let spec = server_cli.runs.lock().unwrap().last().unwrap().clone();
    assert_eq!(spec.args, ["check", "--json"]);
    assert!(
        spec.env
            .iter()
            .any(|(name, value)| name == "HARNESS_KNOWLEDGE_DATABASE_URL" && value == POSTGRES)
    );
    assert!(
        spec.env
            .iter()
            .all(|(name, _)| name != "HARNESS_KNOWLEDGE_DATABASE_TOKEN"),
        "PostgreSQL has no token"
    );
    assert!(
        !format!("{:?}", spec.args).contains("s3cr3t"),
        "never on the command line"
    );
}

#[test]
fn a_test_that_cannot_run_says_why() {
    let (_dir, env) = installed();
    let server_cli = cli(ALL_FEATURES).starting(&["check"], answers(json!({})));
    let test = |kind, url: Option<&str>| {
        wait(knowledge_remote_test(
            &env,
            &server_cli,
            kind,
            url.map(str::to_owned),
            None,
        ))
        .message
        .unwrap()
    };

    assert!(test(MemoryRemoteKind::Documentdb, Some(POSTGRES)).contains("DocumentDB"));
    let docdb = "mongodb://u:p@cluster.cluster-abc.eu-west-1.docdb.amazonaws.com:27017/knowledge";
    assert!(test(MemoryRemoteKind::Mongodb, Some(docdb)).contains("DocumentDB"));
    assert!(test(MemoryRemoteKind::Postgres, Some("mysql://x/y")).contains("must start with"));
    assert!(test(MemoryRemoteKind::Postgres, None).contains("no database URL is saved"));
    assert!(
        server_cli.commands().iter().all(|command| command[0] != "check"),
        "only the version was asked"
    );

    let dir = tempfile::tempdir().unwrap();
    let bare = harness_business::env::Env::for_tests(dir.path());
    assert_eq!(
        wait(knowledge_remote_test(
            &bare,
            &server_cli,
            MemoryRemoteKind::Postgres,
            Some(POSTGRES.to_owned()),
            None
        ))
        .message
        .as_deref(),
        Some("Harness Knowledge is not installed")
    );
}

#[test]
fn routing_is_saved_in_its_own_table_and_only_when_a_place_is_not_the_default() {
    use harness_business::model::MemoryNotesRouting;

    let (_dir, env) = installed();
    let routing = KnowledgeSupport::Listed(harness_business::model::KnowledgeFeatures {
        routing: true,
        ..Default::default()
    });
    let mut routed = change(MemoryRemoteKind::Postgres, Some(POSTGRES), None);
    routed.repository_documents = Some(MemoryNotesRouting::RemoteLocal);
    routed.global_documents = Some(MemoryNotesRouting::Local);

    let saved = knowledge_remote_set(&env, routed, routing).unwrap();

    assert_eq!(saved.repository_documents, MemoryNotesRouting::RemoteLocal);
    assert_eq!(saved.global_documents, MemoryNotesRouting::Local);
    assert!(saved.routing_supported);
    let text = std::fs::read_to_string(knowledge_config_path(&env).unwrap()).unwrap();
    assert!(
        text.contains("[routing]\nrepository_documents = \"remote+local\"\nglobal_documents = \"local\"\n"),
        "{text}"
    );

    // Saving the connection again keeps the routing, and back to remote leaves no table for an old server to refuse.
    let kept = knowledge_remote_set(&env, change(MemoryRemoteKind::Postgres, None, None), routing).unwrap();
    assert_eq!(kept.repository_documents, MemoryNotesRouting::RemoteLocal);

    let mut back = change(MemoryRemoteKind::Postgres, None, None);
    back.repository_documents = Some(MemoryNotesRouting::Remote);
    back.global_documents = Some(MemoryNotesRouting::Remote);
    let reset = knowledge_remote_set(&env, back, routing).unwrap();
    assert_eq!(reset.global_documents, MemoryNotesRouting::Remote);
    let text = std::fs::read_to_string(knowledge_config_path(&env).unwrap()).unwrap();
    assert!(!text.contains("routing"), "{text}");
}

#[test]
fn routing_is_refused_for_a_server_without_it_and_bad_tables_block_the_file() {
    use harness_business::model::MemoryNotesRouting;

    let (_dir, env) = installed();
    let mut routed = change(MemoryRemoteKind::Postgres, Some(POSTGRES), None);
    routed.global_documents = Some(MemoryNotesRouting::Local);

    assert!(matches!(
        knowledge_remote_set(&env, routed, SUPPORT),
        Err(BusinessError::Unsupported(_))
    ));
    assert!(!knowledge_config_path(&env).unwrap().exists());

    let path = knowledge_config_path(&env).unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "[routing]\nglobal_documents = \"remote+local\"\nextra = 1\n").unwrap();
    let read = knowledge_remote_get(&env, SUPPORT);
    assert_eq!(read.global_documents, MemoryNotesRouting::RemoteLocal);
    assert!(read.problem.is_some(), "an unknown key blocks the file");

    std::fs::write(&path, "[routing]\nglobal_documents = \"nowhere\"\n").unwrap();
    assert!(knowledge_remote_get(&env, SUPPORT).problem.is_some());
}

#[test]
fn routing_values_read_as_the_server_reads_them_and_an_unknown_one_blocks_saving() {
    use harness_business::model::MemoryNotesRouting;

    let (_dir, env) = installed();
    knowledge_remote_set(&env, change(MemoryRemoteKind::Postgres, Some(POSTGRES), None), SUPPORT).unwrap();

    // Rewritten in place, so the file keeps the permissions the app gave it.
    let path = knowledge_config_path(&env).unwrap();

    // The server trims and ignores case, so the app must not see `remote` there.
    let text = "[database]\nurl = \"postgres://ana:pw@db.example.com/k\"\n[routing]\nrepository_documents = \" Remote+Local \"\nglobal_documents = \"LOCAL\"\n";
    std::fs::write(&path, text).unwrap();
    let read = knowledge_remote_get(&env, SUPPORT);
    assert_eq!(read.repository_documents, MemoryNotesRouting::RemoteLocal);
    assert_eq!(read.global_documents, MemoryNotesRouting::Local);
    assert!(read.problem.is_none(), "{:?}", read.problem);

    // A value the server refuses is not silently replaced with `remote` by a save.
    std::fs::write(&path, text.replace("LOCAL", "somewhere")).unwrap();
    let blocked = knowledge_remote_set(&env, change(MemoryRemoteKind::Postgres, None, None), SUPPORT);
    assert!(matches!(blocked, Err(BusinessError::InvalidInput(_))), "{blocked:?}");
    assert!(std::fs::read_to_string(&path).unwrap().contains("somewhere"));
}

#[test]
fn a_routing_variable_wins_over_the_file_and_is_reported_as_set() {
    use harness_business::model::MemoryNotesRouting;

    let (_dir, env) = installed();
    let routing = KnowledgeSupport::Listed(harness_business::model::KnowledgeFeatures {
        routing: true,
        ..Default::default()
    });
    let env = env.with_var("HARNESS_KNOWLEDGE_REPOSITORY_DOCUMENTS", " Local ");

    let mut routed = change(MemoryRemoteKind::Postgres, Some(POSTGRES), None);
    routed.repository_documents = Some(MemoryNotesRouting::RemoteLocal);
    routed.global_documents = Some(MemoryNotesRouting::RemoteLocal);
    let saved = knowledge_remote_set(&env, routed, routing).unwrap();

    assert_eq!(
        saved.repository_documents,
        MemoryNotesRouting::Local,
        "the variable's value"
    );
    assert!(saved.repository_documents_env && !saved.global_documents_env);
    assert_eq!(saved.global_documents, MemoryNotesRouting::RemoteLocal);

    // The chosen value for the variable's place never reached the file.
    let text = std::fs::read_to_string(knowledge_config_path(&env).unwrap()).unwrap();
    assert!(text.contains("repository_documents = \"remote\""), "{text}");
    assert!(text.contains("global_documents = \"remote+local\""), "{text}");
}
