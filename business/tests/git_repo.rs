//! `git_status`, `git_branches` and `git_diff` over temporary repositories.

mod git_support;

use std::fs;

use git_support::{GitHome, Repo, git_home};
use harness_business::git::{git_branches, git_diff, git_status};
use harness_business::model::{
    DiffLineKind, DiffRequest, DiffScope, FileDiff, GitFileStatus, IpcErrorCode, StatusEntry,
};
use pretty_assertions::assert_eq;

fn request(scope: DiffScope, paths: &[&str]) -> DiffRequest {
    DiffRequest {
        scope,
        paths: paths.iter().map(|path| (*path).to_owned()).collect(),
    }
}

async fn diff(home: &GitHome, repo: &Repo, scope: DiffScope) -> Vec<FileDiff> {
    git_diff(&home.env, &home.runner, &repo.path, &request(scope, &[]))
        .await
        .unwrap()
}

fn find<'a>(files: &'a [FileDiff], path: &str, staged: bool) -> &'a FileDiff {
    files
        .iter()
        .find(|file| file.path == path && file.staged == staged)
        .unwrap_or_else(|| panic!("no diff for {path} (staged {staged})"))
}

fn texts(file: &FileDiff) -> Vec<(DiffLineKind, String)> {
    file.hunks
        .iter()
        .flat_map(|hunk| &hunk.lines)
        .map(|line| (line.kind, line.text.clone()))
        .collect()
}

/// A repository with one commit of `a.txt` .. `d.txt`.
fn base_repo(home: &GitHome) -> Repo {
    let mut repo = home.repo("repo");
    for (name, content) in [
        ("a.txt", "one\ntwo\n"),
        ("b.txt", "b\n"),
        ("c.txt", "c\n"),
        ("d.txt", "d\nd\nd\n"),
    ] {
        repo.write(name, content);
    }
    repo.commit("base");
    repo
}

#[tokio::test]
async fn directories_outside_a_repository_are_not_repos() {
    let home = git_home();
    let plain = home.home.join("plain");
    fs::create_dir_all(&plain).unwrap();

    let status = git_status(&home.env, &home.runner, &plain).await.unwrap();
    assert!(!status.is_repo);
    assert!(status.entries.is_empty());
    assert!(git_branches(&home.env, &home.runner, &plain).await.unwrap().is_empty());

    let diffs = git_diff(&home.env, &home.runner, &plain, &request(DiffScope::All, &[])).await;
    assert!(diffs.unwrap().is_empty());
}

#[tokio::test]
async fn status_reports_staged_unstaged_untracked_and_renamed_paths() {
    let home = git_home();
    let repo = base_repo(&home);

    repo.write("a.txt", "one\ntwo\nthree\n");
    repo.write("b.txt", "b2\n");
    repo.git(&["add", "b.txt"]);
    fs::remove_file(repo.path.join("c.txt")).unwrap();
    repo.git(&["mv", "d.txt", "e.txt"]);
    repo.write("f.txt", "f\n");
    repo.git(&["add", "f.txt"]);
    repo.write("f.txt", "f2\n");
    repo.write("new dir/ü.txt", "u\n");

    let status = git_status(&home.env, &home.runner, &repo.path).await.unwrap();
    let entry = |path: &str, staged, unstaged| StatusEntry {
        path: path.to_owned(),
        orig_path: None,
        staged,
        unstaged,
    };
    use GitFileStatus::{Added, Deleted, Modified, Renamed, Untracked};

    assert!(status.is_repo);
    assert_eq!(status.branch.as_deref(), Some("main"));
    assert_eq!(status.head.as_ref().map(String::len), Some(7));
    assert_eq!((status.upstream.clone(), status.ahead, status.behind), (None, 0, 0));
    assert_eq!(
        status.entries,
        [
            entry("a.txt", None, Some(Modified)),
            entry("b.txt", Some(Modified), None),
            entry("c.txt", None, Some(Deleted)),
            StatusEntry {
                orig_path: Some("d.txt".to_owned()),
                ..entry("e.txt", Some(Renamed), None)
            },
            entry("f.txt", Some(Added), Some(Modified)),
            entry("new dir/ü.txt", None, Some(Untracked)),
        ]
    );
}

