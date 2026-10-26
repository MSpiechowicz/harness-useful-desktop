//! Git commands for the Changes view: status, branches and working-tree diffs of a project, and
//! discarding changes in it.
//!
//! The project must be the active project or a recent one. git runs through the business layer's
//! hardened runner (no shell, time and output limits, repository programs and inherited `GIT_*`
//! variables switched off); a directory outside any repository reports `isRepo: false`.

use harness_business::git;
use harness_business::model::{Branch, DiffRequest, FileDiff, GitStatus};
use tauri::State;
use tauri::async_runtime::block_on;

use super::{authorized_dir, blocking};
use crate::error::CommandResult;
use crate::state::AppState;

/// Branch, upstream, ahead/behind and changed paths of `project`'s repository.
#[tauri::command]
pub async fn git_status(state: State<'_, AppState>, project: String) -> CommandResult<GitStatus> {
    status(&state, project).await
}

/// Local branches, then remote ones (at most 200), with their last commits.
#[tauri::command]
pub async fn git_branches(state: State<'_, AppState>, project: String) -> CommandResult<Vec<Branch>> {
    branches(&state, project).await
}

/// The staged, unstaged and/or untracked changes `request` selects, limited to its
/// project-relative `paths` when given.
#[tauri::command]
pub async fn git_diff(
    state: State<'_, AppState>,
    project: String,
    request: DiffRequest,
) -> CommandResult<Vec<FileDiff>> {
    diff(&state, project, request).await
}

/// The lines of one changed file (project-relative `path`), from the index when `staged`, so the diff
/// view can expand the unchanged lines between hunks. Deny-listed, binary and oversized files are refused.
#[tauri::command]
pub async fn git_file_lines(
    state: State<'_, AppState>,
    project: String,
    path: String,
    staged: bool,
) -> CommandResult<Vec<String>> {
    file_lines(&state, project, path, staged).await
}

/// Reverts every staged and unstaged change in `project` to HEAD and deletes its untracked files
/// (ignored ones stay). The UI asks the user to confirm first.
#[tauri::command]
pub async fn git_discard_all(state: State<'_, AppState>, project: String) -> CommandResult<()> {
    discard_all(&state, project).await
}

/// Reverts the project-relative `paths` of one changed file (the file and, for a rename, its old
/// name) to HEAD, deleting an untracked or newly staged one. The UI asks the user to confirm first.
#[tauri::command]
pub async fn git_discard_files(state: State<'_, AppState>, project: String, paths: Vec<String>) -> CommandResult<()> {
    discard_files(&state, project, paths).await
}

async fn status(state: &AppState, project: String) -> CommandResult<GitStatus> {
    let project = authorized_dir(state, project).await?;

    Ok(git::git_status(&state.env, &state.runner, &project).await?)
}

async fn branches(state: &AppState, project: String) -> CommandResult<Vec<Branch>> {
    let project = authorized_dir(state, project).await?;

    Ok(git::git_branches(&state.env, &state.runner, &project).await?)
}

async fn discard_all(state: &AppState, project: String) -> CommandResult<()> {
    let project = authorized_dir(state, project).await?;

    Ok(git::git_discard_all(&state.env, &state.runner, &project).await?)
}

/// Runs in a blocking task: besides running git, a diff reads the untracked files itself.
async fn diff(state: &AppState, project: String, request: DiffRequest) -> CommandResult<Vec<FileDiff>> {
    let project = authorized_dir(state, project).await?;
    let env = state.env.clone();
    let runner = state.runner;

    blocking(move || Ok(block_on(git::git_diff(&env, &runner, &project, &request))?)).await
}

async fn discard_files(state: &AppState, project: String, paths: Vec<String>) -> CommandResult<()> {
    let project = authorized_dir(state, project).await?;

    Ok(git::git_discard_files(&state.env, &state.runner, &project, &paths).await?)
}

