//! Saved-session copies and deletion: the `emptyCopy` flag of `sessions_list` and
//! `Harness::delete_session` over the OMP fixtures (`tests/fixtures/sessions/omp`), and the files
//! deletion refuses (no session header, a name the harnesses do not write, an id carried by
//! several files).

mod common;
mod session_support;

use std::fs;
use std::path::{Path, PathBuf};

use harness_business::harness::Harness;
use harness_business::harnesses::{Omp, Pi};
use harness_business::model::{HarnessId, IpcErrorCode, SessionSummary};
use harness_business::pi_family::sessions::{SUMMARY_BYTES, SessionCache};
use pretty_assertions::assert_eq;
use session_support::{OMP_FORK, OMP_MAIN, SessionHome, session, session_home};

/// The fixture copy of [`OMP_MAIN`]: copied entries and two `session_exit` records only.
const OMP_COPY: &str = "0199c2b0-4e1a-7d2b-9c3d-2f5e8a7b6c41";

/// Header of a copy of `parent` made at 2026-10-09T10:30:00Z.
fn copy_header(id: &str, parent: &str) -> String {
    format!(
        r#"{{"type":"session","version":3,"id":"{id}","timestamp":"2026-10-09T10:30:00.000Z","cwd":"/x","parentSession":"{parent}"}}"#
    )
}

/// A `message` entry of `role` at `time`.
fn message(id: &str, role: &str, time: &str) -> String {
    format!(
        r#"{{"type":"message","id":"{id}","parentId":null,"timestamp":"{time}","message":{{"role":"{role}","content":"hello"}}}}"#
    )
}

