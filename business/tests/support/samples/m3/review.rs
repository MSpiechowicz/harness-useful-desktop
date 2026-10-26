//! Review fixture (`review/`): comments on the `git/` diff composed by the real composer.

use harness_business::model::{CommentSide, ReviewComment, ReviewRequest};
use harness_business::review::compose_review;
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

/// The `review/` fixtures.
pub fn fixtures() -> Vec<(&'static str, Value)> {
    let composed = compose_review(&request()).expect("the sample review composes");
    vec![("review/review_composed.json", to_json(composed))]
}
