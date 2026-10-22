//! Session tags (see "Session tags" in `docs/routines-format.md`): the routine run behind a
//! harness session, looked up by harness and session id, the newest 2000 kept.

use harness_business::model::{HarnessId, SessionRoutineTag};
use harness_business::routines::SessionTags;
use harness_business::routines::session_tags::MAX_SESSION_TAGS;
use pretty_assertions::assert_eq;

fn tag(run: usize) -> SessionRoutineTag {
    SessionRoutineTag {
        routine_id: "f".repeat(32),
        run_id: format!("{run:032x}"),
        name: "Morning review".to_owned(),
    }
}

#[test]
fn a_session_is_found_by_its_harness_and_id() {
    let temp = tempfile::tempdir().unwrap();
    let tags = SessionTags::new(temp.path());

    assert_eq!(tags.lookup(HarnessId::Omp, "abc"), None);

    tags.record(HarnessId::Omp, "abc", &tag(1)).unwrap();

    assert_eq!(tags.lookup(HarnessId::Omp, "abc"), Some(tag(1)));
    assert_eq!(tags.lookup(HarnessId::Pi, "abc"), None, "another harness's session");

    let text = std::fs::read_to_string(temp.path().join("routine-sessions.json")).unwrap();
    assert!(text.contains("\"omp:abc\""), "{text}");
    assert!(text.contains("\"routineId\""), "{text}");
}

#[test]
fn the_newest_entries_are_kept() {
    let temp = tempfile::tempdir().unwrap();
    let tags = SessionTags::new(temp.path());

    // A full file, oldest first, then three more.
    let sessions: serde_json::Map<String, serde_json::Value> = (0..MAX_SESSION_TAGS)
        .map(|run| (format!("pi:s{run}"), serde_json::to_value(tag(run)).unwrap()))
        .collect();
    let full = serde_json::json!({"version": 1, "sessions": sessions});
    std::fs::write(temp.path().join("routine-sessions.json"), full.to_string()).unwrap();

    for run in MAX_SESSION_TAGS..MAX_SESSION_TAGS + 3 {
        tags.record(HarnessId::Pi, &format!("s{run}"), &tag(run)).unwrap();
    }

    let index = tags.index();
    assert_eq!(index.get(HarnessId::Pi, "s0"), None);
    assert_eq!(index.get(HarnessId::Pi, "s2"), None);
    assert_eq!(index.get(HarnessId::Pi, "s3"), Some(&tag(3)));

    // Tagging an old entry again makes it the newest.
    tags.record(HarnessId::Pi, "s3", &tag(9)).unwrap();
    tags.record(HarnessId::Pi, "new", &tag(10)).unwrap();

    let index = tags.index();
    assert_eq!(index.get(HarnessId::Pi, "s3"), Some(&tag(9)));
    assert_eq!(index.get(HarnessId::Pi, "s4"), None);
}

#[test]
fn an_invalid_file_is_set_aside_by_the_next_write() {
    let temp = tempfile::tempdir().unwrap();
    let tags = SessionTags::new(temp.path());
    std::fs::write(temp.path().join("routine-sessions.json"), "{\"version\": 2}").unwrap();

    assert!(tags.index().is_empty());

    tags.record(HarnessId::Omp, "x", &tag(1)).unwrap();
    assert_eq!(tags.lookup(HarnessId::Omp, "x"), Some(tag(1)));
}
