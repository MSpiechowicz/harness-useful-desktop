//! `project_files` and the files the working tree lost: a tracked file that was deleted is not
//! listed, and so does not crowd the files that are there out of a short list.

use std::fs;

use harness_business::git::ProjectFileIndex;

use super::git_project_files::{base_repo, paths, search};
use crate::git_support::git_home;

/// More than the 400 files a search for one result looks at on the disk.
const DELETED: usize = 450;

#[tokio::test]
async fn many_deleted_files_neither_crowd_the_list_nor_are_listed() {
    let home = git_home();
    let mut repo = base_repo(&home);
    for number in 0..DELETED {
        repo.write(&format!("gone/f{number:03}.md"), "x\n");
    }
    repo.commit("files that will go");
    fs::remove_dir_all(repo.path.join("gone")).unwrap();

    let index = ProjectFileIndex::build(&home.env, &home.runner, &repo.path)
        .await
        .unwrap();

    // The deleted files are changed for git status and would rank first. Without them, the files that
    // are there come out of a one-result search.
    assert_eq!(index.search(&repo.path, "", 1).len(), 1);
    assert_eq!(
        index.len(),
        6,
        "the four files of the base repository and their two folders"
    );

    let listed = search(&home, &repo.path, "").await;

    assert!(
        listed.iter().all(|file| !file.path.starts_with("gone")),
        "{:?}",
        paths(&listed)
    );
    assert!(listed.iter().all(|file| !file.changed), "{:?}", paths(&listed));
}

#[tokio::test]
async fn a_file_that_was_staged_for_deletion_or_edited_is_handled_as_before() {
    let home = git_home();
    let mut repo = base_repo(&home);
    repo.write("kept.md", "x\n");
    repo.write("removed.md", "x\n");
    repo.commit("two more");

    repo.git(&["rm", "-q", "removed.md"]);
    repo.write("kept.md", "edited\n");

    let listed = search(&home, &repo.path, "").await;

    assert!(!paths(&listed).contains(&"removed.md"), "{:?}", paths(&listed));
    assert!(listed.iter().any(|file| file.path == "kept.md" && file.changed));
}
