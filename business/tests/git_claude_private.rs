//! Claude Code's private files in a repository's diffs: its global file `.claude.json` (a home
//! dotfiles repository) and the private files of a config directory `CLAUDE_CONFIG_DIR` names
//! inside the project are listed without content, while project folders that only share a name
//! with Claude's per-session stores are shown.

mod git_support;

use git_support::git_home;
use harness_business::BusinessError;
use harness_business::git::{git_diff, git_file_lines};
use harness_business::model::{DiffRequest, DiffScope, FileDiff};

const SECRET: &str = "oauth-secret-value";

fn request(scope: DiffScope) -> DiffRequest {
    DiffRequest {
        scope,
        paths: Vec::new(),
    }
}

fn find<'a>(files: &'a [FileDiff], path: &str, staged: bool) -> &'a FileDiff {
    files
        .iter()
        .find(|file| file.path == path && file.staged == staged)
        .unwrap_or_else(|| panic!("{path} (staged: {staged}) missing in {files:?}"))
}

#[tokio::test]
async fn an_untracked_global_file_is_withheld_and_a_project_file_history_folder_is_shown() {
    let home = git_home();
    let mut repo = home.repo("dotfiles");
    repo.write("README.md", "hi\n");
    repo.commit("base");

    repo.write(".claude.json", format!("{{\"oauthAccount\": \"{SECRET}\"}}\n"));
    repo.write(".claude.json.corrupted.1760000000000", format!("{SECRET}\n"));
    repo.write("src/file-history/store.ts", "export const store = 1;\n");

    let files = git_diff(&home.env, &home.runner, &repo.path, &request(DiffScope::Untracked))
        .await
        .unwrap();

    for withheld in [".claude.json", ".claude.json.corrupted.1760000000000"] {
        let file = find(&files, withheld, false);
        assert!(file.truncated && file.hunks.is_empty(), "{withheld}");
    }
    assert!(!format!("{files:?}").contains(SECRET));

    let shown = find(&files, "src/file-history/store.ts", false);
    assert_eq!(shown.additions, 1);
}

#[tokio::test]
async fn a_config_directory_inside_the_project_keeps_its_private_files_out_of_diffs() {
    let home = git_home();
    let mut repo = home.repo("dotfiles");
    repo.write("claude-work/history.jsonl", "{\"display\": \"first\"}\n");
    repo.write("claude-work/settings.json", "{}\n");
    repo.commit("base");

    let env = home
        .env
        .clone()
        .with_var("CLAUDE_CONFIG_DIR", repo.path.join("claude-work").to_string_lossy());

    repo.write("claude-work/history.jsonl", format!("{{\"display\": \"{SECRET}\"}}\n"));
    repo.write("claude-work/settings.json", "{\"model\": \"opus\"}\n");
    repo.git(&["add", "claude-work/history.jsonl"]);

    let files = git_diff(&env, &home.runner, &repo.path, &request(DiffScope::All))
        .await
        .unwrap();

    let staged = find(&files, "claude-work/history.jsonl", true);
    assert!(staged.truncated && staged.hunks.is_empty());
    assert!(!format!("{files:?}").contains(SECRET));
    assert!(!find(&files, "claude-work/settings.json", false).hunks.is_empty());

    for staged in [true, false] {
        let lines = git_file_lines(&env, &home.runner, &repo.path, "claude-work/history.jsonl", staged).await;
        assert!(matches!(lines, Err(BusinessError::Forbidden(_))), "staged: {staged}");
    }
}
