//! Importing drafts: the manifest's `draft`, `ask` and `note`, and the events that say a file was
//! stored as a draft or as a pending version.

use harness_business::BusinessError;
use harness_business::ecosystem::ImportManifest;
use harness_business::model::{
    KnowledgeFileStatus, KnowledgeId, KnowledgeImportEvent, KnowledgeImportOptions, KnowledgeReplacement,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn import_options(replaces: Option<KnowledgeId>) -> KnowledgeImportOptions {
    KnowledgeImportOptions {
        replaces,
        ..Default::default()
    }
}

fn manifest_json(options: KnowledgeImportOptions, drafts: bool) -> Result<Value, BusinessError> {
    let features = harness_business::model::KnowledgeFeatures {
        docs_drafts: drafts,
        ..Default::default()
    };
    let manifest = ImportManifest::with_features(vec!["/docs/a.md".to_owned()], vec![], options, features)?;

    Ok(serde_json::from_slice(&manifest.to_json().unwrap()).unwrap())
}

#[test]
fn a_draft_and_an_ask_with_a_note_are_written_into_the_manifest_only_for_a_server_with_drafts() {
    let draft = KnowledgeImportOptions {
        draft: true,
        ..Default::default()
    };
    let json = manifest_json(draft.clone(), true).unwrap();
    assert_eq!(json["draft"], true);
    assert!(json.get("ask").is_none() && json.get("note").is_none());

    let ask = KnowledgeImportOptions {
        ask: true,
        note: Some("  New retry rules ".to_owned()),
        ..import_options(Some(7.into()))
    };
    let json = manifest_json(ask.clone(), true).unwrap();
    assert_eq!(
        (json["ask"].clone(), json["note"].clone(), json["replaces"].clone()),
        (json!(true), json!("New retry rules"), json!(7))
    );
    assert!(json.get("draft").is_none(), "left out unless asked");

    // An older server would make the document live: refused, never written.
    for options in [draft, ask] {
        assert!(matches!(
            manifest_json(options, false),
            Err(BusinessError::Unsupported(_))
        ));
    }

    let plain = manifest_json(import_options(None), false).unwrap();
    assert!(plain.get("draft").is_none() && plain.get("ask").is_none());
}

#[test]
fn an_ask_needs_a_replaced_document_and_a_note_needs_an_ask() {
    let replacing = vec![KnowledgeReplacement {
        path: "/docs/a.md".to_owned(),
        id: 7.into(),
    }];

    for options in [
        KnowledgeImportOptions {
            ask: true,
            ..Default::default()
        },
        KnowledgeImportOptions {
            note: Some("why".to_owned()),
            ..import_options(Some(7.into()))
        },
        KnowledgeImportOptions {
            ask: true,
            note: Some("  ".to_owned()),
            ..import_options(Some(7.into()))
        },
        KnowledgeImportOptions {
            ask: true,
            note: Some("x".repeat(501)),
            ..import_options(Some(7.into()))
        },
    ] {
        assert!(matches!(
            manifest_json(options, true),
            Err(BusinessError::InvalidInput(_))
        ));
    }

    let features = harness_business::model::KnowledgeFeatures {
        docs_drafts: true,
        docs_similar: true,
        ..Default::default()
    };
    let options = KnowledgeImportOptions {
        ask: true,
        replacing,
        ..Default::default()
    };
    let manifest = ImportManifest::with_features(vec!["/docs/a.md".to_owned()], vec![], options, features).unwrap();
    let json: Value = serde_json::from_slice(&manifest.to_json().unwrap()).unwrap();

    assert_eq!(
        (json["ask"].clone(), json["replacing"]["/docs/a.md"].clone()),
        (json!(true), json!(7))
    );
}

#[test]
fn a_pending_file_and_a_draft_flag_are_read_from_the_import_events() {
    let file = harness_business::ecosystem::parse_import_event(
        r#"{"type":"file","path":"/docs/a.md","status":"pending","docId":21,"chunks":3,"embedding":"done","draft":true}"#,
    );

    assert!(matches!(
        file,
        Some(KnowledgeImportEvent::File {
            status: KnowledgeFileStatus::Pending,
            draft: true,
            doc_id: Some(_),
            ..
        })
    ));

    let plain = harness_business::ecosystem::parse_import_event(
        r#"{"type":"file","path":"/docs/a.md","status":"added","docId":7}"#,
    );
    assert!(
        matches!(
            plain,
            Some(KnowledgeImportEvent::File {
                draft: false,
                asked: false,
                ..
            })
        ),
        "absent from an older server"
    );

    let asked = harness_business::ecosystem::parse_import_event(
        r#"{"type":"file","path":"/docs/a.md","status":"added","docId":7,"draft":true,"asked":true}"#,
    );
    assert!(matches!(
        asked,
        Some(KnowledgeImportEvent::File {
            draft: true,
            asked: true,
            ..
        })
    ));

    // The field is optional in the event as the page receives it too.
    let stored: KnowledgeImportEvent =
        serde_json::from_str(r#"{"type":"file","path":"a.md","status":"added","reason":null,"docId":7,"chunks":1,"embedding":null,"warnings":[],"draft":true}"#)
            .unwrap();
    assert!(matches!(stored, KnowledgeImportEvent::File { asked: false, .. }));

    let done = harness_business::ecosystem::parse_import_event(
        r#"{"type":"done","counts":{"added":1,"updated":0,"pending":2,"unchanged":0,"skipped":0,"failed":0},"cancelled":false,"counter":"tokenizer","embeddingPending":0}"#,
    );
    assert!(matches!(done, Some(KnowledgeImportEvent::Done { counts, .. }) if counts.pending == 2));
}
