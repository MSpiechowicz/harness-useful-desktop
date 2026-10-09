//! Synthetic repository fixtures (`git/`): the demo project on a feature branch with a modified,
//! an added, a renamed, a binary and an untracked file.

use harness_business::model::{
    Branch, CommitInfo, DiffLine, DiffLineKind, FileDiff, GitFileStatus, GitStatus, Hunk, StatusEntry,
};
use serde_json::Value;

use super::to_json;

const BRANCH: &str = "feat/empty-input";
const UPSTREAM: &str = "origin/feat/empty-input";

fn entry(path: &str, staged: Option<GitFileStatus>, unstaged: Option<GitFileStatus>) -> StatusEntry {
    StatusEntry {
        path: path.to_owned(),
        orig_path: None,
        staged,
        unstaged,
    }
}

fn status() -> GitStatus {
    use GitFileStatus::{Added, Modified, Renamed, Untracked};

    let renamed = StatusEntry {
        orig_path: Some("docs/usage.md".to_owned()),
        ..entry("docs/guide.md", Some(Renamed), None)
    };

    GitStatus {
        is_repo: true,
        branch: Some(BRANCH.to_owned()),
        head: Some("4e1d2c7".to_owned()),
        upstream: Some(UPSTREAM.to_owned()),
        ahead: 2,
        behind: 1,
        entries: vec![
            entry("assets/logo.png", None, Some(Modified)),
            renamed,
            entry("notes/todo.md", None, Some(Untracked)),
            entry("src/error.rs", Some(Added), None),
            entry("src/lib.rs", None, Some(Modified)),
        ],
    }
}

fn commit(sha: &str, summary: &str, author: &str, date: &str) -> CommitInfo {
    CommitInfo {
        sha: sha.to_owned(),
        summary: summary.to_owned(),
        author: author.to_owned(),
        date: date.to_owned(),
    }
}

fn branch(name: &str, last_commit: CommitInfo) -> Branch {
    Branch {
        name: name.to_owned(),
        is_current: false,
        is_remote: name.starts_with("origin/"),
        upstream: None,
        ahead: None,
        behind: None,
        last_commit: Some(last_commit),
    }
}

/// `branch` tracking `upstream`.
fn tracking(branch: Branch, upstream: &str, ahead: u32, behind: u32) -> Branch {
    Branch {
        upstream: Some(upstream.to_owned()),
        ahead: Some(ahead),
        behind: Some(behind),
        ..branch
    }
}

/// Local branches first, then remote ones; each group by last commit date, newest first.
fn branches() -> Vec<Branch> {
    let head = commit(
        "4e1d2c7",
        "Return an empty list for empty input",
        "Demo User",
        "2026-10-09T10:12:44Z",
    );
    let remote = commit(
        "c7a2e95",
        "Document the record format",
        "Alex Reviewer",
        "2026-10-09T08:05:19Z",
    );
    let main = commit("1f8d6b3", "Release 0.3.1", "Alex Reviewer", "2026-10-06T12:00:00Z");
    let spike = commit("5d0c4e8", "WIP: streaming parser", "Demo User", "2026-09-30T19:44:10Z");

    let current = Branch {
        is_current: true,
        ..tracking(branch(BRANCH, head), UPSTREAM, 2, 1)
    };

    vec![
        current,
        tracking(branch("main", main.clone()), "origin/main", 0, 0),
        branch("spike/streaming-parser", spike),
        branch(UPSTREAM, remote),
        branch("origin/main", main),
    ]
}

