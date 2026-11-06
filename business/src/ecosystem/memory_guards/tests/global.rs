//! `guards/global.json`: the opt-in version 2, the move from version 1 and the `everyProject` patch.

use pretty_assertions::assert_eq;

use super::*;

const V1: &str = r#"{
  "version": 1,
  "notes": { "paused": true, "offRepos": ["/work/a", "/work/b"] },
  "scripts": { "paused": false, "offRepos": ["/work/c"] }
}
"#;

fn put(data: &Path, text: &str) {
    std::fs::create_dir_all(data.join(GUARDS_DIR)).unwrap();
    std::fs::write(global_path(data), text).unwrap();
}

fn text(data: &Path) -> String {
    std::fs::read_to_string(global_path(data)).unwrap()
}

fn version_2_off() -> serde_json::Value {
    serde_json::json!({
        "version": 2,
        "notes": { "paused": false, "everyProject": false, "onRepos": [], "repoKeys": {} },
        "scripts": { "paused": false, "everyProject": false, "onRepos": [], "repoKeys": {} },
    })
}

#[test]
fn a_missing_file_becomes_version_two_with_saving_off() {
    let (_dir, data, _repo) = setup();

    ensure_global(&data).unwrap();

    let file: serde_json::Value = serde_json::from_str(&text(&data)).unwrap();
    assert_eq!(file, version_2_off());
    assert_eq!(read_global(&data).unwrap(), MemoryGuards::default());

    #[cfg(unix)]
    assert_eq!(mode(&global_path(&data)), 0o600);
}

#[test]
fn version_one_keeps_its_pauses_and_loses_its_list() {
    let (_dir, data, _repo) = setup();
    put(&data, V1);

    ensure_global(&data).unwrap();

    let file: serde_json::Value = serde_json::from_str(&text(&data)).unwrap();
    assert_eq!(
        file,
        serde_json::json!({
            "version": 2,
            "notes": { "paused": true, "everyProject": false, "onRepos": [], "repoKeys": {} },
            "scripts": { "paused": false, "everyProject": false, "onRepos": [], "repoKeys": {} },
        })
    );
}

#[test]
fn version_two_is_left_byte_for_byte_and_the_move_is_idempotent() {
    let (_dir, data, repo) = setup();
    let current = format!(
        "{{\"version\":2,\"notes\":{{\"paused\":false,\"everyProject\":true,\"onRepos\":[\"{0}\"],\"repoKeys\":{{\"{0}\":null}}}},\
         \"scripts\":{{\"paused\":true,\"everyProject\":false,\"onRepos\":[],\"repoKeys\":{{}}}}}}",
        repo.display().to_string().replace('\\', "\\\\")
    );
    put(&data, &current);

    ensure_global(&data).unwrap();
    assert_eq!(text(&data), current);

    put(&data, V1);
    ensure_global(&data).unwrap();
    let moved = text(&data);
    ensure_global(&data).unwrap();
    assert_eq!(text(&data), moved, "a second run changes nothing");
}

#[test]
fn a_version_two_file_from_before_the_pins_is_written_again_with_no_entries_and_empty_pins() {
    let (_dir, data, repo) = setup();
    let root = repo.display().to_string();
    let before = serde_json::json!({
        "version": 2,
        "notes": { "paused": false, "everyProject": true, "onRepos": [root] },
        "scripts": { "paused": true, "everyProject": false, "onRepos": [], "repoKeys": {} },
    });
    put(&data, &before.to_string());

    // An entry with no pin turns nothing on, so it isn't shown as on. Reading writes nothing.
    let read = read_global(&data).unwrap();
    assert!(read.notes.on_repos.is_empty());
    assert!(read.notes.dead_repos.is_empty());
    assert!(read.notes.repo_keys.is_empty());
    assert!(read.notes.every_project && read.scripts.paused);
    assert_eq!(text(&data), before.to_string());

    // The start writes it again: the other settings kept, no entries, the pins empty.
    ensure_global(&data).unwrap();
    let after: serde_json::Value = serde_json::from_str(&text(&data)).unwrap();
    assert_eq!(
        after,
        serde_json::json!({
            "version": 2,
            "notes": { "paused": false, "everyProject": true, "onRepos": [], "repoKeys": {} },
            "scripts": { "paused": true, "everyProject": false, "onRepos": [], "repoKeys": {} },
        })
    );

    // And a second run leaves it byte for byte.
    let moved = text(&data);
    ensure_global(&data).unwrap();
    assert_eq!(text(&data), moved);
}