/// Runs in a blocking task: the working-tree side is read from disk.
async fn file_lines(state: &AppState, project: String, path: String, staged: bool) -> CommandResult<Vec<String>> {
    let project = authorized_dir(state, project).await?;
    let env = state.env.clone();
    let runner = state.runner;

    blocking(move || Ok(block_on(git::git_file_lines(&env, &runner, &project, &path, staged))?)).await
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use harness_business::model::{DiffScope, GitFileStatus, IpcErrorCode};

    use super::*;
    use crate::test_support::{TempDir, open_git_project, setup_git, stranger_beside};

    struct Setup {
        state: AppState,
        project: String,
        _dir: TempDir,
    }

    /// An opened project that is a repository on `main` with one committed, then modified file
    /// and one untracked file.
    fn setup() -> Setup {
        let dir = TempDir::new("git");
        let home = dir.path().join("home");
        let project = home.join("demo");
        std::fs::create_dir_all(&project).expect("create project");

        git(&home, &project, &["init", "-q", "-b", "main"]);
        std::fs::write(project.join("code.txt"), "one\n").expect("write");
        git(&home, &project, &["add", "code.txt"]);
        git(&home, &project, &["commit", "-q", "-m", "base"]);
        std::fs::write(project.join("code.txt"), "two\n").expect("write");
        std::fs::write(project.join("new.txt"), "new\n").expect("write");

        let (state, project) = open_git_project(&dir, &home, &project);

        Setup {
            state,
            project,
            _dir: dir,
        }
    }

    impl Setup {
        /// The project directory.
        fn dir(&self) -> &Path {
            Path::new(&self.project)
        }

        /// The empty home directory the project sits in.
        fn home(&self) -> &Path {
            self.dir().parent().expect("project has a parent")
        }
    }

    /// A file's text with Windows line endings read as `\n`: Git there may check files out with CRLF.
    fn read_lf(path: &Path) -> String {
        std::fs::read_to_string(path).expect("read").replace("\r\n", "\n")
    }

    fn git(home: &Path, dir: &Path, args: &[&str]) {
        setup_git(home, dir, args);
    }

    fn request(scope: DiffScope, paths: &[&str]) -> DiffRequest {
        DiffRequest {
            scope,
            paths: paths.iter().map(|&path| path.to_owned()).collect(),
        }
    }

    #[test]
    fn a_projects_repository_is_reported() {
        let setup = setup();
        let (state, project) = (&setup.state, &setup.project);

        let status = block_on(status(state, project.clone())).expect("status");
        let changes: Vec<(&str, Option<GitFileStatus>)> = status
            .entries
            .iter()
            .map(|entry| (entry.path.as_str(), entry.unstaged))
            .collect();
        assert_eq!((status.is_repo, status.branch.as_deref()), (true, Some("main")));
        assert_eq!(
            changes,
            [
                ("code.txt", Some(GitFileStatus::Modified)),
                ("new.txt", Some(GitFileStatus::Untracked))
            ]
        );

        let branches = block_on(branches(state, project.clone())).expect("branches");
        assert_eq!(branches.len(), 1);
        assert!(branches[0].is_current);

        let files = block_on(diff(state, project.clone(), request(DiffScope::All, &[]))).expect("diff");
        let summary: Vec<(&str, GitFileStatus, u32, u32)> = files
            .iter()
            .map(|file| (file.path.as_str(), file.status, file.additions, file.deletions))
            .collect();
        assert_eq!(
            summary,
            [
                ("code.txt", GitFileStatus::Modified, 1, 1),
                ("new.txt", GitFileStatus::Untracked, 1, 0)
            ]
        );

        let only = block_on(diff(state, project.clone(), request(DiffScope::Unstaged, &["new.txt"]))).expect("diff");
        assert!(only.is_empty());
    }

    #[test]
    fn unknown_projects_and_escaping_paths_are_refused() {
        let setup = setup();
        let state = &setup.state;
        let stranger = stranger_beside(&setup.project);

        let refused = [
            block_on(status(state, stranger.clone())).map(drop),
            block_on(branches(state, stranger.clone())).map(drop),
            block_on(diff(state, stranger.clone(), request(DiffScope::All, &[]))).map(drop),
            block_on(discard_all(state, stranger)),
        ];
        for error in refused {
            assert_eq!(error.expect_err("unknown project").code(), IpcErrorCode::Forbidden);
        }

        for path in ["../stranger", "/etc/passwd"] {
            let error = block_on(diff(state, setup.project.clone(), request(DiffScope::All, &[path]))).expect_err(path);
            assert_eq!(error.code(), IpcErrorCode::InvalidInput, "{path}");
        }
    }

    #[test]
    fn a_changed_files_lines_come_from_the_working_tree_or_the_index() {
        let setup = setup();
        let (state, project) = (&setup.state, &setup.project);

        let working =
            block_on(file_lines(state, project.clone(), "code.txt".to_owned(), false)).expect("working lines");
        assert_eq!(working, ["two"]);

        // The index still holds the committed text.
        let index = block_on(file_lines(state, project.clone(), "code.txt".to_owned(), true)).expect("index lines");
        assert_eq!(index, ["one"]);
    }

    #[test]
    fn discarding_all_reverts_staged_and_unstaged_edits_and_deletes_untracked_files() {
        let setup = setup();
        let (state, project, dir, home) = (&setup.state, &setup.project, setup.dir(), setup.home());
        std::fs::write(dir.join(".gitignore"), "*.log\n").expect("write");
        std::fs::write(dir.join("kept.log"), "ignored\n").expect("write");
        std::fs::write(dir.join("added.txt"), "staged\n").expect("write");
        git(home, dir, &["add", ".gitignore"]);
        git(home, dir, &["commit", "-q", "-m", "ignore"]);
        git(home, dir, &["add", "added.txt"]);
        std::fs::create_dir_all(dir.join("scratch")).expect("create");
        std::fs::write(dir.join("scratch/notes.txt"), "draft\n").expect("write");

        block_on(discard_all(state, project.clone())).expect("discard");

        let status = block_on(status(state, project.clone())).expect("status");
        assert!(status.entries.is_empty(), "{:?}", status.entries);
        assert_eq!(read_lf(&dir.join("code.txt")), "one\n");
        assert!(!dir.join("added.txt").exists() && !dir.join("scratch").exists() && !dir.join("new.txt").exists());
        assert!(dir.join("kept.log").exists(), "ignored files stay");
    }

    #[test]
    fn discarding_files_touches_only_the_named_ones() {
        let setup = setup();
        let (state, project, dir, home) = (&setup.state, &setup.project, setup.dir(), setup.home());
        std::fs::write(dir.join("other.txt"), "keep\n").expect("write");
        std::fs::write(dir.join("added.txt"), "staged\n").expect("write");
        git(home, dir, &["add", "added.txt", "code.txt"]);

        let discard = |paths: &[&str]| {
            block_on(discard_files(
                state,
                project.clone(),
                paths.iter().map(|path| (*path).to_owned()).collect(),
            ))
        };
        discard(&["code.txt"]).expect("tracked");
        discard(&["added.txt", "new.txt"]).expect("staged new and untracked");

        assert_eq!(read_lf(&dir.join("code.txt")), "one\n");
        assert!(!dir.join("added.txt").exists() && !dir.join("new.txt").exists());
        assert!(dir.join("other.txt").exists(), "other files stay");
        let status = block_on(status(state, project.clone())).expect("status");
        assert_eq!(status.entries.len(), 1, "{:?}", status.entries);

        assert_eq!(discard(&[]).expect_err("empty").code(), IpcErrorCode::InvalidInput);
        assert_eq!(
            discard(&["../x"]).expect_err("escape").code(),
            IpcErrorCode::InvalidInput
        );
    }

    #[test]
    fn discarding_all_in_a_subdirectory_project_leaves_the_rest_of_the_repository() {
        let dir = TempDir::new("git-sub");
        let home = dir.path().join("home");
        let repo = home.join("repo");
        let project = repo.join("app");
        std::fs::create_dir_all(&project).expect("create project");

        git(&home, &repo, &["init", "-q", "-b", "main"]);
        std::fs::write(repo.join("outside.txt"), "one\n").expect("write");
        std::fs::write(project.join("inside.txt"), "one\n").expect("write");
        git(&home, &repo, &["add", "."]);
        git(&home, &repo, &["commit", "-q", "-m", "base"]);
        std::fs::write(repo.join("outside.txt"), "two\n").expect("write");
        std::fs::write(repo.join("stray.txt"), "new\n").expect("write");
        std::fs::write(project.join("inside.txt"), "two\n").expect("write");

        let (state, project_arg) = open_git_project(&dir, &home, &project);

        block_on(discard_all(&state, project_arg)).expect("discard");

        assert_eq!(read_lf(&project.join("inside.txt")), "one\n");
        assert_eq!(read_lf(&repo.join("outside.txt")), "two\n");
        assert!(repo.join("stray.txt").exists());
    }

    #[test]
    fn file_lines_refuse_secrets_and_escaping_paths() {
        let setup = setup();
        let (state, project) = (&setup.state, &setup.project);
        std::fs::write(Path::new(project).join(".env"), "TOKEN=1\n").expect("write");

        let secret = block_on(file_lines(state, project.clone(), ".env".to_owned(), false)).expect_err("secret");
        assert_eq!(secret.code(), IpcErrorCode::Forbidden);

        let escaping = block_on(file_lines(state, project.clone(), "../x".to_owned(), false)).expect_err("escape");
        assert_eq!(escaping.code(), IpcErrorCode::InvalidInput);

        let stranger = Path::new(project).parent().unwrap().join("stranger");
        std::fs::create_dir_all(&stranger).expect("create");
        let unknown = block_on(file_lines(
            state,
            stranger.to_string_lossy().into_owned(),
            "a".to_owned(),
            false,
        ))
        .expect_err("unknown project");
        assert_eq!(unknown.code(), IpcErrorCode::Forbidden);
    }
}
