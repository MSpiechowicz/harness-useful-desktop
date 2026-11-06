//! A chat or an agent whose project path goes through a link (a linked parent such as
//! `~/Projekty -> /data/projects`) is in the repository the user turned on: the entry is the
//! canonical target (`add_repo` resolves it), and the UI's root, the hooks' state and the chat's
//! file all resolve the path the same way. Harness Memory tests the server's side
//! (`tests/mcp_guard_linked_parent.rs` in its repository).

use std::os::unix::fs::symlink;

use pretty_assertions::assert_eq;

use super::changes::{add_repo, text};
use super::*;
use crate::hooks::{MemoryGuardState, SaveRule, read_memory_guards};

#[test]
fn a_project_reached_through_a_linked_parent_is_on_by_its_canonical_entry() {
    let (dir, data, _repo) = setup();
    let base = dunce::canonicalize(dir.path()).unwrap();
    let real = base.join("real");
    let project = real.join("proj");
    std::fs::create_dir_all(project.join(".git")).unwrap();

    // `linked/proj` is the project as the user's shell and the agent name it.
    symlink(&real, base.join("linked")).unwrap();
    let through_link = base.join("linked").join("proj");

    // The user turns it on from the linked path: the entry is the canonical target.
    let added = add_repo(&data, MemoryGuardKind::Notes, &through_link);
    assert_eq!(added.notes.on_repos, [text(&project)]);
    assert!(added.notes.dead_repos.is_empty(), "the entry is alive");

    // The UI's root for the folder, as `memory_saving_root` answers.
    assert_eq!(
        saving_root_listed(Some(&data), &through_link),
        Some(text(&project)),
        "the UI compares the canonical root"
    );
    assert_eq!(saving_root_of_folder(&through_link), Some(text(&project)));

    // The chat's file names the root the hooks and the server resolve.
    let guards = chat(false, true, true);
    write_chat(&data, KEY, &guards, chat_repo_root(&through_link).as_deref()).unwrap();
    let root = read_chat_repo(&data, KEY).unwrap();
    assert_eq!(root.as_deref(), Some(text(&project).as_str()));

    // The hooks' state for that root: notes are on, scripts were never turned on.
    let state = read_memory_guards(&data, Some(KEY), root.as_deref().map(Path::new));
    assert_eq!(
        state,
        MemoryGuardState {
            notes: SaveRule::Saved,
            scripts: SaveRule::ProjectOff
        }
    );

    // A folder in the project, reached the same way, has the same root.
    std::fs::create_dir(project.join("src")).unwrap();
    let inside = through_link.join("src");
    assert_eq!(saving_root_listed(Some(&data), &inside), Some(text(&project)));
}