#[tokio::test]
async fn diffs_carry_line_numbers_no_newline_markers_and_crlf_content() {
    let home = git_home();
    let mut repo = home.repo("repo");
    let lines: Vec<String> = (1..=20).map(|number| format!("line {number}")).collect();
    repo.write("lib.rs", lines.join("\n"));
    repo.write("crlf.txt", "alpha\r\nbeta\r\n");
    repo.commit("base");

    let mut changed = lines.clone();
    changed[1] = "line 2 changed".to_owned();
    changed.push("line 21".to_owned());
    repo.write("lib.rs", changed.join("\n") + "\n");
    repo.write("crlf.txt", "alpha\r\nBETA\r\n");

    let files = diff(&home, &repo, DiffScope::Unstaged).await;
    let lib = find(&files, "lib.rs", false);

    assert_eq!(lib.status, GitFileStatus::Modified);
    assert_eq!(lib.hunks.len(), 2);
    assert_eq!((lib.additions, lib.deletions), (3, 2));

    let first = &lib.hunks[0];
    assert_eq!(
        (first.old_start, first.old_lines, first.new_start, first.new_lines),
        (1, 5, 1, 5)
    );
    let removed = first
        .lines
        .iter()
        .find(|line| line.kind == DiffLineKind::Remove)
        .unwrap();
    assert_eq!(
        (removed.old_no, removed.new_no, removed.text.as_str()),
        (Some(2), None, "line 2")
    );

    let last = &lib.hunks[1];
    let kinds: Vec<DiffLineKind> = last.lines.iter().map(|line| line.kind).collect();
    assert!(kinds.contains(&DiffLineKind::NoNewline));
    let added = last.lines.iter().rfind(|line| line.kind == DiffLineKind::Add).unwrap();
    assert_eq!((added.new_no, added.text.as_str()), (Some(21), "line 21"));

    let crlf = find(&files, "crlf.txt", false);
    assert!(texts(crlf).contains(&(DiffLineKind::Add, "BETA\r".to_owned())));
}

