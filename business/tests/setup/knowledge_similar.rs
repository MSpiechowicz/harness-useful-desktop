//! Finding the stored documents a selection of files is probably a version of (`docs-similar`):
//! the command's arguments, the answer read strictly, what only the intake's own paths can reach,
//! and the import manifest's `replacing`, written only for a server that lists the feature.

use crate::knowledge_stand_in;
use crate::team_support;

use harness_business::BusinessError;
use harness_business::ecosystem::{
    ImportManifest, KnowledgeFeatures, KnowledgeIntakes, MAX_SIMILAR_ENTRIES, similar_docs,
};
use harness_business::model::{
    KnowledgeId, KnowledgeImportOptions, KnowledgeLink, KnowledgeMatchReason, KnowledgePlace, KnowledgeReplacement,
};
use knowledge_stand_in::{ALL_FEATURES, REPO, cli, installed, similar_match};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use team_support::{Cli, answers, exits, wait};

/// A selection of the two files and the folder of a temporary directory.
struct Selection {
    _dir: tempfile::TempDir,
    intakes: KnowledgeIntakes,
    token: String,
    files: Vec<String>,
    folder: String,
}

fn selection() -> Selection {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("handbook");
    std::fs::create_dir(&folder).unwrap();

    let mut paths = Vec::new();

    for name in ["guide.md", "api.md"] {
        let path = dir.path().join(name);
        std::fs::write(&path, format!("# {name}")).unwrap();
        paths.push(path);
    }

    paths.push(folder.clone());

    let intakes = KnowledgeIntakes::new();
    let intake = intakes.put(&paths);
    let files = intake.files.iter().map(|entry| entry.path.clone()).collect();

    Selection {
        token: intake.token,
        files,
        folder: folder.to_string_lossy().into_owned(),
        intakes,
        _dir: dir,
    }
}

fn link(repo: &str) -> KnowledgeLink {
    KnowledgeLink {
        repo: repo.to_owned(),
        globs: Vec::new(),
    }
}

fn run(
    cli: &Cli,
    selection: &Selection,
    token: &str,
    links: &[KnowledgeLink],
) -> Result<harness_business::model::KnowledgeSimilar, BusinessError> {
    let (_dir, env) = installed();

    wait(similar_docs(
        &env,
        cli,
        &KnowledgeFeatures::new(),
        &selection.intakes,
        token,
        links,
    ))
}

fn similar_cli(features: &str, answer: Value) -> Cli {
    cli(features).starting(&["docs", "similar"], answers(answer))
}

#[test]
fn the_selections_own_paths_are_compared_in_the_scope_of_the_first_link() {
    let selection = selection();
    let answer = json!({ "similar": [
        { "path": selection.files[0], "match": similar_match(json!(7), "Guide", "sourceKey") },
        { "path": selection.files[1], "match": null }
    ] });
    let server_cli = similar_cli(ALL_FEATURES, answer);

    let found = run(
        &server_cli,
        &selection,
        &selection.token,
        &[link(REPO), link("git:github.com/acme/other")],
    )
    .unwrap();

    let mut expected = vec!["docs", "similar", "--json", "--repo", REPO];
    let paths: Vec<&str> = selection
        .files
        .iter()
        .map(String::as_str)
        .chain([selection.folder.as_str()])
        .collect();
    expected.extend(&paths);
    assert_eq!(
        server_cli.commands().pop().unwrap(),
        expected,
        "files, then the folder, as staged"
    );

    let first = found.items[0].found.as_ref().expect("a match");
    assert_eq!(first.id, KnowledgeId::Remote(7));
    assert_eq!(
        (first.reason, first.place),
        (KnowledgeMatchReason::SourceKey, KnowledgePlace::Local)
    );
    assert_eq!((first.shared_chunks, first.chunks), (3, 4));
    assert_eq!(first.added_by.as_ref().map(|by| by.name.as_str()), Some("Ada Lovelace"));
    assert_eq!(first.updated_by, None);
    assert_eq!(found.items[1].found, None);
    assert!(!found.offline);

    let global = similar_cli(ALL_FEATURES, json!({ "similar": [] }));
    run(&global, &selection, &selection.token, &[]).unwrap();
    assert_eq!(
        &global.commands().pop().unwrap()[..4],
        ["docs", "similar", "--json", "--global"]
    );
}