#[test]
fn a_file_that_cannot_be_read_is_left_alone_and_the_start_goes_on() {
    let (_dir, data, _repo) = setup();

    for broken in [
        "not json",
        "",
        r#"{"version":3,"notes":{"paused":false,"everyProject":true,"onRepos":[]}}"#,
        r#"{"version":2,"notes":{"paused":false}}"#,
        r#"{"version":2,"notes":{"paused":false,"everyProject":false,"onRepos":[],"repoKeys":{"/a":7}},"scripts":{"paused":false,"everyProject":false,"onRepos":[],"repoKeys":{}}}"#,
        r#"{"version":1}"#,
    ] {
        put(&data, broken);

        ensure_global(&data).unwrap();
        assert_eq!(text(&data), broken, "{broken:?} stays as it is");
        assert!(
            matches!(read_global(&data), Err(BusinessError::Protocol(_))),
            "{broken:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_symbolic_link_is_refused_and_nothing_is_written_through_it() {
    let (dir, data, _repo) = setup();
    let elsewhere = dir.path().join("elsewhere.json");
    std::fs::write(&elsewhere, V1).unwrap();
    std::fs::create_dir_all(data.join(GUARDS_DIR)).unwrap();
    std::os::unix::fs::symlink(&elsewhere, global_path(&data)).unwrap();

    ensure_global(&data).unwrap();

    assert_eq!(std::fs::read_to_string(&elsewhere).unwrap(), V1, "not written through");
    assert!(std::fs::symlink_metadata(global_path(&data)).unwrap().is_symlink());
}

#[cfg(unix)]
#[test]
fn a_folder_that_cannot_be_written_is_an_error_when_the_file_is_missing() {
    let (dir, _data, _repo) = setup();
    let data = open_data_folder(dir.path());

    assert!(matches!(ensure_global(&data), Err(BusinessError::InvalidInput(_))));
    assert!(!global_path(&data).exists());
}

#[test]
fn a_version_one_file_reads_as_saving_off_before_anything_moved_it() {
    let (_dir, data, _repo) = setup();
    put(&data, V1);

    let read = read_global(&data).unwrap();
    assert_eq!(
        read,
        MemoryGuards {
            notes: GlobalKindGuard {
                paused: true,
                ..GlobalKindGuard::default()
            },
            scripts: GlobalKindGuard::default(),
            migration_pending: true,
        }
    );
    assert_eq!(text(&data), V1, "reading changes nothing");

    let pause = MemoryGuardsPatch {
        paused: Some(true),
        ..patch(MemoryGuardKind::Scripts)
    };
    let updated = update_global(&data, &pause).unwrap();
    assert!(updated.notes.paused && updated.scripts.paused);

    let file: serde_json::Value = serde_json::from_str(&text(&data)).unwrap();
    assert_eq!(file["version"], 2, "a change writes version 2");
    assert_eq!(file["notes"]["onRepos"], serde_json::json!([]));
}

#[test]
fn every_project_is_turned_on_and_off_per_kind() {
    let (_dir, data, _repo) = setup();

    let on = MemoryGuardsPatch {
        every_project: Some(true),
        ..patch(MemoryGuardKind::Notes)
    };
    let guards = update_global(&data, &on).unwrap();
    assert!(guards.notes.every_project);
    assert!(!guards.scripts.every_project);

    let file: serde_json::Value = serde_json::from_str(&text(&data)).unwrap();
    assert_eq!(file["notes"]["everyProject"], true);
    assert_eq!(file["scripts"]["everyProject"], false);

    let off = MemoryGuardsPatch {
        every_project: Some(false),
        ..patch(MemoryGuardKind::Notes)
    };
    assert!(!update_global(&data, &off).unwrap().notes.every_project);
}

#[test]
fn a_repository_is_removed_by_a_folder_inside_it_and_too_many_are_refused() {
    let (_dir, data, repo) = setup();
    let root = repo.to_string_lossy().into_owned();

    let add = MemoryGuardsPatch {
        add_repo: Some(root.clone()),
        ..patch(MemoryGuardKind::Scripts)
    };
    assert_eq!(update_global(&data, &add).unwrap().scripts.on_repos, [root]);

    let remove = MemoryGuardsPatch {
        remove_repo: Some(repo.join("src").to_string_lossy().into_owned()),
        ..patch(MemoryGuardKind::Scripts)
    };
    assert!(update_global(&data, &remove).unwrap().scripts.on_repos.is_empty());

    let many = MemoryGuards {
        notes: GlobalKindGuard {
            on_repos: (0..=MAX_ON_REPOS).map(|index| format!("/gone/{index}")).collect(),
            ..GlobalKindGuard::default()
        },
        ..MemoryGuards::default()
    };
    assert!(matches!(
        write_global(&data, &many),
        Err(BusinessError::InvalidInput(_))
    ));
}
