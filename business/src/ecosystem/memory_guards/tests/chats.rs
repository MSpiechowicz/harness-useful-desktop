//! The chat files: their project root, the newest-revision writes and the startup sweep.

use std::time::{Duration, SystemTime};

use pretty_assertions::assert_eq;

use super::*;
use crate::model::MemoryChatGuardsSet;

#[test]
fn a_chat_file_names_the_root_of_its_project() {
    let (dir, data, repo) = setup();
    let root = repo.to_string_lossy().into_owned();

    assert_eq!(chat_repo_root(&repo.join("src")).as_deref(), Some(root.as_str()));

    let loose = dunce::canonicalize(dir.path()).unwrap().join("loose");
    std::fs::create_dir(&loose).unwrap();
    assert_eq!(
        chat_repo_root(&loose).as_deref(),
        Some(loose.to_string_lossy().as_ref()),
        "a folder outside a repository names itself"
    );
    assert_eq!(chat_repo_root(&dir.path().join("gone")), None);

    write_chat(&data, KEY, &chat(false, true, true), chat_repo_root(&repo).as_deref()).unwrap();

    let text = std::fs::read_to_string(chat_path(&data, KEY)).unwrap();
    let file: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(file["repoRoot"], root.as_str());
    assert_eq!(read_chat_repo(&data, KEY).unwrap().as_deref(), Some(root.as_str()));
}

fn applied(revision: u64) -> MemoryChatGuardsSet {
    MemoryChatGuardsSet {
        applied: true,
        revision,
    }
}

fn late(revision: u64) -> MemoryChatGuardsSet {
    MemoryChatGuardsSet {
        applied: false,
        revision,
    }
}

/// The data folder of [`setup`] with a chat file that names its repository root, the root, and
/// the writes to the chat.
fn written_chat() -> (tempfile::TempDir, PathBuf, String, ChatGuardWrites) {
    let (dir, data, repo) = setup();
    let root = repo.to_string_lossy().into_owned();
    write_chat(&data, KEY, &chat(false, true, true), Some(&root)).unwrap();

    (dir, data, root, ChatGuardWrites::default())
}

#[test]
fn a_late_chat_write_never_undoes_a_newer_one() {
    let (_dir, data, root, writes) = written_chat();

    let set = |guards: MemoryChatGuards, revision: u64| writes.set_guards(&data, KEY, &guards, revision, None).unwrap();

    assert_eq!(set(chat(true, true, true), 2), applied(2));
    assert_eq!(
        set(chat(false, false, false), 1),
        late(2),
        "older, named with the last revision"
    );
    assert_eq!(set(chat(false, false, false), 2), late(2), "the same");
    assert_eq!(read_chat(&data, KEY).unwrap(), Some(chat(true, true, true)));

    assert_eq!(set(chat(false, false, true), 3), applied(3));
    assert_eq!(read_chat(&data, KEY).unwrap(), Some(chat(false, false, true)));
    assert_eq!(
        read_chat_repo(&data, KEY).unwrap().as_deref(),
        Some(root.as_str()),
        "a toggle keeps the repository"
    );

    const OTHER: &str = "1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d";
    assert_eq!(
        writes
            .set_guards(&data, OTHER, &chat(true, true, true), 0, None)
            .unwrap(),
        applied(0),
        "each chat has its own revisions"
    );
}

#[test]
fn a_toggle_over_an_unreadable_chat_file_keeps_the_project_root() {
    let (_dir, data, root, writes) = written_chat();
    std::fs::write(chat_path(&data, KEY), "{ not json").unwrap();
    assert!(read_chat(&data, KEY).is_err(), "the file cannot be read");

    let set = writes
        .set_guards(&data, KEY, &chat(true, false, true), 1, Some(&root))
        .unwrap();

    assert_eq!(set, applied(1));
    assert_eq!(read_chat(&data, KEY).unwrap(), Some(chat(true, false, true)));
    assert_eq!(
        read_chat_repo(&data, KEY).unwrap().as_deref(),
        Some(root.as_str()),
        "rebuilt from the chat's project"
    );

    const ELSEWHERE: &str = "/somewhere/else";
    writes
        .set_guards(&data, KEY, &chat(false, false, true), 2, Some(ELSEWHERE))
        .unwrap();
    assert_eq!(
        read_chat_repo(&data, KEY).unwrap().as_deref(),
        Some(root.as_str()),
        "a readable file keeps the root it names"
    );
}