/// A `session_exit` record at `time`.
fn exit(id: &str, time: &str) -> String {
    format!(r#"{{"type":"custom","customType":"session_exit","id":"{id}","parentId":null,"timestamp":"{time}"}}"#)
}

fn omp_home() -> (SessionHome, PathBuf) {
    let home = SessionHome::with_fixtures(&[(&Omp, "omp")]);
    let dir = Omp.sessions_dir(&home.env, &home.project);
    (home, dir)
}

/// Writes the session `id` into `dir` as `lines`.
fn write_session(dir: &Path, id: &str, lines: &[String]) -> PathBuf {
    let path = dir.join(format!("2026-10-09T10-30-00-000Z_{id}.jsonl"));
    fs::write(&path, lines.join("\n") + "\n").unwrap();
    path
}

fn file_of(dir: &Path, id: &str) -> PathBuf {
    dir.join(match id {
        OMP_MAIN => format!("2026-10-09T10-00-00-000Z_{id}.jsonl"),
        OMP_FORK => format!("2026-10-09T11-02-41-000Z_{id}.jsonl"),
        _ => format!("2026-10-09T10-30-00-000Z_{id}.jsonl"),
    })
}

fn listed(home: &SessionHome, cache: &SessionCache, id: &str) -> Option<SessionSummary> {
    let list = Omp.list_sessions(&home.env, &home.project, cache).unwrap();
    list.into_iter().find(|summary| summary.session.id == id)
}

fn empty_copy(home: &SessionHome, id: &str) -> bool {
    listed(home, &SessionCache::default(), id).unwrap().empty_copy
}

fn delete(home: &SessionHome, id: &str, empty_copy_only: bool) -> harness_business::Result<()> {
    Omp.delete_session(&home.env, &home.project, &session(HarnessId::Omp, id), empty_copy_only)
}

fn error_code(result: harness_business::Result<()>) -> IpcErrorCode {
    result.expect_err("expected an error").code()
}

#[test]
fn copies_that_add_nothing_to_a_listed_parent_are_empty_copies() {
    let (home, dir) = omp_home();
    let list = Omp
        .list_sessions(&home.env, &home.project, &SessionCache::default())
        .unwrap();

    let flagged: Vec<&str> = list
        .iter()
        .filter(|summary| summary.empty_copy)
        .map(|summary| summary.session.id.as_str())
        .collect();
    assert_eq!(flagged, [OMP_COPY], "the parent and real forks are not copies");

    let late = "0199c2b0-0000-7000-8000-00000000000a";
    let lines = [
        copy_header(late, OMP_MAIN),
        message("m1", "user", "2026-10-09T10:03:00.000Z"),
        exit("x1", "2026-10-09T10:30:00.000Z"),
        message("m2", "user", "2026-10-09T10:31:00.000Z"),
    ];
    write_session(&dir, late, &lines);
    assert!(!empty_copy(&home, late), "a prompt after the copy is new content");

    let untimed = "0199c2b0-0000-7000-8000-00000000000b";
    let lines = [
        copy_header(untimed, OMP_MAIN),
        message("m1", "user", "2026-10-09T10:03:00.000Z"),
        r#"{"type":"custom","id":"c1","parentId":null}"#.to_owned(),
        exit("x1", "2026-10-09T10:30:00.000Z"),
    ];
    write_session(&dir, untimed, &lines);
    assert!(!empty_copy(&home, untimed), "an entry without a time counts as content");

    let unexited = "0199c2b0-0000-7000-8000-00000000000c";
    let lines = [
        copy_header(unexited, OMP_MAIN),
        message("m1", "user", "2026-10-09T10:03:00.000Z"),
    ];
    write_session(&dir, unexited, &lines);
    assert!(!empty_copy(&home, unexited), "a copy is only known by its exit records");

    let orphan = "0199c2b0-0000-7000-8000-00000000000d";
    let lines = [
        copy_header(orphan, "0199eeee-0000-7000-8000-00000000000e"),
        message("m1", "user", "2026-10-09T10:03:00.000Z"),
        exit("x1", "2026-10-09T10:30:00.000Z"),
    ];
    write_session(&dir, orphan, &lines);
    assert!(!empty_copy(&home, orphan), "the parent is not in the folder");
}

#[test]
fn large_copies_are_judged_by_their_tail() {
    let (home, dir) = omp_home();
    let id = "0199c2b0-0000-7000-8000-00000000000f";
    let padding = "x".repeat(4096);

    let mut lines = vec![
        copy_header(id, OMP_MAIN),
        message("m0", "user", "2026-10-09T10:03:00.000Z"),
    ];
    let entries = usize::try_from(SUMMARY_BYTES).unwrap() / padding.len() + 16;
    for index in 0..entries {
        lines.push(format!(
            r#"{{"type":"custom","customType":"note","id":"e{index}","parentId":null,"timestamp":"2026-10-09T10:05:00.000Z","data":"{padding}"}}"#
        ));
    }
    lines.push(exit("x1", "2026-10-09T10:30:00.000Z"));

    let path = write_session(&dir, id, &lines);
    assert!(fs::metadata(&path).unwrap().len() > SUMMARY_BYTES);
    assert!(empty_copy(&home, id));

    lines.push(message("m1", "user", "2026-10-09T10:31:00.000Z"));
    write_session(&dir, id, &lines);
    assert!(!empty_copy(&home, id), "a prompt at the end of a large copy");
}

#[test]
fn cached_listings_still_check_that_the_parent_exists() {
    let (home, _dir) = omp_home();
    let cache = SessionCache::default();

    assert!(listed(&home, &cache, OMP_COPY).unwrap().empty_copy);

    delete(&home, OMP_MAIN, false).unwrap();

    let copy = listed(&home, &cache, OMP_COPY).unwrap();
    assert!(!copy.empty_copy, "the copy now holds the only transcript");
    assert_eq!(copy.parent_session.as_deref(), Some(OMP_MAIN));
}

#[test]
fn deleting_removes_the_file_its_companion_folder_and_backups() {
    let (home, dir) = omp_home();
    let main = file_of(&dir, OMP_MAIN);
    let fork = file_of(&dir, OMP_FORK);
    let companion = dir.join(main.file_stem().unwrap());
    let backup = PathBuf::from(format!("{}.7311.bak", main.display()));
    let fork_backup = PathBuf::from(format!("{}.7312.bak", fork.display()));
    fs::write(&backup, "stale").unwrap();
    fs::write(&fork_backup, "stale").unwrap();
    assert!(companion.is_dir());

    delete(&home, OMP_MAIN, false).unwrap();

    assert!(!main.exists() && !companion.exists() && !backup.exists());
    assert!(fork.exists() && fork_backup.exists(), "other sessions are untouched");
    assert!(listed(&home, &SessionCache::default(), OMP_MAIN).is_none());
    assert_eq!(error_code(delete(&home, OMP_MAIN, false)), IpcErrorCode::NotFound);
}

#[cfg(unix)]
#[test]
fn deleting_never_follows_symlinks() {
    let (home, dir) = omp_home();
    let fork = file_of(&dir, OMP_FORK);
    let outside = home.home.join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("keep.txt"), "keep").unwrap();

    let linked_companion = dir.join(fork.file_stem().unwrap());
    let linked_backup = PathBuf::from(format!("{}.1.bak", fork.display()));
    std::os::unix::fs::symlink(&outside, &linked_companion).unwrap();
    std::os::unix::fs::symlink(outside.join("keep.txt"), &linked_backup).unwrap();

    delete(&home, OMP_FORK, false).unwrap();

    assert!(!fork.exists());
    assert!(outside.join("keep.txt").exists(), "the linked folder was entered");
    assert!(fs::symlink_metadata(&linked_companion).is_ok() && fs::symlink_metadata(&linked_backup).is_ok());
}

