//! The unified-diff parser on hand-written git output: edge cases a temporary repository cannot
//! produce cheaply (limits, combined diffs, odd headers).

use harness_business::git::{DiffBudget, parse_diff, parse_status, unquote};
use harness_business::model::{DiffLineKind, GitFileStatus, MAX_FILE_DIFF_BYTES};
use pretty_assertions::assert_eq;

fn parse(raw: &str) -> Vec<harness_business::model::FileDiff> {
    parse_diff(raw, false, &mut DiffBudget::default())
}

#[test]
fn header_like_lines_inside_a_hunk_stay_lines() {
    let raw = "diff --git a/x.md b/x.md\nindex 1..2 100644\n--- a/x.md\n+++ b/x.md\n@@ -1,2 +1,2 @@\n--- old rule\n+++ new rule\n \n";
    let files = parse(raw);

    let kinds: Vec<(DiffLineKind, &str)> = files[0].hunks[0]
        .lines
        .iter()
        .map(|line| (line.kind, line.text.as_str()))
        .collect();
    assert_eq!(
        kinds,
        [
            (DiffLineKind::Remove, "-- old rule"),
            (DiffLineKind::Add, "++ new rule"),
            (DiffLineKind::Context, ""),
        ]
    );
}

#[test]
fn blank_context_lines_without_a_space_count_as_context() {
    let raw = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1,3 +1,3 @@\n a\n\n-b\n+c\n";
    let lines = &parse(raw)[0].hunks[0].lines;

    assert_eq!(lines[1].kind, DiffLineKind::Context);
    assert_eq!((lines[1].old_no, lines[1].new_no), (Some(2), Some(2)));
    assert_eq!(lines[2].old_no, Some(3));
}

#[test]
fn quoted_and_spaced_paths_are_decoded() {
    let raw = concat!(
        "diff --git \"a/tab\\t\\\"q\\\".txt\" \"b/tab\\t\\\"q\\\".txt\"\n",
        "--- \"a/tab\\t\\\"q\\\".txt\"\n+++ \"b/tab\\t\\\"q\\\".txt\"\n@@ -1 +1 @@\n-q\n+r\n",
        "diff --git a/sp ace.txt b/sp ace2.txt\nsimilarity index 100%\nrename from sp ace.txt\nrename to sp ace2.txt\n",
        "diff --git a/sp ace.txt b/sp ace.txt\n--- a/sp ace.txt\t\n+++ b/sp ace.txt\t\n@@ -1 +1 @@\n-a\n+b\n",
        "diff --git \"a/\\303\\274.txt\" \"b/\\303\\274.txt\"\nnew file mode 100644\n",
    );
    let files = parse(raw);

    assert_eq!(files[0].path, "tab\t\"q\".txt");
    assert_eq!(files[1].path, "sp ace2.txt");
    assert_eq!(files[1].old_path.as_deref(), Some("sp ace.txt"));
    assert_eq!(files[1].status, GitFileStatus::Renamed);
    assert_eq!(files[2].path, "sp ace.txt");
    assert_eq!(files[3].path, "ü.txt");
    assert_eq!(files[3].status, GitFileStatus::Added);

    assert_eq!(unquote("\"a\\001b\" rest"), ("a\u{1}b".to_owned(), " rest"));
}

#[test]
fn binary_mode_only_and_combined_diffs_have_no_hunks() {
    let raw = concat!(
        "diff --git a/logo.png b/logo.png\nindex 1..2 100644\nBinary files a/logo.png and b/logo.png differ\n",
        "diff --git a/run.sh b/run.sh\nold mode 100644\nnew mode 100755\n",
        "diff --cc conflict.txt\nindex 1,2..3\n@@@ -1,1 -1,1 +1,5 @@@\n++<<<<<<< ours\n",
    );
    let files = parse(raw);

    assert!(files[0].is_binary && files[0].hunks.is_empty());
    assert_eq!((files[1].status, files[1].hunks.len()), (GitFileStatus::Modified, 0));
    assert_eq!(files[2].status, GitFileStatus::Conflicted);
    assert!(files[2].hunks.is_empty() && files[2].truncated);
}

#[test]
fn deny_listed_paths_keep_counts_but_not_content() {
    let raw = "diff --git a/.env b/.env\n--- a/.env\n+++ b/.env\n@@ -1 +1 @@\n-TOKEN=old-secret\n+TOKEN=new-secret\n";
    let file = &parse(raw)[0];

    assert!(file.truncated && file.hunks.is_empty());
    assert_eq!((file.additions, file.deletions), (1, 1));
}

#[test]
fn large_files_are_cut_and_the_total_budget_stops_later_files() {
    let line = "x".repeat(1023);
    let count = MAX_FILE_DIFF_BYTES / 1024 + 10;
    let body: String = (0..count).map(|_| format!("+{line}\n")).collect();
    let big = format!("diff --git a/big b/big\n--- a/big\n+++ b/big\n@@ -0,0 +1,{count} @@\n{body}");
    let small = "diff --git a/small b/small\n--- a/small\n+++ b/small\n@@ -1 +1 @@\n-a\n+b\n";

    let files = parse(&format!("{big}{small}"));
    let kept: usize = files[0].hunks.iter().map(|hunk| hunk.lines.len()).sum();
    assert!(files[0].truncated);
    assert_eq!(files[0].additions as usize, count, "counts cover the whole diff");
    assert!(kept < count && kept > 0);
    assert!(!files[1].truncated);

    let mut budget = DiffBudget::default();
    for _ in 0..10 {
        parse_diff(&big, false, &mut budget);
    }
    assert!(budget.exhausted());
    let late = parse_diff(small, false, &mut budget);
    assert!(late[0].truncated && late[0].hunks.is_empty());
}

#[test]
fn status_records_with_spaces_renames_and_conflicts_parse() {
    let raw = concat!(
        "# branch.oid 4e1d2c7a9b\0# branch.head feat/x\0# branch.upstream origin/feat/x\0# branch.ab +2 -1\0",
        "1 .M N... 100644 100644 100644 aaa bbb sub/with space.txt\0",
        "2 R. N... 100644 100644 100644 aaa bbb R100 sub/new name.md\0sub/old name.md\0",
        "u UU N... 100644 100644 100644 100644 aaa bbb ccc sub/conflict.txt\0",
        "? other/outside.txt\0? sub/untracked.txt\0",
    );
    let status = parse_status(raw, "sub/");

    assert_eq!(status.head.as_deref(), Some("4e1d2c7"));
    assert_eq!(status.branch.as_deref(), Some("feat/x"));
    assert_eq!((status.ahead, status.behind), (2, 1));

    let paths: Vec<(&str, Option<&str>)> = status
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry.orig_path.as_deref()))
        .collect();
    assert_eq!(
        paths,
        [
            ("conflict.txt", None),
            ("new name.md", Some("old name.md")),
            ("untracked.txt", None),
            ("with space.txt", None),
        ]
    );
    assert_eq!(status.entries[0].staged, Some(GitFileStatus::Conflicted));
}
