//! Who may change the routines root (Unix): the root, `routines/` and `team/` are read only while
//! the user owns them and neither group nor others can write to them, so no other account can
//! swap a folder or plant a routine that runs as this user. A root of the user's own that others
//! can write to loses their write access, keeping its other bits (never through a symbolic link).
#![cfg(unix)]

mod routine_support;

use std::os::unix::fs::PermissionsExt;

use harness_business::BusinessError;
use harness_business::model::HookAuthor;
use harness_business::routines::RoutineStore;
use harness_business::routines::team::{set_team_enabled, team_views};
use harness_business::signed_docs::owned_by_current_user;
use pretty_assertions::assert_eq;
use routine_support::{body, command_draft, command_json, moment, rid, root, team_row};

#[test]
fn a_root_of_the_user_others_can_write_to_loses_their_write_access_and_is_read() {
    let root = root();
    let mine = root
        .store
        .save(&command_draft("Report"), HookAuthor::Desktop, moment(0))
        .unwrap();

    let row = team_row(&rid(1), body(&rid(1), "Nightly", false, command_json()), false);
    root.folder.write_row(&row).unwrap();
    set_team_enabled(&root.store, &root.folder, &rid(1), true, None).unwrap();

    std::fs::set_permissions(root.store.root(), std::fs::Permissions::from_mode(0o770)).unwrap();

    let listed = root.store.list();
    assert_eq!(listed.folder_problem, None);
    assert_eq!(listed.routines[0].id, mine.id);
    assert_eq!(mode(root.store.root()), 0o750, "tightened, not refused");
    assert_eq!(team_views(&root.store, &root.folder, true).views.len(), 1);

    std::fs::set_permissions(root.store.root(), std::fs::Permissions::from_mode(0o777)).unwrap();
    root.store
        .save(&command_draft("Another"), HookAuthor::Desktop, moment(1))
        .unwrap();
    assert_eq!(mode(root.store.root()), 0o755);
}

#[test]
fn a_root_that_is_a_link_to_a_folder_others_can_write_to_is_refused_and_left_alone() {
    let temp = tempfile::tempdir().unwrap();
    let shared = temp.path().join("shared");
    std::fs::create_dir(&shared).unwrap();
    std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o777)).unwrap();

    let link = temp.path().join("harness-routines");
    std::os::unix::fs::symlink(&shared, &link).unwrap();
    let store = RoutineStore::new(link, temp.path().join("app-config"));

    let refused = store.save(&command_draft("Report"), HookAuthor::Desktop, moment(0));
    assert!(matches!(refused, Err(BusinessError::Forbidden(_))), "{refused:?}");

    let problem = store.list().folder_problem.unwrap();
    assert!(problem.contains("root folder"), "{problem}");
    assert_eq!(mode(&shared), 0o777, "nothing is changed through a link");
}

fn mode(path: &std::path::Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn only_what_this_user_owns_counts_as_theirs() {
    let root = root();
    root.store
        .save(&command_draft("Report"), HookAuthor::Desktop, moment(0))
        .unwrap();

    for path in [root.store.root().to_path_buf(), root.store.routines_dir()] {
        assert!(owned_by_current_user(&std::fs::metadata(&path).unwrap()), "{path:?}");
    }

    // `/` belongs to root: another user, unless the tests run as root (who owns what they create).
    let ours = std::fs::metadata(root.store.root()).unwrap();
    let running_as_root = std::os::unix::fs::MetadataExt::uid(&ours) == 0;
    assert_eq!(owned_by_current_user(&std::fs::metadata("/").unwrap()), running_as_root);
}