#[test]
fn deleting_rejects_unknown_unsafe_and_foreign_sessions() {
    let (home, dir) = omp_home();
    let other = home.home.join("Projects/other");
    fs::create_dir_all(&other).unwrap();

    assert_eq!(
        error_code(delete(&home, "0199eeee-0000-7000-8000-000000000003", false)),
        IpcErrorCode::NotFound,
        "a subagent transcript in a companion folder is not a session"
    );
    assert_eq!(error_code(delete(&home, "../x", false)), IpcErrorCode::InvalidInput);

    let pi_ref = session(HarnessId::Pi, OMP_MAIN);
    assert_eq!(
        error_code(Pi.delete_session(&home.env, &home.project, &pi_ref, false)),
        IpcErrorCode::NotFound,
        "PI has no such session"
    );
    assert_eq!(
        error_code(Omp.delete_session(&home.env, &home.project, &pi_ref, false)),
        IpcErrorCode::InvalidInput
    );

    let main = session(HarnessId::Omp, OMP_MAIN);
    assert_eq!(
        error_code(Omp.delete_session(&home.env, &other, &main, false)),
        IpcErrorCode::NotFound
    );
    assert!(file_of(&dir, OMP_MAIN).exists());
}

#[test]
fn empty_copy_only_deletes_nothing_but_empty_copies() {
    let (home, dir) = omp_home();

    assert_eq!(error_code(delete(&home, OMP_MAIN, true)), IpcErrorCode::InvalidInput);
    assert_eq!(error_code(delete(&home, OMP_FORK, true)), IpcErrorCode::InvalidInput);
    assert!(file_of(&dir, OMP_MAIN).exists() && file_of(&dir, OMP_FORK).exists());

    delete(&home, OMP_COPY, true).unwrap();
    assert!(!file_of(&dir, OMP_COPY).exists());
    assert!(file_of(&dir, OMP_MAIN).exists(), "the parent stays");
}

#[test]
fn deleting_refuses_files_without_the_sessions_header() {
    let home = session_home();
    let elsewhere = home.home.join("elsewhere");
    fs::create_dir_all(home.project.join(".pi")).unwrap();
    let companion = elsewhere.join("2026-10-09T10-00-00-000Z_b");
    fs::create_dir_all(&companion).unwrap();
    let settings = serde_json::json!({ "sessionDir": elsewhere });
    fs::write(home.project.join(".pi/settings.json"), settings.to_string()).unwrap();
    assert_eq!(Pi.sessions_dir(&home.env, &home.project), elsewhere);

    let file = elsewhere.join("2026-10-09T10-00-00-000Z_b.jsonl");
    let delete = || Pi.delete_session(&home.env, &home.project, &session(HarnessId::Pi, "b"), false);
    let headers = [
        r#"{"name":"not a session"}"#,
        r#"{"type":"session","version":3,"id":"other","cwd":"/x"}"#,
        r#"{"type":"message","id":"m1"}"#,
    ];

    for header in headers {
        fs::write(&file, format!("{header}\n")).unwrap();
        assert_eq!(error_code(delete()), IpcErrorCode::InvalidInput, "{header}");
        assert!(file.is_file() && companion.is_dir(), "{header}");
    }

    fs::write(&file, r#"{"type":"session","version":3,"id":"b","cwd":"/x"}"#).unwrap();
    delete().unwrap();
    assert!(!file.exists() && !companion.exists());
}

#[test]
fn deleting_refuses_files_the_harnesses_do_not_name() {
    let (home, dir) = omp_home();
    let header = r#"{"type":"session","version":3,"id":"proj","cwd":"/x"}"#;

    for stem in [
        "important_proj",
        "2026-10-09T10-00-00Z_proj",
        "2026-10-9T10-00-00-000Z_proj",
    ] {
        let file = dir.join(format!("{stem}.jsonl"));
        let companion = dir.join(stem);
        fs::write(&file, format!("{header}\n")).unwrap();
        fs::create_dir_all(&companion).unwrap();

        assert_eq!(
            error_code(delete(&home, "proj", false)),
            IpcErrorCode::InvalidInput,
            "{stem}"
        );
        assert!(file.is_file() && companion.is_dir(), "{stem}");
        fs::remove_file(&file).unwrap();
    }
}

#[test]
fn deleting_refuses_an_id_carried_by_several_files() {
    let (home, dir) = omp_home();
    let id = "0199c2b0-0000-7000-8000-000000000010";
    let lines = [
        format!(r#"{{"type":"session","version":3,"id":"{id}","timestamp":"2026-10-09T10:30:00.000Z","cwd":"/x"}}"#),
        message("m1", "user", "2026-10-09T10:31:00.000Z"),
    ];
    let first = write_session(&dir, id, &lines);
    let second = dir.join(format!("2026-10-10T08-00-00-000Z_{id}.jsonl"));
    fs::copy(&first, &second).unwrap();

    assert_eq!(error_code(delete(&home, id, false)), IpcErrorCode::InvalidInput);
    assert!(first.is_file() && second.is_file());

    let reading = Omp.session_messages(&home.env, &home.project, &session(HarnessId::Omp, id), None, 10);
    assert_eq!(reading.unwrap().total, 1, "reading still picks one of them");

    fs::remove_file(&second).unwrap();
    delete(&home, id, false).unwrap();
    assert!(!first.exists());
}
