//! The working folders of global routines (see "Folders" in `docs/routines-format.md`): one
//! private folder per routine in `<root>/workspace` (`team-<id>` for a team routine), created when
//! missing, a real folder of the routines root, never a symbolic link and never one other users
//! can change, and deleted once its routine is gone.

mod routine_support;

use harness_business::model::{HookAuthor, HookOrigin};
use harness_business::routines::{
    checked_workspace, ensure_routine_workspace, forget_workspaces, routine_workspaces, workspace_dir,
    workspace_in_use, workspace_name, workspace_owner,
};
use pretty_assertions::assert_eq;
use routine_support::{command_draft, moment, root};

const FIRST: &str = "0123456789abcdef0123456789abcdef";
const SECOND: &str = "fedcba9876543210fedcba9876543210";

#[test]
fn each_global_routine_runs_in_a_private_folder_of_its_own() {
    let root = root();
    let base = root.store.root();

    let first = ensure_routine_workspace(base, FIRST).unwrap();
    let second = ensure_routine_workspace(base, SECOND).unwrap();
    let parent = dunce::canonicalize(workspace_dir(base)).unwrap();

    assert_eq!(first, parent.join(FIRST));
    assert_eq!(second, parent.join(SECOND));
    assert_eq!(
        ensure_routine_workspace(base, FIRST).unwrap(),
        first,
        "the folder is reused"
    );

    assert_eq!(checked_workspace(base, None).unwrap(), parent);
    assert_eq!(routine_workspaces(base), [first.clone(), second]);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        for dir in [&parent, &first] {
            assert_eq!(std::fs::metadata(dir).unwrap().permissions().mode() & 0o777, 0o700);
        }
    }

    assert!(
        ensure_routine_workspace(base, "../escape").is_err(),
        "only a routine's folder name names a folder"
    );
}

#[cfg(unix)]
#[test]
fn a_workspace_that_is_a_symbolic_link_is_refused() {
    let root = root();
    let base = root.store.root();
    let elsewhere = root.temp.path().join("elsewhere");
    std::fs::create_dir_all(elsewhere.join(FIRST)).unwrap();
    std::fs::create_dir_all(base).unwrap();

    std::os::unix::fs::symlink(&elsewhere, workspace_dir(base)).unwrap();

    assert!(ensure_routine_workspace(base, FIRST).is_err());
    assert!(checked_workspace(base, None).is_err());
    assert!(checked_workspace(base, Some(FIRST)).is_err());
    assert!(routine_workspaces(base).is_empty());
}

#[cfg(unix)]
#[test]
fn a_routine_folder_that_is_a_symbolic_link_is_refused() {
    let root = root();
    let base = root.store.root();
    let elsewhere = root.temp.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();

    let kept = ensure_routine_workspace(base, SECOND).unwrap();
    std::os::unix::fs::symlink(&elsewhere, workspace_dir(base).join(FIRST)).unwrap();

    assert!(ensure_routine_workspace(base, FIRST).is_err());
    assert!(checked_workspace(base, Some(FIRST)).is_err());
    assert_eq!(routine_workspaces(base), [kept], "the link is left out");
}

#[cfg(unix)]
#[test]
fn a_workspace_others_can_write_to_is_refused() {
    use std::os::unix::fs::PermissionsExt;

    let root = root();
    let base = root.store.root();
    let folder = ensure_routine_workspace(base, FIRST).unwrap();

    // The root of the user's own loses the write bits instead (see `routines_folder_owner.rs`).
    for (path, mode) in [(&folder, 0o777), (&workspace_dir(base), 0o770)] {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();

        let refused = checked_workspace(base, Some(FIRST));
        assert!(refused.is_err(), "{path:?} at {mode:o}: {refused:?}");

        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(checked_workspace(base, Some(FIRST)).unwrap(), folder);
    }
}

#[test]
fn a_team_routine_has_a_folder_of_its_own_beside_the_local_one_with_its_id() {
    let root = root();
    let base = root.store.root();

    let local = ensure_routine_workspace(base, &workspace_name(FIRST, HookOrigin::Local)).unwrap();
    let team = ensure_routine_workspace(base, &workspace_name(FIRST, HookOrigin::Team)).unwrap();

    assert_ne!(local, team);
    assert_eq!(team.file_name().unwrap(), format!("team-{FIRST}").as_str());
    assert_eq!(
        workspace_owner(&format!("team-{FIRST}")),
        Some((FIRST, HookOrigin::Team))
    );
    assert_eq!(workspace_owner(FIRST), Some((FIRST, HookOrigin::Local)));
    assert_eq!(workspace_owner("team-x"), None);
    assert!(ensure_routine_workspace(base, "team-../x").is_err());
}

#[test]
fn the_folders_of_routines_that_are_gone_are_deleted_unless_a_run_keeps_them() {
    let root = root();
    let base = root.store.root();
    let saved = root
        .store
        .save(&command_draft("Kept"), HookAuthor::Desktop, moment(0))
        .unwrap();

    let kept = ensure_routine_workspace(base, &saved.id).unwrap();
    let gone = ensure_routine_workspace(base, FIRST).unwrap();
    let running = ensure_routine_workspace(base, SECOND).unwrap();
    let team = ensure_routine_workspace(base, &workspace_name(SECOND, HookOrigin::Team)).unwrap();
    std::fs::write(gone.join("notes.txt"), "left by a run").unwrap();

    assert!(workspace_in_use(base, &saved.id));
    assert!(!workspace_in_use(base, FIRST));

    let forgotten = forget_workspaces(base, |name| name == SECOND);

    assert_eq!(forgotten, [FIRST.to_owned(), format!("team-{SECOND}")]);
    assert!(kept.is_dir(), "its routine is there");
    assert!(running.is_dir(), "a run keeps it");
    assert!(!gone.exists() && !team.exists());
}

#[cfg(unix)]
#[test]
fn forgetting_never_follows_a_symbolic_link() {
    let root = root();
    let base = root.store.root();
    let elsewhere = root.temp.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::write(elsewhere.join("keep.txt"), "mine").unwrap();

    let inside = ensure_routine_workspace(base, SECOND).unwrap();
    std::os::unix::fs::symlink(&elsewhere, inside.join("link")).unwrap();
    ensure_routine_workspace(base, FIRST).unwrap();
    std::fs::remove_dir(workspace_dir(base).join(FIRST)).unwrap();
    std::os::unix::fs::symlink(&elsewhere, workspace_dir(base).join(FIRST)).unwrap();

    let forgotten = forget_workspaces(base, |_| false);

    assert_eq!(forgotten, [SECOND.to_owned()], "the link in place of a folder is left");
    assert!(workspace_dir(base).join(FIRST).is_symlink());
    assert_eq!(std::fs::read_to_string(elsewhere.join("keep.txt")).unwrap(), "mine");
}