#[test]
fn every_reason_an_unreadable_file_and_an_offline_remote_are_read() {
    let selection = selection();
    let reasons = ["sourceKey", "sameContent", "sharedChunks", "title"];
    let mut entries: Vec<Value> = reasons
        .iter()
        .enumerate()
        .map(|(at, reason)| {
            json!({ "path": format!("/docs/{reason}.md"), "match": similar_match(json!(format!("local:{}", at + 1)), "A\u{202e}B\u{0007}", reason) })
        })
        .collect();
    entries.push(
        json!({ "path": "/docs/broken.pdf", "match": null, "error": "no text\u{200b} layer\u{202e}\nsecret=glpat-x" }),
    );
    let server_cli = similar_cli(ALL_FEATURES, json!({ "similar": entries, "offline": true }));

    let found = run(&server_cli, &selection, &selection.token, &[]).unwrap();

    let read: Vec<_> = found
        .items
        .iter()
        .filter_map(|entry| entry.found.as_ref())
        .map(|found| found.reason)
        .collect();
    assert_eq!(
        read,
        [
            KnowledgeMatchReason::SourceKey,
            KnowledgeMatchReason::SameContent,
            KnowledgeMatchReason::SharedChunks,
            KnowledgeMatchReason::Title
        ]
    );
    let first = found.items[0].found.as_ref().unwrap();
    assert_eq!(first.id, KnowledgeId::Local("local:1".to_owned()));
    assert_eq!(first.title, "AB", "control and invisible characters are dropped");

    let broken = &found.items[4];
    assert_eq!(broken.found, None);
    assert!(
        broken
            .error
            .as_deref()
            .is_some_and(|error| error.starts_with("no text layer "))
    );
    assert!(
        !broken
            .error
            .as_deref()
            .unwrap_or_default()
            .contains(['\u{200b}', '\u{202e}', '\n']),
        "the error shows on one line without hidden characters"
    );
    assert!(
        !broken.error.as_deref().unwrap_or_default().contains("glpat-x"),
        "redacted"
    );
    assert!(found.offline);
}

#[test]
fn only_the_selections_paths_are_ever_compared() {
    let selection = selection();
    let server_cli = similar_cli(ALL_FEATURES, json!({ "similar": [] }));

    let error = run(&server_cli, &selection, "intake-forged", &[]).expect_err("no such selection");
    assert!(matches!(error, BusinessError::NotFound(_)));
    assert!(
        server_cli.commands().is_empty(),
        "nothing ran for a token that was never given"
    );

    // The selection is used up as its import starts.
    selection.intakes.forget(&selection.token);
    assert!(matches!(
        run(&server_cli, &selection, &selection.token, &[]),
        Err(BusinessError::NotFound(_))
    ));
    assert!(server_cli.commands().is_empty());
}

#[test]
fn a_server_without_docs_similar_is_never_asked() {
    let selection = selection();
    let before = "docs\nimport-ndjson\nsearch\nheld\nmcp\n";
    let server_cli = similar_cli(before, json!({ "similar": [] }));

    let error = run(&server_cli, &selection, &selection.token, &[]).expect_err("older server");

    assert!(matches!(error, BusinessError::Unsupported(message) if message.contains("finding similar documents")));
    assert_eq!(server_cli.commands(), [["features"]], "only the probe ran");
}

#[test]
fn links_the_import_would_refuse_are_refused_before_anything_runs() {
    let selection = selection();
    let server_cli = similar_cli(ALL_FEATURES, json!({ "similar": [] }));

    let error = run(&server_cli, &selection, &selection.token, &[link("not a key")]).expect_err("bad link");

    assert!(matches!(error, BusinessError::InvalidInput(_)));
    assert!(server_cli.commands().is_empty());
}

