//! `review_compose`: diff comments into one deterministic Markdown prompt.
//!
//! Layout: an intro line, the optional note, then one `### <path>` section per file (in order of
//! first appearance) with its comments in request order. A comment is a list item
//! `- Lines a–b (new|old): body` (`- Line a (…)` for one line; continuation lines of the body are
//! indented into the item) followed by the commented lines in a fenced block whose language comes
//! from the file extension. A fence is always longer than any backtick run inside the snippet.

use std::fmt::Write as _;

use crate::error::{BusinessError, Result};
use crate::model::{
    CommentSide, ComposedReview, MAX_REVIEW_COMMENTS, MAX_REVIEW_PROMPT_BYTES, MAX_REVIEW_SNIPPET_LINES,
    MAX_REVIEW_TEXT_BYTES, ReviewComment, ReviewRequest,
};

/// First line of every review prompt.
pub const INTRO: &str = "Please address these review comments on the current changes.";

/// Longest accepted comment path.
const MAX_PATH_BYTES: usize = 4096;

/// Composes `request` (see the module docs). Fails with `InvalidInput` when a limit is exceeded,
/// a line range is invalid, a path is empty or holds control characters, or the request is empty.
pub fn compose_review(request: &ReviewRequest) -> Result<ComposedReview> {
    validate(request)?;

    let mut files: Vec<&str> = Vec::new();
    for comment in &request.comments {
        if !files.contains(&comment.path.as_str()) {
            files.push(&comment.path);
        }
    }

    let mut sections = vec![format!("{INTRO}\n")];

    if let Some(note) = request.note.as_deref().map(str::trim).filter(|note| !note.is_empty()) {
        sections.push(format!("{note}\n"));
    }

    for path in &files {
        sections.push(format!("### {path}\n"));

        let comments = request.comments.iter().filter(|comment| comment.path == *path);
        sections.extend(comments.map(comment_text));
    }

    let prompt = sections.join("\n");

    if prompt.len() > MAX_REVIEW_PROMPT_BYTES {
        return Err(invalid(format!(
            "the review prompt is larger than {} KiB",
            MAX_REVIEW_PROMPT_BYTES / 1024
        )));
    }

    Ok(ComposedReview {
        bytes: u32::try_from(prompt.len()).unwrap_or(u32::MAX),
        file_count: u32::try_from(files.len()).unwrap_or(u32::MAX),
        comment_count: u32::try_from(request.comments.len()).unwrap_or(u32::MAX),
        prompt,
    })
}

fn comment_text(comment: &ReviewComment) -> String {
    let side = match comment.side {
        CommentSide::Old => "old",
        CommentSide::New => "new",
    };

    let lines = if comment.start_line == comment.end_line {
        format!("Line {}", comment.start_line)
    } else {
        format!("Lines {}–{}", comment.start_line, comment.end_line)
    };

    let body = comment.body.trim().replace("\r\n", "\n").replace('\n', "\n  ");
    let mut text = format!("- {lines} ({side}): {body}\n");

    if !comment.snippet.is_empty() {
        let snippet = comment.snippet.join("\n");
        let fence = "`".repeat(longest_backtick_run(&snippet).max(2) + 1);
        let language = language(&comment.path);

        let _ = write!(text, "\n{fence}{language}\n{snippet}\n{fence}\n");
    }

    text
}

fn longest_backtick_run(text: &str) -> usize {
    text.split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or(0)
}

/// The fenced-code language hint for `path` (empty when unknown).
pub fn language(path: &str) -> &'static str {
    let name = path.rsplit('/').next().unwrap_or(path);

    if name.eq_ignore_ascii_case("dockerfile") {
        return "dockerfile";
    }

    if name.eq_ignore_ascii_case("makefile") {
        return "makefile";
    }

    let extension = match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => extension.to_ascii_lowercase(),
        _ => return "",
    };

    match extension.as_str() {
        "rs" => "rust",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "jsx",
        "svelte" => "svelte",
        "vue" => "vue",
        "py" => "python",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" | "hh" => "cpp",
        "cs" => "csharp",
        "rb" => "ruby",
        "php" => "php",
        "sh" | "bash" => "bash",
        "zsh" => "zsh",
        "fish" => "fish",
        "ps1" => "powershell",
        "json" | "jsonc" => "json",
        "yml" | "yaml" => "yaml",
        "toml" => "toml",
        "md" | "markdown" => "markdown",
        "html" | "htm" => "html",
        "css" => "css",
        "scss" => "scss",
        "sql" => "sql",
        "xml" => "xml",
        "lua" => "lua",
        "dart" => "dart",
        "ex" | "exs" => "elixir",
        "hs" => "haskell",
        "zig" => "zig",
        _ => "",
    }
}

fn validate(request: &ReviewRequest) -> Result<()> {
    let has_note = request.note.as_deref().is_some_and(|note| !note.trim().is_empty());

    if request.comments.is_empty() && !has_note {
        return Err(invalid("a review needs a comment or a note".to_owned()));
    }

    if request.comments.len() > MAX_REVIEW_COMMENTS {
        return Err(invalid(format!("at most {MAX_REVIEW_COMMENTS} comments per review")));
    }

    if request
        .note
        .as_ref()
        .is_some_and(|note| note.len() > MAX_REVIEW_TEXT_BYTES)
    {
        return Err(invalid(format!(
            "the note is larger than {MAX_REVIEW_TEXT_BYTES} bytes"
        )));
    }

    request.comments.iter().try_for_each(validate_comment)
}

fn validate_comment(comment: &ReviewComment) -> Result<()> {
    let path = &comment.path;

    if path.trim().is_empty() || path.len() > MAX_PATH_BYTES || path.chars().any(char::is_control) {
        return Err(invalid("a comment path is empty or invalid".to_owned()));
    }

    if comment.start_line == 0 || comment.end_line < comment.start_line {
        return Err(invalid(format!("invalid line range in a comment on {path}")));
    }

    if comment.body.trim().is_empty() || comment.body.len() > MAX_REVIEW_TEXT_BYTES {
        return Err(invalid(format!(
            "a comment on {path} is empty or larger than {MAX_REVIEW_TEXT_BYTES} bytes"
        )));
    }

    if comment.snippet.len() > MAX_REVIEW_SNIPPET_LINES {
        return Err(invalid(format!(
            "a comment on {path} quotes more than {MAX_REVIEW_SNIPPET_LINES} lines"
        )));
    }

    Ok(())
}

fn invalid(message: String) -> BusinessError {
    BusinessError::InvalidInput(message)
}
