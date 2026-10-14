//! The text of one changed file, so the diff view can show the unchanged lines between two hunks.
//!
//! The staged side is the blob in the index, the working side the file on disk, read the same way
//! untracked files are: only inside the project, never a deny-listed file, never past
//! [`MAX_UNTRACKED_FILE_BYTES`], never a binary one.

use std::fs;
use std::path::Path;

use super::untracked::read_contained;
use super::{GitCli, validate_paths};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::MAX_UNTRACKED_FILE_BYTES;
use crate::process::ProcessRunner;
use crate::security::is_denied;

/// Bytes inspected for a NUL byte.
const BINARY_SNIFF_BYTES: usize = 8 * 1024;

/// The lines of project-relative `path`, from the index when `staged`, else from the working tree.
/// A trailing newline does not make an extra empty line; a `\r` of CRLF content is kept.
pub async fn git_file_lines(
    env: &Env,
    runner: &dyn ProcessRunner,
    project: &Path,
    path: &str,
    staged: bool,
) -> Result<Vec<String>> {
    validate_paths(&[path.to_owned()])?;

    if is_denied(Path::new(path)).is_some() {
        return Err(BusinessError::Forbidden("this file is not shown".to_owned()));
    }

    let content = if staged {
        staged_content(env, runner, project, path).await?
    } else {
        worktree_content(project, path)?
    };

    lines_of(&content)
}

async fn staged_content(env: &Env, runner: &dyn ProcessRunner, project: &Path, path: &str) -> Result<Vec<u8>> {
    let git = GitCli::new(env, runner, project)?;
    let output = git.run_ok(["show".to_owned(), format!(":./{path}")]).await?;

    if output.truncated || output.stdout.len() as u64 > MAX_UNTRACKED_FILE_BYTES {
        return Err(too_large());
    }

    Ok(output.stdout.into_bytes())
}

fn worktree_content(project: &Path, path: &str) -> Result<Vec<u8>> {
    let full = project.join(path);
    let metadata = fs::symlink_metadata(&full).map_err(|_| BusinessError::NotFound("the file is gone".to_owned()))?;

    if !metadata.is_file() {
        return Err(BusinessError::InvalidInput("not a regular file".to_owned()));
    }
    if metadata.len() > MAX_UNTRACKED_FILE_BYTES {
        return Err(too_large());
    }

    read_contained(project, &full).ok_or_else(|| BusinessError::Forbidden("this file is not shown".to_owned()))
}

fn lines_of(content: &[u8]) -> Result<Vec<String>> {
    if content[..content.len().min(BINARY_SNIFF_BYTES)].contains(&0) {
        return Err(BusinessError::InvalidInput("a binary file has no lines".to_owned()));
    }

    let text = String::from_utf8_lossy(content);
    let body = text.strip_suffix('\n').unwrap_or(&text);

    if body.is_empty() {
        return Ok(Vec::new());
    }

    Ok(body.split('\n').map(str::to_owned).collect())
}

fn too_large() -> BusinessError {
    BusinessError::InvalidInput(format!(
        "the file is larger than {} KiB",
        MAX_UNTRACKED_FILE_BYTES / 1024
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_lines_without_a_phantom_last_one() {
        assert_eq!(lines_of(b"one\ntwo\n").unwrap(), ["one", "two"]);
        assert_eq!(lines_of(b"one\ntwo").unwrap(), ["one", "two"]);
        assert_eq!(lines_of(b"a\r\nb\r\n").unwrap(), ["a\r", "b\r"]);
        assert_eq!(lines_of(b"").unwrap(), Vec::<String>::new());
        assert_eq!(lines_of(b"\n").unwrap(), Vec::<String>::new());
    }

    #[test]
    fn a_binary_file_has_no_lines() {
        assert!(matches!(lines_of(b"ab\0cd"), Err(BusinessError::InvalidInput(_))));
    }
}
