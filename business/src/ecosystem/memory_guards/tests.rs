use pretty_assertions::assert_eq;

use std::collections::BTreeMap;

use super::*;
use crate::model::{GlobalKindGuard, MemoryGuardKind};

mod changes;
mod chats;
mod dead_repos;
mod global;
mod hardening;
#[cfg(unix)]
mod linked_parent;
mod read_lock;
mod saving_root;
mod saving_root_cases;
#[cfg(unix)]
mod saving_root_links;

const KEY: &str = "0f8e5a7c-1d2b-4c3a-9e8f-7a6b5c4d3e2f";

/// [`super::update_global`] for a patch that may turn a repository on: it is pinned with the key a
/// repository with no remote has (the key the server asks of the folder is its own concern, see
/// `memory_project_key`).
fn update_global(data_dir: &Path, patch: &MemoryGuardsPatch) -> Result<MemoryGuards> {
    let pin = patch.add_repo.as_ref().map(|repo| Pin(Some(format!("path:{repo}"))));

    super::update_global(data_dir, patch, pin)
}

/// A data folder that does not exist yet, inside a temporary folder that holds a git repository
/// `repo` with a subfolder `repo/src`.
fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    std::fs::create_dir_all(repo.join("src")).unwrap();

    let repo = dunce::canonicalize(&repo).unwrap();
    let data = dir.path().join("data");

    (dir, data, repo)
}

fn chat(paused: bool, notes: bool, scripts: bool) -> MemoryChatGuards {
    MemoryChatGuards {
        paused,
        notes: KindGuard { on: notes, ask: false },
        scripts: KindGuard { on: scripts, ask: true },
    }
}

/// A data folder in `dir` that other users can write to (`0757`).
#[cfg(unix)]
fn open_data_folder(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let data = dir.join("shared data");
    std::fs::create_dir(&data).unwrap();
    std::fs::set_permissions(&data, std::fs::Permissions::from_mode(0o757)).unwrap();

    data
}

#[cfg(unix)]
fn mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;

    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn chat_keys_are_lowercase_uuids() {
    assert!(is_chat_key(KEY));

    for bad in [
        "",
        "0F8E5A7C-1D2B-4C3A-9E8F-7A6B5C4D3E2F",
        "0f8e5a7c1d2b4c3a9e8f7a6b5c4d3e2f",
        "0f8e5a7c-1d2b-4c3a-9e8f-7a6b5c4d3e2",
        "../../../etc/passwd-aaaa-bbbb-cccc",
        "0f8e5a7c-1d2b-4c3a-9e8f-7a6b5c4d3e2g",
    ] {
        assert!(!is_chat_key(bad), "{bad:?}");
        assert!(
            matches!(check_chat_key(bad), Err(BusinessError::InvalidInput(_))),
            "{bad:?}"
        );
    }
}

#[test]
fn missing_files_leave_saving_off() {
    let (_dir, data, _repo) = setup();

    assert_eq!(read_global(&data).unwrap(), MemoryGuards::default());
    assert_eq!(read_chat(&data, KEY).unwrap(), None);
}

