//! `project_files` over temporary repositories: what is listed, what is never offered, and how
//! matches are ranked. The walk of a plain folder and the cache have test files of their own.

use crate::git_support;

use std::fs;
use std::path::Path;

use git_support::{GitHome, Repo, git_home};
use harness_business::git::project_files;
use harness_business::model::{MAX_PROJECT_FILES, ProjectFile};
use pretty_assertions::assert_eq;

pub(crate) async fn search(home: &GitHome, project: &Path, query: &str) -> Vec<ProjectFile> {
    project_files(&home.env, &home.runner, project, query, MAX_PROJECT_FILES)
        .await
        .unwrap()
}

pub(crate) fn paths(files: &[ProjectFile]) -> Vec<&str> {
    files.iter().map(|file| file.path.as_str()).collect()
}

pub(crate) fn find<'a>(files: &'a [ProjectFile], path: &str) -> &'a ProjectFile {
    files
        .iter()
        .find(|file| file.path == path)
        .unwrap_or_else(|| panic!("{path} is not among {:?}", paths(files)))
}

/// A repository with one commit holding `src/main.rs`, `src/lib.rs`, `docs/guide.md` and `README.md`.
pub(crate) fn base_repo(home: &GitHome) -> Repo {
    let mut repo = home.repo("repo");
    for name in ["src/main.rs", "src/lib.rs", "docs/guide.md", "README.md"] {
        repo.write(name, "x\n");
    }
    repo.commit("base");
    repo
}

#[tokio::test]
async fn tracked_and_untracked_files_are_listed_with_their_folders_and_ignored_ones_are_not() {
    let home = git_home();
    let mut repo = base_repo(&home);
    repo.write(".gitignore", "*.tmp\nbuild-output/\n");
    repo.commit("ignore");

    repo.write("src/new.rs", "untracked\n");
    repo.write("scratch.tmp", "ignored\n");
    repo.write("build-output/app.bin", "ignored\n");
    repo.write("src/nested/ignored.tmp", "ignored\n");

    let files = search(&home, &repo.path, "").await;
    let listed = paths(&files);

    for expected in [
        "README.md",
        "src/main.rs",
        "src/lib.rs",
        "src/new.rs",
        "docs/guide.md",
        ".gitignore",
    ] {
        assert!(listed.contains(&expected), "{expected} is missing from {listed:?}");
    }
    for hidden in [
        "scratch.tmp",
        "build-output/app.bin",
        "build-output",
        "src/nested/ignored.tmp",
        "src/nested",
    ] {
        assert!(!listed.contains(&hidden), "{hidden} must stay out of {listed:?}");
    }

    let folders: Vec<&str> = files
        .iter()
        .filter(|file| file.is_dir)
        .map(|file| file.path.as_str())
        .collect();
    assert_eq!(folders.len(), 2, "{folders:?}");
    assert!(folders.contains(&"src") && folders.contains(&"docs"));

    assert!(
        files
            .iter()
            .all(|file| !file.path.contains('\\') && !file.path.ends_with('/'))
    );
}

#[tokio::test]
async fn changed_files_come_first_without_a_query_then_shallow_paths_then_alphabetical() {
    let home = git_home();
    let repo = base_repo(&home);
    repo.write("src/lib.rs", "edited\n");
    repo.write("zz/deep/new.txt", "untracked\n");

    let files = search(&home, &repo.path, "").await;
    let listed = paths(&files);

    assert_eq!(
        listed,
        [
            "src/lib.rs",
            "zz/deep/new.txt",
            "docs",
            "README.md",
            "src",
            "zz",
            "docs/guide.md",
            "src/main.rs",
            "zz/deep",
        ]
    );
    assert!(files[0].changed && files[1].changed && files[2..].iter().all(|file| !file.changed));
}

#[tokio::test]
async fn a_much_better_match_beats_a_changed_file_but_a_close_one_does_not() {
    let home = git_home();
    let mut repo = home.repo("repo");
    for name in ["src/router.rs", "src/old/xrxoxuxtxexr.rs", "src/rotator.rs"] {
        repo.write(name, "x\n");
    }
    repo.commit("base");
    repo.write("src/old/xrxoxuxtxexr.rs", "edited\n");

    let files = search(&home, &repo.path, "router").await;

    assert_eq!(
        paths(&files)[0],
        "src/router.rs",
        "a clean match at the start of the name wins"
    );
    assert!(find(&files, "src/old/xrxoxuxtxexr.rs").changed);

    // Two matches of about the same quality: the changed one leads.
    let mut repo = home.repo("repo2");
    for name in ["a/util.rs", "b/util.rs"] {
        repo.write(name, "x\n");
    }
    repo.commit("base");
    repo.write("b/util.rs", "edited\n");

    let files = search(&home, &repo.path, "util").await;
    assert_eq!(&paths(&files)[..2], ["b/util.rs", "a/util.rs"]);
}

