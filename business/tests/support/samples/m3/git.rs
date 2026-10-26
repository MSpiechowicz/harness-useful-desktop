//! Repository fixtures (`git/`): real `git_status`, `git_branches`, `git_sync_state` and `git_diff` output over a
//! temporary copy of the demo project on a feature branch with a modified, an added, a renamed,
//! a binary and an untracked file. Identities and dates are fixed, so the shas are stable.

use std::sync::OnceLock;

use harness_business::git::{git_branches, git_diff, git_status, git_sync_state};
use harness_business::model::{DiffRequest, DiffScope};
use serde_json::Value;

use crate::common::exported;
use crate::git_support::{GitHome, Repo, git_home};

const BRANCH: &str = "feat/empty-input";

/// `src/lib.rs` as released: the parser without a final newline.
const LIB_RELEASED: &str = "//! Line-oriented record parser.

pub fn parse(input: &str) -> Result<Vec<Record>, String> {
    let mut records = Vec::new();
    for line in input.lines() {
        records.push(Record::from_line(line)?);
    }
    Ok(records)
}

pub struct Record {
    fields: Vec<String>,
}

impl Record {
    fn from_line(line: &str) -> Result<Self, String> {
        let fields = line.split(',').map(str::to_owned).collect();
        Ok(Self { fields })
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}";

/// `src/lib.rs` in the working tree: the error type, the new signatures and a test.
fn lib_changed() -> String {
    let header = "//! Line-oriented record parser.\n";
    let body = LIB_RELEASED
        .strip_prefix(header)
        .expect("released header")
        .replace(", String>", ", Error>");

    format!("{header}\nmod error;\n\npub use error::Error;\n{body}\n{TESTS}")
}

/// The test module appended to `src/lib.rs`.
const TESTS: &str = "
#[cfg(test)]
mod tests {
    #[test]
    fn parses_empty_input() {
        assert!(super::parse(\"\").unwrap().is_empty());
    }
}
";

const ERROR_RS: &str = "/// Errors returned by [`crate::parse`].
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// A line is not a valid record.
    InvalidLine { line: usize },
}
";

const USAGE_MD: &str = "# Usage\n\nCall `demo::parse` with the file contents.\nEach line is one record.\n";

/// Builds the demo repository: `main` released and pushed, a spike branch, and the feature
/// branch 2 commits ahead of and 1 behind its upstream, with uncommitted changes.
fn demo_repo(home: &GitHome) -> Repo {
    let remote = home.bare("origin.git");
    let mut repo = home.repo("demo");

    repo.write("src/lib.rs", LIB_RELEASED);
    repo.write("README.md", "# demo\n\nParses records.\n");
    repo.write("docs/usage.md", USAGE_MD);
    repo.write("assets/logo.png", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x10");
    repo.git(&["add", "-A"]);
    repo.commit_as("Release 0.3.1", "Alex Reviewer");
    repo.git(&["remote", "add", "origin", &remote.to_string_lossy()]);
    repo.git(&["push", "-q", "-u", "origin", "main"]);

    repo.git(&["checkout", "-q", "-b", "spike/streaming-parser"]);
    repo.write("src/stream.rs", "// WIP\n");
    repo.commit("WIP: streaming parser");

    repo.git(&["checkout", "-q", "-b", BRANCH, "main"]);
    repo.write("docs/format.md", "# Record format\n");
    repo.git(&["add", "-A"]);
    repo.commit_as("Document the record format", "Alex Reviewer");
    repo.git(&["push", "-q", "-u", "origin", BRANCH]);
    repo.git(&["reset", "-q", "--hard", "main"]);

    repo.write("CHANGELOG.md", "# Changelog\n");
    repo.commit("Start the changelog");
    repo.write("README.md", "# demo\n\nParses records, one per line.\n");
    repo.commit("Return an empty list for empty input");

    repo.write("src/lib.rs", lib_changed());
    repo.write("src/error.rs", ERROR_RS);
    repo.git(&["add", "src/error.rs"]);
    repo.git(&["mv", "docs/usage.md", "docs/guide.md"]);
    repo.write("docs/guide.md", USAGE_MD.replace("# Usage", "# Guide"));
    repo.git(&["add", "docs/guide.md"]);
    repo.write("assets/logo.png", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x20");
    repo.write("notes/todo.md", "# TODO\n\n- Benchmark `parse` on 100 MB inputs.\n");

    repo
}

/// The `git/` fixtures; computed once per test binary.
pub fn fixtures() -> Vec<(&'static str, Value)> {
    static FIXTURES: OnceLock<Vec<(&'static str, Value)>> = OnceLock::new();

    FIXTURES.get_or_init(export).clone()
}

fn export() -> Vec<(&'static str, Value)> {
    let home = git_home();
    let repo = demo_repo(&home);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    let (env, runner, project) = (&home.env, &home.runner, repo.path.as_path());
    let request = DiffRequest {
        scope: DiffScope::All,
        paths: Vec::new(),
    };

    let (status, branches, sync, diff) = runtime.block_on(async {
        (
            git_status(env, runner, project).await.expect("git status"),
            git_branches(env, runner, project).await.expect("git branches"),
            git_sync_state(env, runner, project, None)
                .await
                .expect("git sync state"),
            git_diff(env, runner, project, &request).await.expect("git diff"),
        )
    });

    vec![
        ("git/git_status.json", exported(status, &home.home, project)),
        ("git/git_branches.json", exported(branches, &home.home, project)),
        ("git/git_sync_state.json", exported(sync, &home.home, project)),
        ("git/git_diff.json", exported(diff, &home.home, project)),
    ]
}
