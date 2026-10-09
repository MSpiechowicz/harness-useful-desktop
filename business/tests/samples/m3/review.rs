//! Synthetic review fixture (`review/`): comments on the `git/` diff composed into a prompt.

use harness_business::model::{CommentSide, ComposedReview, ReviewComment, ReviewRequest};
use serde_json::Value;

use super::to_json;

fn comment(path: &str, side: CommentSide, lines: (u32, u32), body: &str, snippet: &[&str]) -> ReviewComment {
    ReviewComment {
        path: path.to_owned(),
        side,
        start_line: lines.0,
        end_line: lines.1,
        body: body.to_owned(),
        snippet: snippet.iter().map(|line| (*line).to_owned()).collect(),
    }
}

fn request() -> ReviewRequest {
    let test = [
        "    fn parses_empty_input() {",
        "        assert!(super::parse(\"\").unwrap().is_empty());",
        "    }",
    ];
    let signature = ["pub fn parse(input: &str) -> Result<Vec<Record>, String> {"];

    ReviewRequest {
        comments: vec![
            comment(
                "src/lib.rs",
                CommentSide::New,
                (32, 34),
                "Also cover whitespace-only input here.",
                &test,
            ),
            comment(
                "src/lib.rs",
                CommentSide::Old,
                (3, 3),
                "This signature is public API; note the breaking change in CHANGELOG.md.",
                &signature,
            ),
            comment(
                "docs/guide.md",
                CommentSide::New,
                (1, 1),
                "Keep the `# Usage` title; external links point to it.",
                &["# Guide"],
            ),
        ],
        note: Some("Address every comment, then run `cargo test`.".to_owned()),
    }
}

fn language(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("rs") => "rust",
        Some("md") => "markdown",
        _ => "",
    }
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
    let snippet = comment.snippet.join("\n");

    format!(
        "- {lines} ({side}): {}\n\n```{}\n{snippet}\n```\n",
        comment.body,
        language(&comment.path)
    )
}

/// The prompt as the composer lays it out: intro, note, then comments grouped by file in first
/// appearance order.
fn composed(request: &ReviewRequest) -> ComposedReview {
    let mut files: Vec<&str> = Vec::new();
    for comment in &request.comments {
        if !files.contains(&comment.path.as_str()) {
            files.push(&comment.path);
        }
    }

    let mut sections = vec!["Please address these review comments on the current changes.\n".to_owned()];
    sections.extend(request.note.as_ref().map(|note| format!("{note}\n")));

    for path in &files {
        sections.push(format!("### {path}\n"));
        let comments = request.comments.iter().filter(|comment| comment.path == *path);
        sections.extend(comments.map(comment_text));
    }

    let prompt = sections.join("\n");
    ComposedReview {
        bytes: prompt.len() as u32,
        file_count: files.len() as u32,
        comment_count: request.comments.len() as u32,
        prompt,
    }
}

/// The `review/` fixtures.
pub fn fixtures() -> Vec<(&'static str, Value)> {
    vec![("review/review_composed.json", to_json(composed(&request())))]
}