#[test]
fn many_paths_go_in_calls_of_at_most_fifty_and_the_answers_are_joined_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let paths: Vec<_> = (0..120)
        .map(|n| {
            let path = dir.path().join(format!("doc-{n:03}.md"));
            std::fs::write(&path, "# x").unwrap();
            path
        })
        .collect();
    let intakes = KnowledgeIntakes::new();
    let intake = intakes.put(&paths);
    let selection = Selection {
        _dir: dir,
        token: intake.token,
        files: intake.files.iter().map(|entry| entry.path.clone()).collect(),
        folder: String::new(),
        intakes,
    };
    let server_cli = cli(ALL_FEATURES).answering(|args| {
        (args.starts_with(&["docs".to_owned(), "similar".to_owned()])).then(|| {
            let entries: Vec<Value> = args[4..]
                .iter()
                .map(|path| json!({ "path": path, "match": null }))
                .collect();

            answers(json!({ "similar": entries, "offline": args.len() == 24 }))
        })
    });

    let found = run(&server_cli, &selection, &selection.token, &[]).unwrap();

    let sizes: Vec<_> = server_cli
        .commands()
        .iter()
        .skip(1)
        .map(|args| args.len() - 4)
        .collect();
    assert_eq!(sizes, [50, 50, 20]);
    let order: Vec<_> = found.items.iter().map(|entry| entry.path.clone()).collect();
    assert_eq!(order, selection.files, "the order of the selection");
    assert!(found.offline, "offline when any call was, the last one here");
}

#[test]
fn an_answer_this_app_cannot_read_is_a_protocol_error() {
    let selection = selection();
    let good = similar_match(json!(7), "Guide", "sourceKey");
    let with = |mutate: &dyn Fn(&mut Value)| {
        let mut found = good.clone();
        mutate(&mut found);

        json!({ "similar": [{ "path": "/docs/a.md", "match": found }] })
    };

    for answer in [
        json!({ "similar": "none" }),
        json!({ "items": [] }),
        json!({ "similar": [{ "match": null }] }),
        with(&|found| found["reason"] = json!("lookalike")),
        with(&|found| found["place"] = json!("moon")),
        with(&|found| found["id"] = json!("local:7:3")),
        with(&|found| found["id"] = json!(0)),
        with(&|found| found["updatedAt"] = json!("x".repeat(41))),
        with(&|found| found["chunks"] = json!(-1)),
        json!({ "similar": [{ "path": "x".repeat(4097), "match": null }] }),
        json!({ "similar": vec![json!({ "path": "/a.md", "match": null }); MAX_SIMILAR_ENTRIES + 1] }),
    ] {
        let server_cli = similar_cli(ALL_FEATURES, answer.clone());

        let error = run(&server_cli, &selection, &selection.token, &[]).expect_err("unreadable");

        assert!(matches!(error, BusinessError::Protocol(_)), "{answer}: {error:?}");
    }

    let not_json = cli(ALL_FEATURES).starting(&["docs", "similar"], exits(0, "no json", ""));
    assert!(matches!(
        run(&not_json, &selection, &selection.token, &[]),
        Err(BusinessError::Protocol(_))
    ));
}

#[test]
fn a_failing_comparison_is_the_servers_line() {
    let selection = selection();
    let failing = cli(ALL_FEATURES).starting(
        &["docs", "similar"],
        exits(1, "", "harness-knowledge: at most 5000 files are compared at once\n"),
    );

    let error = run(&failing, &selection, &selection.token, &[]).expect_err("failed");

    assert!(matches!(error, BusinessError::Process(message) if message.contains("at most 5000")));
}

fn options(replacing: &[(&str, KnowledgeId)], replaces: Option<KnowledgeId>) -> KnowledgeImportOptions {
    KnowledgeImportOptions {
        links: vec![],
        replaces,
        replacing: replacing
            .iter()
            .map(|(path, id)| KnowledgeReplacement {
                path: (*path).to_owned(),
                id: id.clone(),
            })
            .collect(),
        place: None,
    }
}