/// A hunk from unified-diff body lines (` ` context, `+` add, `-` remove, `\` no newline);
/// counts, header and line numbers are derived from the starts.
fn hunk(old_start: u32, new_start: u32, section: &str, body: &[&str]) -> Hunk {
    let (mut old_no, mut new_no) = (old_start, new_start);
    let mut lines = Vec::new();

    for line in body {
        let (prefix, text) = line.split_at(1);
        let (kind, old, new) = match prefix {
            " " => (DiffLineKind::Context, Some(old_no), Some(new_no)),
            "+" => (DiffLineKind::Add, None, Some(new_no)),
            "-" => (DiffLineKind::Remove, Some(old_no), None),
            _ => (DiffLineKind::NoNewline, None, None),
        };

        old_no += u32::from(old.is_some());
        new_no += u32::from(new.is_some());
        lines.push(DiffLine {
            kind,
            old_no: old,
            new_no: new,
            text: if kind == DiffLineKind::NoNewline {
                String::new()
            } else {
                text.to_owned()
            },
        });
    }

    let (old_lines, new_lines) = (old_no - old_start, new_no - new_start);
    let header = format!("@@ -{old_start},{old_lines} +{new_start},{new_lines} @@{section}");

    Hunk {
        header,
        old_start,
        old_lines,
        new_start,
        new_lines,
        lines,
    }
}

fn count(hunks: &[Hunk], kind: DiffLineKind) -> u32 {
    hunks
        .iter()
        .flat_map(|hunk| &hunk.lines)
        .filter(|line| line.kind == kind)
        .count() as u32
}

fn file(path: &str, status: GitFileStatus, staged: bool, hunks: Vec<Hunk>) -> FileDiff {
    FileDiff {
        path: path.to_owned(),
        old_path: None,
        status,
        staged,
        is_binary: false,
        additions: count(&hunks, DiffLineKind::Add),
        deletions: count(&hunks, DiffLineKind::Remove),
        hunks,
        truncated: false,
    }
}

/// `src/lib.rs`: an unstaged change with two hunks; the second fixes a missing final newline.
fn modified() -> FileDiff {
    let imports = hunk(
        1,
        1,
        "",
        &[
            " //! Line-oriented record parser.",
            " ",
            "+mod error;",
            "+",
            "+pub use error::Error;",
            "+",
            "-pub fn parse(input: &str) -> Result<Vec<Record>, String> {",
            "+pub fn parse(input: &str) -> Result<Vec<Record>, Error> {",
            "     let mut records = Vec::new();",
            "     for line in input.lines() {",
            "         records.push(Record::from_line(line)?);",
        ],
    );
    let tests = hunk(
        20,
        24,
        " impl Record {",
        &[
            "     pub fn is_empty(&self) -> bool {",
            "         self.fields.is_empty()",
            "     }",
            "-}",
            "\\ No newline at end of file",
            "+}",
            "+",
            "+#[cfg(test)]",
            "+mod tests {",
            "+    #[test]",
            "+    fn parses_empty_input() {",
            "+        assert!(super::parse(\"\").unwrap().is_empty());",
            "+    }",
            "+}",
        ],
    );

    file("src/lib.rs", GitFileStatus::Modified, false, vec![imports, tests])
}

fn diff() -> Vec<FileDiff> {
    let added = hunk(
        0,
        1,
        "",
        &[
            "+/// Errors returned by [`crate::parse`].",
            "+#[derive(Debug, PartialEq, Eq)]",
            "+pub enum Error {",
            "+    /// A line is not a valid record.",
            "+    InvalidLine { line: usize },",
            "+}",
        ],
    );
    let title = hunk(
        1,
        1,
        "",
        &[
            "-# Usage",
            "+# Guide",
            " ",
            " Call `demo::parse` with the file contents.",
        ],
    );
    let todo = hunk(0, 1, "", &["+# TODO", "+", "+- Benchmark `parse` on 100 MB inputs."]);

    let renamed = FileDiff {
        old_path: Some("docs/usage.md".to_owned()),
        ..file("docs/guide.md", GitFileStatus::Renamed, true, vec![title])
    };
    let binary = FileDiff {
        is_binary: true,
        ..file("assets/logo.png", GitFileStatus::Modified, false, Vec::new())
    };

    vec![
        modified(),
        file("src/error.rs", GitFileStatus::Added, true, vec![added]),
        renamed,
        binary,
        file("notes/todo.md", GitFileStatus::Untracked, false, vec![todo]),
    ]
}

/// The `git/` fixtures.
pub fn fixtures() -> Vec<(&'static str, Value)> {
    vec![
        ("git/git_status.json", to_json(status())),
        ("git/git_branches.json", to_json(branches())),
        ("git/git_diff.json", to_json(diff())),
    ]
}
