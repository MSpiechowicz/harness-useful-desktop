//! The checks of what goes to `harness-knowledge docs …` for a review, and the file that carries the
//! line comments of one review ([`CommentsFile`]).

use std::ffi::OsString;
use std::path::Path;

use crate::error::{BusinessError, Result};
use crate::model::{KnowledgeId, KnowledgeLineComment, KnowledgeReviewSubmit};

/// The most characters of a comment or an overall review text.
pub const COMMENT_MAX_CHARS: usize = 4000;

/// The most characters of the section heading a line comment names.
pub(super) const SECTION_MAX_CHARS: usize = 300;

/// The most line comments one review holds.
pub(super) const COMMENTS_PER_REVIEW_MAX: usize = 200;

/// The largest line number of a line comment.
pub(super) const LINE_MAX: u32 = 1_000_000;

/// A file with the line comments of one review, removed when dropped. Created for the user alone.
pub(super) struct CommentsFile {
    path: std::path::PathBuf,
}

impl CommentsFile {
    pub(super) fn write(comments: &[KnowledgeLineComment]) -> Result<Self> {
        use std::io::Write;

        // The bodies go as the length check measured them: trimmed.
        let sent: Vec<KnowledgeLineComment> = comments
            .iter()
            .map(|comment| KnowledgeLineComment {
                body: comment.body.trim().to_owned(),
                ..comment.clone()
            })
            .collect();
        let json = serde_json::to_vec(&sent).map_err(|error| BusinessError::Internal(error.to_string()))?;
        let path = std::env::temp_dir().join(format!("harness-knowledge-comments-{}.json", uuid::Uuid::new_v4()));

        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);

        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);

        let file = Self { path };
        let mut open = options.open(&file.path).map_err(|error| BusinessError::Io {
            context: "writing the comments of a review".to_owned(),
            source: error,
        })?;

        open.write_all(&json).map_err(|error| BusinessError::Io {
            context: "writing the comments of a review".to_owned(),
            source: error,
        })?;

        Ok(file)
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for CommentsFile {
    fn drop(&mut self) {
        std::fs::remove_file(&self.path).ok();
    }
}

/// The checks of a review before it is sent: the texts, the number of comments and each line range.
pub(super) fn check_review(review: &KnowledgeReviewSubmit) -> Result<()> {
    if let Some(body) = &review.body
        && !body.trim().is_empty()
    {
        check_text(body)?;
    }

    if review.comments.len() > COMMENTS_PER_REVIEW_MAX {
        return Err(invalid(&format!(
            "a review has at most {COMMENTS_PER_REVIEW_MAX} comments"
        )));
    }

    for comment in &review.comments {
        check_text(&comment.body)?;

        let section = comment.section.chars().count();
        let lines = comment.from >= 1 && comment.from <= comment.to && comment.to <= LINE_MAX;
        let occurrence = (1..=LINE_MAX).contains(&comment.occurrence);

        if section == 0 || section > SECTION_MAX_CHARS || !lines || !occurrence {
            return Err(invalid("a comment names a section and lines inside its diff"));
        }
    }

    Ok(())
}

/// `text` trimmed, when it has 1 to 4000 characters.
pub(super) fn check_text(text: &str) -> Result<&str> {
    let text = text.trim();
    let length = text.chars().count();

    if (1..=COMMENT_MAX_CHARS).contains(&length) {
        Ok(text)
    } else {
        Err(invalid(&format!("a comment has 1 to {COMMENT_MAX_CHARS} characters")))
    }
}

/// `revision` as the server's argument: a revision is never negative.
pub(super) fn check_revision(revision: i64) -> Result<OsString> {
    if revision >= 0 {
        Ok(revision.to_string().into())
    } else {
        Err(invalid("that is not a revision"))
    }
}

/// A comment's id as the server's argument: the grammar of a [`KnowledgeId`] (a positive number, or
/// `local:N` / `local:N:M`), so nothing a teammate supplied can read as an option.
pub(super) fn check_comment_id(id: &KnowledgeId) -> Result<String> {
    if id.is_valid() {
        Ok(id.to_string())
    } else {
        Err(invalid("that is not a comment id"))
    }
}

pub(super) fn invalid(message: &str) -> BusinessError {
    BusinessError::InvalidInput(message.to_owned())
}