#[tokio::test]
async fn diffs_cover_renames_deletions_binaries_and_unusual_paths() {
    // Windows filenames cannot hold a tab or a double quote, so it checks a path with a space instead.
    let quoted = if cfg!(windows) { "space q.txt" } else { "tab\t\"q\".txt" };
    let home = git_home();
    let mut repo = home.repo("repo");
    repo.write("docs/usage.md", "# Usage\n\nCall parse.\nMore.\nEven more.\n");
    repo.write("gone.txt", "bye\n");
    repo.write("logo.png", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR");
    repo.write(quoted, "q\n");
    repo.write("zażółć.txt", "z\n");
    repo.commit("base");

    repo.git(&["mv", "docs/usage.md", "docs/guide.md"]);
    repo.write("docs/guide.md", "# Guide\n\nCall parse.\nMore.\nEven more.\n");
    repo.git(&["add", "docs/guide.md"]);
    repo.git(&["rm", "-q", "gone.txt"]);
    repo.write("logo.png", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR2");
    repo.write(quoted, "q2\n");
    repo.write("zażółć.txt", "z2\n");

    let files = diff(&home, &repo, DiffScope::All).await;

    let renamed = find(&files, "docs/guide.md", true);
    assert_eq!(renamed.status, GitFileStatus::Renamed);
    assert_eq!(renamed.old_path.as_deref(), Some("docs/usage.md"));
    assert_eq!((renamed.additions, renamed.deletions), (1, 1));

    assert_eq!(find(&files, "gone.txt", true).status, GitFileStatus::Deleted);

    let binary = find(&files, "logo.png", false);
    assert!(binary.is_binary);
    assert!(binary.hunks.is_empty());

    assert_eq!(find(&files, quoted, false).additions, 1);
    assert_eq!(find(&files, "zażółć.txt", false).additions, 1);

    let staged_only = diff(&home, &repo, DiffScope::Staged).await;
    assert!(staged_only.iter().all(|file| file.staged));
    let filtered = git_diff(
        &home.env,
        &home.runner,
        &repo.path,
        &request(DiffScope::All, &["zażółć.txt"]),
    )
    .await
    .unwrap();
    assert_eq!(
        filtered.iter().map(|file| file.path.as_str()).collect::<Vec<_>>(),
        ["zażółć.txt"]
    );
}

#[tokio::test]
async fn untracked_files_are_read_safely() {
    let home = git_home();
    let mut repo = home.repo("repo");
    repo.write("README.md", "hi\n");
    repo.commit("base");

    repo.write("notes/todo.md", "# TODO\n\n- Benchmark");
    repo.write("blob.bin", b"ab\0cd");
    repo.write("big.txt", "x".repeat(600 * 1024));
    repo.write(".env", "TOKEN=secret-value\n");
    repo.write("empty.txt", "");

    let files = diff(&home, &repo, DiffScope::Untracked).await;
    assert!(
        files
            .iter()
            .all(|file| file.status == GitFileStatus::Untracked && !file.staged)
    );

    let todo = find(&files, "notes/todo.md", false);
    assert_eq!(todo.additions, 3);
    let hunk = &todo.hunks[0];
    assert_eq!(
        (hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines),
        (0, 0, 1, 3)
    );
    assert_eq!(hunk.lines.last().unwrap().kind, DiffLineKind::NoNewline);

    assert!(find(&files, "blob.bin", false).is_binary);

    for withheld in ["big.txt", ".env"] {
        let file = find(&files, withheld, false);
        assert!(file.truncated && file.hunks.is_empty(), "{withheld}");
    }
    assert!(!format!("{files:?}").contains("secret-value"));
    assert!(find(&files, "empty.txt", false).hunks.is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn untracked_symlinks_show_their_target_not_the_content() {
    let home = git_home();
    let mut repo = home.repo("repo");
    repo.write("README.md", "hi\n");
    repo.commit("base");

    let outside = home.home.join("outside.txt");
    fs::write(&outside, "outside-content\n").unwrap();
    std::os::unix::fs::symlink(&outside, repo.path.join("link")).unwrap();

    let files = diff(&home, &repo, DiffScope::Untracked).await;
    let link = find(&files, "link", false);

    assert_eq!(
        texts(link)[0],
        (DiffLineKind::Add, outside.to_string_lossy().into_owned())
    );
    assert!(!format!("{files:?}").contains("outside-content"));
}

#[tokio::test]
async fn detached_heads_and_unborn_branches_are_reported() {
    let home = git_home();
    let mut repo = home.repo("repo");
    repo.write("new.txt", "n\n");
    repo.git(&["add", "new.txt"]);

    let unborn = git_status(&home.env, &home.runner, &repo.path).await.unwrap();
    assert_eq!((unborn.branch.as_deref(), unborn.head.as_deref()), (Some("main"), None));
    let staged = diff(&home, &repo, DiffScope::Staged).await;
    assert_eq!(find(&staged, "new.txt", true).status, GitFileStatus::Added);

    repo.commit_staged("first");
    repo.write("new.txt", "n2\n");
    repo.commit("second");
    repo.git(&["checkout", "-q", "--detach", "HEAD~1"]);

    let detached = git_status(&home.env, &home.runner, &repo.path).await.unwrap();
    assert_eq!(detached.branch, None);
    assert!(detached.head.is_some());

    let branches = git_branches(&home.env, &home.runner, &repo.path).await.unwrap();
    assert!(branches.iter().all(|branch| !branch.is_current));
}

#[tokio::test]
async fn upstreams_report_ahead_and_behind() {
    let home = git_home();
    let remote = home.bare("origin.git");
    let mut repo = home.repo("repo");
    repo.write("a.txt", "1\n");
    repo.commit("one");
    repo.write("a.txt", "2\n");
    repo.commit("two");
    repo.git(&["remote", "add", "origin", &remote.to_string_lossy()]);
    repo.git(&["push", "-q", "-u", "origin", "main"]);
    repo.git(&["reset", "-q", "--hard", "HEAD~1"]);
    repo.write("b.txt", "b\n");
    repo.commit("three");
    repo.git(&["branch", "-q", "spike"]);

    let status = git_status(&home.env, &home.runner, &repo.path).await.unwrap();
    assert_eq!(status.upstream.as_deref(), Some("origin/main"));
    assert_eq!((status.ahead, status.behind), (1, 1));

    let branches = git_branches(&home.env, &home.runner, &repo.path).await.unwrap();
    let names: Vec<&str> = branches.iter().map(|branch| branch.name.as_str()).collect();
    assert_eq!(names, ["main", "spike", "origin/main"]);

    let main = &branches[0];
    assert!(main.is_current && !main.is_remote);
    assert_eq!(
        (main.upstream.as_deref(), main.ahead, main.behind),
        (Some("origin/main"), Some(1), Some(1))
    );
    let commit = main.last_commit.as_ref().unwrap();
    assert_eq!(
        (commit.summary.as_str(), commit.author.as_str()),
        ("three", "Demo User")
    );
    assert_eq!(commit.date, "2026-10-01T03:00:00Z");
    assert_eq!(Some(&commit.sha), status.head.as_ref());

    assert!(branches[2].is_remote);
    assert_eq!((branches[1].ahead, branches[1].upstream.as_deref()), (None, None));
}

#[tokio::test]
async fn subdirectory_projects_see_only_their_own_paths() {
    let home = git_home();
    let mut repo = home.repo("repo");
    repo.write("top.txt", "t\n");
    repo.write("sub/inner.txt", "i\n");
    repo.commit("base");
    repo.write("top.txt", "t2\n");
    repo.write("sub/inner.txt", "i2\n");
    repo.write("sub/new.txt", "n\n");

    let project = repo.path.join("sub");
    let status = git_status(&home.env, &home.runner, &project).await.unwrap();
    let paths: Vec<&str> = status.entries.iter().map(|entry| entry.path.as_str()).collect();
    assert_eq!(paths, ["inner.txt", "new.txt"]);

    let files = git_diff(&home.env, &home.runner, &project, &request(DiffScope::All, &[]))
        .await
        .unwrap();
    let paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
    assert_eq!(paths, ["inner.txt", "new.txt"]);
}

#[tokio::test]
async fn diff_paths_must_stay_inside_the_project() {
    let home = git_home();
    let repo = base_repo(&home);

    for path in ["../x", "/etc/passwd", "a\0b", ""] {
        let result = git_diff(&home.env, &home.runner, &repo.path, &request(DiffScope::All, &[path])).await;
        assert_eq!(result.unwrap_err().code(), IpcErrorCode::InvalidInput, "{path:?}");
    }
}