#[test]
fn a_moved_chat_names_its_new_project_root() {
    let (dir, data, repo) = setup();
    let writes = ChatGuardWrites::default();
    let loose = dunce::canonicalize(dir.path()).unwrap().join("loose");
    std::fs::create_dir(&loose).unwrap();

    writes.set_project(&data, KEY, &repo).unwrap();
    assert_eq!(
        read_chat(&data, KEY).unwrap(),
        None,
        "no file is created for a chat without one"
    );

    write_chat(&data, KEY, &chat(true, true, false), None).unwrap();
    writes.set_project(&data, KEY, &repo).unwrap();
    assert_eq!(
        read_chat_repo(&data, KEY).unwrap(),
        Some(repo.to_string_lossy().into_owned())
    );
    assert_eq!(read_chat(&data, KEY).unwrap(), Some(chat(true, true, false)));

    writes.set_project(&data, KEY, &loose).unwrap();
    assert_eq!(
        read_chat_repo(&data, KEY).unwrap(),
        Some(loose.to_string_lossy().into_owned()),
        "a folder outside a repository names itself"
    );

    writes.set_project(&data, KEY, &dir.path().join("gone")).unwrap();
    assert_eq!(
        read_chat_repo(&data, KEY).unwrap(),
        None,
        "a folder that is gone names none"
    );
}

#[test]
fn the_sweep_removes_only_old_chat_files() {
    let (_dir, data, _repo) = setup();
    const FRESH: &str = "1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d";

    write_chat(&data, KEY, &MemoryChatGuards::default(), None).unwrap();
    write_chat(&data, FRESH, &MemoryChatGuards::default(), None).unwrap();

    let chats = data.join(GUARDS_DIR).join(CHATS_DIR);
    let stray = chats.join("notes.txt");
    std::fs::write(&stray, "kept").unwrap();

    let long_ago = SystemTime::now() - CHAT_MAX_AGE - Duration::from_secs(60);
    for path in [chat_path(&data, KEY), stray.clone()] {
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(long_ago)
            .unwrap();
    }

    assert_eq!(sweep_chats(&data, CHAT_MAX_AGE), 1);
    assert!(!chat_path(&data, KEY).exists(), "the old chat file");
    assert!(chat_path(&data, FRESH).exists(), "a recent chat file");
    assert!(stray.exists(), "a file that is not a chat file");
    assert_eq!(sweep_chats(&data.join("missing"), CHAT_MAX_AGE), 0);
}

#[cfg(unix)]
#[test]
fn the_sweep_never_follows_a_symbolic_link() {
    let (dir, data, _repo) = setup();
    write_chat(&data, KEY, &MemoryChatGuards::default(), None).unwrap();

    let elsewhere = dir.path().join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    let target = elsewhere.join(format!("{KEY}.json"));
    std::fs::write(&target, "{}").unwrap();
    let long_ago = SystemTime::now() - CHAT_MAX_AGE - Duration::from_secs(60);
    std::fs::File::options()
        .write(true)
        .open(&target)
        .unwrap()
        .set_modified(long_ago)
        .unwrap();

    const LINKED: &str = "1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d";
    let chats = data.join(GUARDS_DIR).join(CHATS_DIR);
    std::os::unix::fs::symlink(&target, chats.join(format!("{LINKED}.json"))).unwrap();
    assert_eq!(sweep_chats(&data, Duration::ZERO), 1, "only the chat file itself");
    assert!(target.exists(), "nothing removed through a file link");

    std::fs::remove_dir_all(&chats).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &chats).unwrap();
    assert_eq!(sweep_chats(&data, Duration::ZERO), 0);
    assert!(target.exists(), "nothing removed through a folder link");
}