#[test]
fn global_guards_round_trip_privately_with_repositories_as_listed() {
    let (_dir, data, repo) = setup();
    let root = repo.to_string_lossy().into_owned();
    let gone = repo.with_file_name("no-such-repository").to_string_lossy().into_owned();

    let asked = MemoryGuards {
        notes: GlobalKindGuard {
            paused: false,
            every_project: false,
            on_repos: vec![
                repo.join("src").to_string_lossy().into_owned(),
                root.clone(),
                gone.clone(),
                gone.clone(),
            ],
            repo_keys: [
                (
                    repo.join("src").to_string_lossy().into_owned(),
                    Some("git:example.invalid/acme/src".to_owned()),
                ),
                (root.clone(), None),
                (gone.clone(), Some("path:/gone".to_owned())),
                // A pin of an entry that is not listed is not written.
                ("/not/listed".to_owned(), Some("path:/not/listed".to_owned())),
            ]
            .into(),
            dead_repos: Vec::new(),
            changed_repos: Vec::new(),
        },
        scripts: GlobalKindGuard {
            paused: true,
            every_project: true,
            on_repos: Vec::new(),
            repo_keys: BTreeMap::new(),
            dead_repos: Vec::new(),
            changed_repos: Vec::new(),
        },
        ..MemoryGuards::default()
    };

    let written = write_global(&data, &asked).unwrap();
    let mut expected = vec![
        repo.join("src").to_string_lossy().into_owned(),
        root.clone(),
        gone.clone(),
    ];
    expected.sort();
    assert_eq!(
        written.notes.on_repos, expected,
        "listed as given, without duplicates, sorted"
    );
    assert!(written.scripts.paused);
    // What is read also says which entries turn nothing on (the folder that is gone).
    let read = read_global(&data).unwrap();
    assert_eq!(read.notes.dead_repos, std::slice::from_ref(&gone));
    assert_eq!(read, with_dead_repos(written.clone()));
    assert_eq!(
        written.notes.repo_keys.len(),
        3,
        "the pin of an unlisted entry is dropped"
    );

    let text = std::fs::read_to_string(global_path(&data)).unwrap();
    let file: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(file["version"], 2);
    assert_eq!(file["notes"]["onRepos"][1], root.as_str());
    assert_eq!(file["notes"]["everyProject"], false);
    assert_eq!(file["scripts"]["paused"], true);
    assert_eq!(file["scripts"]["everyProject"], true);

    // Each kind holds exactly what the server reads (it refuses unknown keys): what is computed on
    // a read never reaches the file, and the pins do, `null` for a folder with no project.
    for kind in ["notes", "scripts"] {
        let mut keys: Vec<&str> = file[kind].as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["everyProject", "onRepos", "paused", "repoKeys"], "{kind}");
    }
    assert_eq!(file["notes"]["repoKeys"][&root], serde_json::Value::Null);
    assert_eq!(file["notes"]["repoKeys"][&gone], "path:/gone");
    assert_eq!(file["scripts"]["repoKeys"], serde_json::json!({}));

    #[cfg(unix)]
    {
        assert_eq!(mode(&data), 0o700);
        assert_eq!(mode(&data.join(GUARDS_DIR)), 0o700);
        assert_eq!(mode(&global_path(&data)), 0o600);
    }
}

#[test]
fn chat_guards_round_trip_privately() {
    let (_dir, data, _repo) = setup();
    let guards = chat(false, true, false);

    write_chat(&data, KEY, &guards, None).unwrap();
    assert_eq!(read_chat(&data, KEY).unwrap(), Some(guards));

    let text = std::fs::read_to_string(chat_path(&data, KEY)).unwrap();
    let file: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        file,
        serde_json::json!({
            "version": 1,
            "paused": false,
            "notes": { "on": true, "ask": false },
            "scripts": { "on": false, "ask": true },
            "repoRoot": null,
        })
    );

    #[cfg(unix)]
    {
        assert_eq!(mode(&data.join(GUARDS_DIR).join(CHATS_DIR)), 0o700);
        assert_eq!(mode(&chat_path(&data, KEY)), 0o600);
    }

    assert!(matches!(
        write_chat(&data, "not-a-key", &guards, None),
        Err(BusinessError::InvalidInput(_))
    ));
}

#[cfg(unix)]
#[test]
fn a_data_folder_other_users_can_write_to_names_the_fix() {
    let (dir, _data, _repo) = setup();
    let data = open_data_folder(dir.path());

    let refused = write_chat(&data, KEY, &chat(false, true, true), None);
    let shown = data.display().to_string();

    let Err(BusinessError::InvalidInput(message)) = &refused else {
        panic!("{refused:?}");
    };

    assert!(message.contains(&shown), "{message}");
    assert!(
        message.contains(&format!("chmod 700 '{shown}'")),
        "a path with a space is quoted: {message}"
    );
    assert_eq!(mode(&data), 0o757, "the folder is left as it is");
    assert!(read_chat(&data, KEY).unwrap().is_none(), "nothing written");
}

#[test]
fn a_relative_repository_is_refused() {
    let (_dir, data, _repo) = setup();
    let guards = MemoryGuards {
        notes: GlobalKindGuard {
            on_repos: vec!["relative/repo".to_owned()],
            ..GlobalKindGuard::default()
        },
        ..MemoryGuards::default()
    };

    assert!(matches!(
        write_global(&data, &guards),
        Err(BusinessError::InvalidInput(_))
    ));
    assert!(!global_path(&data).exists());
}

