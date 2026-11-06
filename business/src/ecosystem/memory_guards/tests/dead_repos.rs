//! The entries of `onRepos` that turn nothing on ([`names_itself`]: the folder is gone, a link is
//! in the path, the spelling is not canonical) are named in `deadRepos` when the guards are read or
//! changed, so Settings can tell them from a live one. They are never written: the server refuses
//! a kind with a key it does not know.

use std::fs;

use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use super::changes::{add_repo, remove_repo, text};
use super::*;
use crate::env::Env;

/// `live` is a repository, `ghost` a folder that is gone, `spelled` is `live/src/..` and `link`
/// leads to `live`: all but `live` are dead.
struct Entries {
    _dir: tempfile::TempDir,
    data: PathBuf,
    live: String,
    ghost: String,
    spelled: String,
    link: String,
}

fn entries() -> Entries {
    let (dir, data, repo) = setup();
    let base = dunce::canonicalize(dir.path()).unwrap();

    let spelled = repo.join("src").join("..");
    let link = base.join("entry-link");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&repo, &link).unwrap();
    #[cfg(not(unix))]
    fs::create_dir_all(&link).unwrap();

    let entries = Entries {
        live: text(&repo),
        ghost: text(&base.join("ghost")),
        spelled: text(&spelled),
        link: text(&link),
        data,
        _dir: dir,
    };

    // Written as listed: the app never writes these, a moved folder or an edit leaves them.
    let asked = MemoryGuards {
        notes: GlobalKindGuard {
            on_repos: vec![
                entries.live.clone(),
                entries.ghost.clone(),
                entries.spelled.clone(),
                entries.link.clone(),
            ],
            repo_keys: [&entries.live, &entries.ghost, &entries.spelled, &entries.link]
                .map(|entry| (entry.clone(), Some(format!("path:{entry}"))))
                .into(),
            ..GlobalKindGuard::default()
        },
        scripts: GlobalKindGuard {
            on_repos: vec![entries.live.clone()],
            repo_keys: [(entries.live.clone(), Some(format!("path:{}", entries.live)))].into(),
            ..GlobalKindGuard::default()
        },
        ..MemoryGuards::default()
    };
    write_global(&entries.data, &asked).unwrap();

    entries
}

impl Entries {
    fn dead(&self) -> Vec<String> {
        let mut dead = vec![self.ghost.clone(), self.spelled.clone()];

        // Without symbolic links `link` is a plain folder, which names itself.
        if cfg!(unix) {
            dead.push(self.link.clone());
        }

        dead.sort();

        dead
    }
}

#[test]
fn a_read_names_the_entries_that_turn_nothing_on() {
    let entries = entries();

    for guards in [
        read_global(&entries.data).unwrap(),
        read_global_healed(&Env::for_tests(entries.data.parent().unwrap()), &entries.data).unwrap(),
    ] {
        assert_eq!(guards.notes.dead_repos, entries.dead());
        assert!(guards.scripts.dead_repos.is_empty(), "every script entry is alive");
        assert!(guards.notes.on_repos.contains(&entries.live));
        assert!(!guards.notes.dead_repos.contains(&entries.live));
    }
}

#[test]
fn a_change_returns_the_dead_entries_of_what_it_wrote() {
    let entries = entries();

    let paused = update_global(
        &entries.data,
        &MemoryGuardsPatch {
            paused: Some(true),
            ..patch(MemoryGuardKind::Notes)
        },
    )
    .unwrap();
    assert_eq!(paused.notes.dead_repos, entries.dead());

    // Removing one of them leaves the others named, and a new live entry is not.
    let removed = remove_repo(&entries.data, MemoryGuardKind::Notes, Path::new(&entries.ghost));
    let mut expected = entries.dead();
    expected.retain(|entry| entry != &entries.ghost);
    assert_eq!(removed.notes.dead_repos, expected);

    let base = dunce::canonicalize(Path::new(&entries.live).parent().unwrap()).unwrap();
    let other = base.join("other");
    fs::create_dir_all(other.join(".git")).unwrap();
    let added = add_repo(&entries.data, MemoryGuardKind::Notes, &other);
    assert!(added.notes.on_repos.contains(&text(&other)));
    assert_eq!(added.notes.dead_repos, expected);
}

#[test]
fn the_dead_entries_are_never_written_to_the_file() {
    let entries = entries();

    // After the write, after a change and after reads, the keys on disk are the server's.
    read_global(&entries.data).unwrap();
    update_global(
        &entries.data,
        &MemoryGuardsPatch {
            every_project: Some(true),
            ..patch(MemoryGuardKind::Scripts)
        },
    )
    .unwrap();
    read_global(&entries.data).unwrap();

    let file: Value = serde_json::from_str(&fs::read_to_string(global_path(&entries.data)).unwrap()).unwrap();
    let keys = |value: &Value| -> Vec<String> {
        let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();

        keys
    };

    assert_eq!(keys(&file), ["notes", "scripts", "version"]);

    for kind in ["notes", "scripts"] {
        assert_eq!(
            keys(&file[kind]),
            ["everyProject", "onRepos", "paused", "repoKeys"],
            "{kind}: neither deadRepos nor changedRepos reach the file"
        );
    }

    assert_eq!(
        file["notes"]["onRepos"].as_array().unwrap().len(),
        4,
        "the dead entries stay listed"
    );
    assert_eq!(file["version"], json!(2));
}