#[tokio::test]
async fn a_match_in_the_file_name_beats_one_in_a_folder_and_shorter_paths_win_ties() {
    let home = git_home();
    let mut repo = home.repo("repo");
    for name in [
        "main/readme.md",
        "src/main.rs",
        "src/deeper/path/main.rs",
        "m/a/i/n.txt",
    ] {
        repo.write(name, "x\n");
    }
    repo.commit("base");

    let files = search(&home, &repo.path, "main").await;
    let listed = paths(&files);

    // The folder `main` and the files named main.rs lead; `src/main.rs` is the shorter of the two files.
    assert_eq!(&listed[..3], ["main", "src/main.rs", "src/deeper/path/main.rs"]);
    assert_eq!(
        listed[3], "main/readme.md",
        "a match that needs the folder comes after the ones in a file name"
    );
}

#[tokio::test]
async fn matching_is_case_insensitive_and_accepts_scattered_characters() {
    let home = git_home();
    let repo = base_repo(&home);

    let files = search(&home, &repo.path, "READme").await;
    assert_eq!(paths(&files), ["README.md"]);

    let files = search(&home, &repo.path, "smr").await;
    assert_eq!(paths(&files), ["src/main.rs"]);

    assert!(search(&home, &repo.path, "zzzz").await.is_empty());
}