fn paths() -> Vec<String> {
    vec!["/docs/a.md".to_owned(), "/docs/b.md".to_owned()]
}

#[test]
fn replacing_is_written_as_a_map_of_paths_only_for_a_server_with_docs_similar() {
    let both = options(
        &[
            ("/docs/b.md", KnowledgeId::Local("local:12".to_owned())),
            ("/docs/a.md", 7.into()),
        ],
        None,
    );

    let manifest = ImportManifest::with_features(paths(), vec![], both.clone(), true).unwrap();
    let json: Value = serde_json::from_slice(&manifest.to_json().unwrap()).unwrap();
    assert_eq!(json["replacing"], json!({ "/docs/a.md": 7, "/docs/b.md": "local:12" }));
    assert!(json.get("replaces").is_none());

    // An older server would import the files as new documents: refused, never written.
    for old in [
        ImportManifest::with_features(paths(), vec![], both.clone(), false),
        ImportManifest::new(paths(), vec![], both),
    ] {
        assert!(matches!(old, Err(BusinessError::Unsupported(_))), "{old:?}");
    }

    let plain = ImportManifest::with_features(paths(), vec![], options(&[], None), true).unwrap();
    let json: Value = serde_json::from_slice(&plain.to_json().unwrap()).unwrap();
    assert!(json.get("replacing").is_none(), "left out when nothing is replaced");
}

#[test]
fn replacing_a_document_of_the_other_place_is_accepted_as_the_user_chose() {
    use harness_business::model::KnowledgePlace;

    // The import goes to this computer, the replaced document is the team's (id 7) and the other is this computer's.
    let mut cross = options(
        &[
            ("/docs/a.md", 7.into()),
            ("/docs/b.md", KnowledgeId::Local("local:12".to_owned())),
        ],
        None,
    );
    cross.place = Some(KnowledgePlace::Local);

    let manifest = ImportManifest::with_features(paths(), vec![], cross, true).unwrap();
    let json: Value = serde_json::from_slice(&manifest.to_json().unwrap()).unwrap();

    assert_eq!(json["place"], "local");
    assert_eq!(json["replacing"], json!({ "/docs/a.md": 7, "/docs/b.md": "local:12" }));
}

#[test]
fn replacing_names_files_of_the_import_and_documents_once() {
    let refused = |options: KnowledgeImportOptions, paths: Vec<String>, folders: Vec<String>| {
        let error = ImportManifest::with_features(paths, folders, options, true).expect_err("refused");

        assert!(matches!(error, BusinessError::InvalidInput(_)), "{error:?}");
    };

    // Not combined with `replaces`, which needs exactly one file.
    refused(
        options(&[("/docs/a.md", 7.into())], Some(9.into())),
        vec!["/docs/a.md".to_owned()],
        vec![],
    );
    // A file that is not one of the import's paths (a folder's file, or an arbitrary one).
    refused(options(&[("/etc/passwd", 7.into())], None), paths(), vec![]);
    refused(
        options(&[("/docs/handbook/c.md", 7.into())], None),
        paths(),
        vec!["/docs/handbook".to_owned()],
    );
    // Ids that are no document's.
    for id in [
        KnowledgeId::Remote(0),
        KnowledgeId::Remote(-3),
        KnowledgeId::Local("local:x".to_owned()),
        KnowledgeId::Local("local:12:340".to_owned()),
    ] {
        refused(options(&[("/docs/a.md", id)], None), paths(), vec![]);
    }
    // A document or a file twice.
    refused(
        options(&[("/docs/a.md", 7.into()), ("/docs/b.md", 7.into())], None),
        paths(),
        vec![],
    );
    refused(
        options(&[("/docs/a.md", 7.into()), ("/docs/a.md", 8.into())], None),
        paths(),
        vec![],
    );
}
