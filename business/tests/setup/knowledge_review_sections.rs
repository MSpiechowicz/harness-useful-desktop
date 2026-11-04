//! A compare numbers the sections that share a heading and names the revision it reviews, so a
//! line comment can say which section it is on.

use crate::knowledge_stand_in;
use crate::team_support;

use harness_business::ecosystem::compare_doc;
use harness_business::model::KnowledgeOutcome;
use knowledge_stand_in::review::{ID, doc_id, docs_answering};
use knowledge_stand_in::{ALL_FEATURES, installed, server};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use team_support::{answers, wait};

fn section(path: &str, occurrence: Option<u32>) -> Value {
    let mut section = json!({
        "path": path, "oldPath": null, "status": "modified", "staged": false, "isBinary": false,
        "truncated": false, "additions": 1, "deletions": 1, "hunks": []
    });

    if let Some(occurrence) = occurrence {
        section["occurrence"] = json!(occurrence);
    }

    section
}

fn compared(reply: Value) -> harness_business::model::KnowledgeCompare {
    let (_dir, env) = installed();
    let server_cli = docs_answering(ALL_FEATURES, &["docs", "compare"], answers(reply));

    match wait(compare_doc(&env, &server_cli, &server(&env), &doc_id(), None)).unwrap() {
        KnowledgeOutcome::Done { value } => value,
        KnowledgeOutcome::Refused { code, message } => panic!("refused {code:?}: {message}"),
    }
}

#[test]
fn a_section_keeps_its_occurrence_and_the_first_is_assumed_when_the_server_does_not_say() {
    let compare = compared(json!({
        "live": 5, "pending": ID, "revision": 7,
        "sections": [section("Install", Some(1)), section("Install", Some(2)), section("Install#2", None), section("Odd", Some(0))]
    }));

    let found: Vec<(&str, u32)> = compare
        .sections
        .iter()
        .map(|section| (section.diff.path.as_str(), section.occurrence))
        .collect();

    assert_eq!(found, [("Install", 1), ("Install", 2), ("Install#2", 1), ("Odd", 1)]);
}

#[test]
fn a_compare_carries_the_revision_of_the_row_that_is_reviewed_when_the_server_prints_it() {
    let printed = compared(json!({ "live": null, "pending": ID, "revision": 7, "sections": [] }));
    assert_eq!(printed.revision, Some(7));

    let older = compared(json!({ "live": null, "pending": ID, "sections": [] }));
    assert_eq!(older.revision, None);

    let negative = compared(json!({ "live": null, "pending": ID, "revision": -1, "sections": [] }));
    assert_eq!(negative.revision, None, "a revision is never negative");
}