#[tokio::test]
async fn secrets_are_never_offered_whatever_the_query() {
    let home = git_home();
    let mut repo = base_repo(&home);
    for name in [
        ".env",
        ".env.local",
        ".ssh/id_rsa",
        ".ssh/config",
        ".ssh/known_hosts",
        "deploy/.SSH/authorized_keys",
        "secrets/.env",
        "certs/server.pem",
        "data/app.sqlite3",
    ] {
        repo.write(name, "secret\n");
    }
    repo.commit("with secrets");
    repo.write("untracked/.env", "secret\n");

    for query in [
        "",
        "env",
        ".env",
        "ssh",
        "config",
        "known_hosts",
        "authorized_keys",
        "deploy",
        "id_rsa",
        "pem",
        "server",
        "sqlite",
        "secrets",
        "certs",
        "data",
        "untracked",
    ] {
        let files = search(&home, &repo.path, query).await;

        for file in &files {
            let lower = file.path.to_lowercase();
            assert!(
                !lower.contains(".env")
                    && !lower.contains("id_rsa")
                    && !lower.ends_with(".pem")
                    && !lower.ends_with(".sqlite3"),
                "{:?} for {query:?}",
                file.path
            );
            // Nothing of a `.ssh` folder, whatever its case, and not the folder itself.
            assert!(
                file.path.split('/').all(|part| !part.eq_ignore_ascii_case(".ssh")),
                "{:?} for {query:?}",
                file.path
            );
            // No folder that only holds denied files.
            assert!(
                !matches!(file.path.as_str(), ".ssh" | "secrets" | "certs" | "data" | "untracked"),
                "{:?} for {query:?}",
                file.path
            );
        }
    }

    let everything = search(&home, &repo.path, "").await;
    assert!(paths(&everything).contains(&"README.md"));
    assert!(
        !paths(&everything).contains(&"deploy"),
        "a folder that only holds `.ssh` files"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_link_to_a_private_file_is_refused_by_the_full_deny_check() {
    let home = git_home();
    let mut repo = base_repo(&home);
    repo.write(".claude/history.jsonl", "private\n");
    std::os::unix::fs::symlink(".claude/history.jsonl", repo.path.join("notes.txt")).unwrap();
    repo.commit("with a link");

    let files = search(&home, &repo.path, "notes").await;

    assert!(files.is_empty(), "{:?}", paths(&files));
}

#[tokio::test]
async fn paths_with_spaces_and_unicode_are_returned_as_they_are() {
    let home = git_home();
    let mut repo = home.repo("repo");
    for name in ["my notes/plan b.md", "zażółć/gęślą.txt", "plain.txt"] {
        repo.write(name, "x\n");
    }
    repo.commit("base");

    let files = search(&home, &repo.path, "").await;
    let listed = paths(&files);
    for expected in ["my notes/plan b.md", "my notes", "zażółć/gęślą.txt", "zażółć"] {
        assert!(listed.contains(&expected), "{expected} is missing from {listed:?}");
    }

    let found = search(&home, &repo.path, "plan b").await;
    assert_eq!(paths(&found), ["my notes/plan b.md"]);
    let found = search(&home, &repo.path, "GĘŚL").await;
    assert_eq!(paths(&found), ["zażółć/gęślą.txt"]);
}

#[cfg(unix)]
#[tokio::test]
async fn a_name_with_a_control_character_or_invalid_utf8_is_never_listed() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let home = git_home();
    let repo = base_repo(&home);
    fs::write(repo.path.join("bad\nname.txt"), "x\n").unwrap();
    fs::write(repo.path.join("tab\tname.txt"), "x\n").unwrap();
    fs::write(repo.path.join(OsStr::from_bytes(b"raw\xffbytes.txt")), "x\n").unwrap();
    fs::create_dir_all(repo.path.join("odd\u{7}dir")).unwrap();
    fs::write(repo.path.join("odd\u{7}dir/inside.txt"), "x\n").unwrap();

    let files = search(&home, &repo.path, "").await;

    assert!(
        files
            .iter()
            .all(|file| !file.path.contains(char::is_control) && !file.path.contains('\u{fffd}')),
        "{:?}",
        paths(&files)
    );
    assert!(paths(&files).contains(&"README.md"));
    assert!(search(&home, &repo.path, "name").await.is_empty());
    assert!(search(&home, &repo.path, "inside").await.is_empty());
}

#[tokio::test]
async fn a_project_inside_a_repository_sees_only_its_own_files_relative_to_itself() {
    let home = git_home();
    let mut repo = base_repo(&home);
    repo.write("app/package.json", "{}\n");
    repo.commit("app");
    repo.write("app/web/index.ts", "x\n");

    let files = search(&home, &repo.path.join("app"), "").await;

    assert_eq!(paths(&files), ["web/index.ts", "package.json", "web"]);
    assert!(files[0].changed && !files[1].changed);
}

#[tokio::test]
async fn a_tracked_file_deleted_from_the_work_tree_is_not_offered() {
    let home = git_home();
    let repo = base_repo(&home);
    fs::remove_file(repo.path.join("src/lib.rs")).unwrap();

    let files = search(&home, &repo.path, "lib").await;

    assert!(files.is_empty(), "{:?}", paths(&files));
}

#[tokio::test]
async fn a_folder_of_a_thousand_untracked_files_marks_each_of_them_changed() {
    let home = git_home();
    let repo = base_repo(&home);
    for number in 0..1001 {
        repo.write(&format!("bulk/f{number:04}.txt"), "x\n");
    }

    let files = search(&home, &repo.path, "f0500").await;

    assert_eq!(paths(&files), ["bulk/f0500.txt"]);
    assert!(files[0].changed);
    assert!(!find(&search(&home, &repo.path, "guide").await, "docs/guide.md").changed);
}

#[tokio::test]
async fn a_nested_repository_is_offered_as_a_folder_unless_the_deny_list_names_it() {
    let home = git_home();
    let repo = base_repo(&home);

    for name in ["vendored", "libs/dep", ".ssh"] {
        let mut nested = home.repo(&format!("repo/{name}"));
        nested.commit_file("inner.txt", "x\n");
    }

    let files = search(&home, &repo.path, "").await;
    let listed = paths(&files);

    for folder in ["vendored", "libs/dep", "libs"] {
        let file = find(&files, folder);
        assert!(file.is_dir && !file.path.ends_with('/'), "{file:?}");
    }
    assert_eq!(listed.iter().filter(|path| **path == "libs").count(), 1, "{listed:?}");
    assert!(
        find(&files, "vendored").changed,
        "an untracked nested repository is a change"
    );

    assert!(listed.iter().all(|path| !path.contains(".ssh")), "{listed:?}");
    assert!(!listed.contains(&"vendored/inner.txt"), "{listed:?}");
}

#[tokio::test]
async fn an_empty_project_gives_an_empty_list() {
    let home = git_home();
    let plain = home.home.join("plain");
    fs::create_dir_all(&plain).unwrap();
    assert!(search(&home, &plain, "").await.is_empty());
    assert!(search(&home, &plain, "anything").await.is_empty());

    let fresh = home.repo("fresh");
    assert!(search(&home, &fresh.path, "").await.is_empty());
}

#[tokio::test]
async fn the_limit_is_clamped_to_one_to_fifty_and_a_long_query_matches_nothing() {
    let home = git_home();
    let mut repo = home.repo("repo");
    for number in 0..70 {
        repo.write(&format!("file{number:02}.txt"), "x\n");
    }
    repo.commit("base");

    let take = |limit: usize| {
        let (env, runner, path) = (&home.env, &home.runner, &repo.path);
        async move { project_files(env, runner, path, "", limit).await.unwrap() }
    };

    assert_eq!(take(0).await.len(), 1);
    assert_eq!(take(7).await.len(), 7);
    assert_eq!(take(50).await.len(), 50);
    assert_eq!(take(5000).await.len(), 50);
    assert_eq!(take(usize::MAX).await.len(), 50);

    let long = "a".repeat(257);
    assert!(search(&home, &repo.path, &long).await.is_empty());
}

#[tokio::test]
async fn the_same_request_gives_the_same_order_every_time() {
    let home = git_home();
    let mut repo = home.repo("repo");
    for number in 0..40 {
        repo.write(&format!("dir{}/Item{number}.txt", number % 4), "x\n");
        repo.write(&format!("dir{}/note{number}.md", number % 3), "x\n");
    }
    repo.commit("base");
    repo.write("dir1/Item1.txt", "edited\n");

    for query in ["", "item", "txt", "d1", "note"] {
        let first = search(&home, &repo.path, query).await;
        for _ in 0..3 {
            assert_eq!(search(&home, &repo.path, query).await, first, "{query:?}");
        }
    }
}
