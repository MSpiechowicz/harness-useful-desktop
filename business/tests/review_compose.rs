//! `review_compose`: layout snapshot, fence escaping and limits.

use harness_business::model::{
    CommentSide, IpcErrorCode, MAX_REVIEW_COMMENTS, MAX_REVIEW_SNIPPET_LINES, MAX_REVIEW_TEXT_BYTES, ReviewComment,
    ReviewRequest,
};
use harness_business::review::{compose_review, language};
use pretty_assertions::assert_eq;

/// A comment on `lines` of `path` quoting `snippet`.
fn comment(path: &str, side: CommentSide, lines: (u32, u32), body: &str, snippet: &[&str]) -> ReviewComment {
    let (start_line, end_line) = lines;
    let snippet = snippet.iter().copied().map(String::from).collect();

    ReviewComment {
        snippet,
        start_line,
        end_line,
        side,
        path: path.into(),
        body: body.into(),
    }
}

fn request(comments: Vec<ReviewComment>, note: Option<&str>) -> ReviewRequest {
    ReviewRequest {
        comments,
        note: note.map(str::to_owned),
    }
}

fn rejected(request: &ReviewRequest) -> bool {
    compose_review(request).is_err_and(|error| error.code() == IpcErrorCode::InvalidInput)
}

#[test]
fn comments_are_grouped_by_file_in_first_appearance_order() {
    let review = request(
        vec![
            comment(
                "src/lib.rs",
                CommentSide::New,
                (32, 34),
                "Cover whitespace-only input.",
                &["    fn parses_empty_input() {", "    }"],
            ),
            comment(
                "docs/guide.md",
                CommentSide::New,
                (1, 1),
                "Keep the title.\nLinks point to it.",
                &["# Guide"],
            ),
            comment("src/lib.rs", CommentSide::Old, (3, 3), "Public API change.", &[]),
        ],
        Some("Address every comment, then run `cargo test`."),
    );

    let composed = compose_review(&review).unwrap();

    insta::assert_snapshot!(composed.prompt, @r"
    Please address these review comments on the current changes.

    Address every comment, then run `cargo test`.

    ### src/lib.rs

    - Lines 32–34 (new): Cover whitespace-only input.

    ```rust
        fn parses_empty_input() {
        }
    ```

    - Line 3 (old): Public API change.

    ### docs/guide.md

    - Line 1 (new): Keep the title.
      Links point to it.

    ```markdown
    # Guide
    ```
    ");
    assert_eq!(composed.bytes as usize, composed.prompt.len());
    assert_eq!((composed.file_count, composed.comment_count), (2, 3));
    assert_eq!(compose_review(&review).unwrap(), composed, "deterministic");
}

#[test]
fn snippets_with_backtick_fences_get_a_longer_fence() {
    let review = request(
        vec![comment(
            "README.md",
            CommentSide::New,
            (4, 6),
            "Fix the example.",
            &["```sh", "cargo run", "````"],
        )],
        None,
    );
    let prompt = compose_review(&review).unwrap().prompt;

    assert!(
        prompt.contains("\n`````markdown\n```sh\ncargo run\n````\n`````\n"),
        "{prompt}"
    );
}

#[test]
fn languages_come_from_the_extension() {
    for (path, expected) in [
        ("src/main.rs", "rust"),
        ("ui/App.svelte", "svelte"),
        ("a/b.TS", "typescript"),
        ("Dockerfile", "dockerfile"),
        (".gitignore", ""),
        ("notes", ""),
    ] {
        assert_eq!(language(path), expected, "{path}");
    }
}

#[test]
fn limits_and_invalid_comments_are_rejected() {
    let valid = || comment("a.rs", CommentSide::New, (1, 2), "Fix.", &["x"]);

    assert!(rejected(&request(Vec::new(), None)));
    assert!(
        compose_review(&request(Vec::new(), Some("Tidy up."))).is_ok(),
        "a note alone is a review"
    );
    assert!(rejected(&request(vec![valid(); MAX_REVIEW_COMMENTS + 1], None)));
    assert!(rejected(&request(
        vec![valid()],
        Some(&"n".repeat(MAX_REVIEW_TEXT_BYTES + 1))
    )));

    let edits: [fn(&mut ReviewComment); 6] = [
        |comment| comment.body = "b".repeat(MAX_REVIEW_TEXT_BYTES + 1),
        |comment| comment.body = "  ".to_owned(),
        |comment| comment.snippet = vec!["x".to_owned(); MAX_REVIEW_SNIPPET_LINES + 1],
        |comment| comment.start_line = 0,
        |comment| comment.end_line = 0,
        |comment| comment.path = "a.rs\n### injected".to_owned(),
    ];

    for edit in edits {
        let mut bad = valid();
        edit(&mut bad);
        assert!(rejected(&request(vec![bad], None)));
    }
}

#[test]
fn prompts_over_the_size_limit_are_rejected() {
    let long_lines = vec!["y".repeat(7000); MAX_REVIEW_SNIPPET_LINES];
    let snippet: Vec<&str> = long_lines.iter().map(String::as_str).collect();
    let comments = (0..3)
        .map(|index| comment(&format!("f{index}.rs"), CommentSide::New, (1, 40), "Shorten.", &snippet))
        .collect();

    assert!(rejected(&request(comments, None)));
}