#[cfg(unix)]
#[test]
fn symbolic_links_are_never_written_through_or_read() {
    let (dir, data, _repo) = setup();
    write_chat(&data, KEY, &MemoryChatGuards::default(), None).unwrap();

    let elsewhere = dir.path().join("elsewhere.json");
    std::fs::write(
        &elsewhere,
        r#"{"version":2,"notes":{"paused":false,"everyProject":true,"onRepos":[],"repoKeys":{}},"scripts":{"paused":false,"everyProject":true,"onRepos":[],"repoKeys":{}}}"#,
    )
    .unwrap();
    std::os::unix::fs::symlink(&elsewhere, global_path(&data)).unwrap();

    assert!(write_global(&data, &MemoryGuards::default()).is_err());
    assert!(matches!(read_global(&data), Err(BusinessError::Protocol(_))));

    let chats = data.join(GUARDS_DIR).join(CHATS_DIR);
    std::fs::remove_dir_all(&chats).unwrap();
    let other = dir.path().join("other-chats");
    std::fs::create_dir(&other).unwrap();
    std::os::unix::fs::symlink(&other, &chats).unwrap();

    assert!(write_chat(&data, KEY, &MemoryChatGuards::default(), None).is_err());
    assert_eq!(
        std::fs::read_dir(&other).unwrap().count(),
        0,
        "nothing written through the link"
    );
}

#[test]
fn a_file_of_another_version_is_unreadable() {
    let (_dir, data, _repo) = setup();
    write_chat(&data, KEY, &MemoryChatGuards::default(), None).unwrap();

    std::fs::write(
        chat_path(&data, KEY),
        r#"{"version":2,"paused":false,"notes":{"on":true,"ask":false},"scripts":{"on":true,"ask":false}}"#,
    )
    .unwrap();

    assert!(matches!(read_chat(&data, KEY), Err(BusinessError::Protocol(_))));
}

fn patch(kind: MemoryGuardKind) -> MemoryGuardsPatch {
    MemoryGuardsPatch {
        kind,
        paused: None,
        add_repo: None,
        every_project: None,
        remove_repo: None,
    }
}

#[test]
fn a_patch_changes_only_what_it_names() {
    let (_dir, data, repo) = setup();
    let root = repo.to_string_lossy().into_owned();

    let paused = update_global(
        &data,
        &MemoryGuardsPatch {
            paused: Some(true),
            ..patch(MemoryGuardKind::Scripts)
        },
    )
    .unwrap();
    assert!(paused.scripts.paused);
    assert_eq!(paused.notes, GlobalKindGuard::default());

    let added = update_global(
        &data,
        &MemoryGuardsPatch {
            add_repo: Some(repo.join("src").to_string_lossy().into_owned()),
            ..patch(MemoryGuardKind::Notes)
        },
    )
    .unwrap();
    assert_eq!(
        added.notes.on_repos,
        std::slice::from_ref(&root),
        "kept as its repository"
    );
    assert!(added.scripts.paused, "the scripts pause stays");

    let removed = update_global(
        &data,
        &MemoryGuardsPatch {
            remove_repo: Some(root),
            ..patch(MemoryGuardKind::Notes)
        },
    )
    .unwrap();
    assert!(removed.notes.on_repos.is_empty());
    assert!(removed.scripts.paused);
    assert_eq!(read_global(&data).unwrap(), removed);
}

#[test]
fn patches_made_at_once_all_apply() {
    let (dir, data, _repo) = setup();
    let repos: Vec<String> = (0..16)
        .map(|index| {
            let gone = dir.path().join(format!("no-such-repository-{index:02}"));
            gone.to_string_lossy().into_owned()
        })
        .collect();

    std::thread::scope(|scope| {
        for repo in &repos {
            let data = &data;
            scope.spawn(move || {
                let add = MemoryGuardsPatch {
                    add_repo: Some(repo.clone()),
                    ..patch(MemoryGuardKind::Notes)
                };
                update_global(data, &add).unwrap();
            });
        }

        scope.spawn(|| {
            let pause = MemoryGuardsPatch {
                paused: Some(true),
                ..patch(MemoryGuardKind::Scripts)
            };
            update_global(&data, &pause).unwrap();
        });
    });

    let saved = read_global(&data).unwrap();
    assert_eq!(saved.notes.on_repos, repos);
    assert!(saved.scripts.paused);
}

#[test]
fn a_patch_over_a_file_that_cannot_be_read_writes_nothing() {
    let (_dir, data, _repo) = setup();
    write_global(&data, &MemoryGuards::default()).unwrap();
    std::fs::write(global_path(&data), "not json").unwrap();

    let pause = MemoryGuardsPatch {
        paused: Some(true),
        ..patch(MemoryGuardKind::Notes)
    };

    assert!(matches!(update_global(&data, &pause), Err(BusinessError::Protocol(_))));
    assert_eq!(std::fs::read_to_string(global_path(&data)).unwrap(), "not json");
}
