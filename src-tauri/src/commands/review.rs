//! Review command: diff comments composed into an agent prompt. The webview sends the prompt
//! itself with `agent_send`; nothing here talks to an agent or reads files.

use harness_business::model::{ComposedReview, ReviewRequest};
use harness_business::review::compose_review;

use crate::error::CommandResult;

/// The deterministic Markdown prompt for `request`; `invalidInput` beyond the review limits.
/// Async so it runs on the async runtime rather than the main thread.
#[tauri::command]
pub async fn review_compose(request: ReviewRequest) -> CommandResult<ComposedReview> {
    Ok(compose_review(&request)?)
}

#[cfg(test)]
mod tests {
    use harness_business::model::{CommentSide, IpcErrorCode, MAX_REVIEW_COMMENTS, ReviewComment};
    use tauri::async_runtime::block_on;

    use super::*;

    fn comment(line: u32) -> ReviewComment {
        ReviewComment {
            path: "src/lib.rs".to_owned(),
            side: CommentSide::New,
            start_line: line,
            end_line: line,
            body: "Handle the empty case.".to_owned(),
            snippet: vec!["fn parse() {}".to_owned()],
        }
    }

    #[test]
    fn comments_are_composed_and_limits_enforced() {
        let request = ReviewRequest {
            comments: vec![comment(3)],
            note: None,
        };
        let composed = block_on(review_compose(request)).expect("composed");
        assert!(composed.prompt.contains("### src/lib.rs"), "{}", composed.prompt);
        assert_eq!((composed.file_count, composed.comment_count), (1, 1));

        let too_many = ReviewRequest {
            comments: (0..=MAX_REVIEW_COMMENTS as u32).map(comment).collect(),
            note: None,
        };
        let error = block_on(review_compose(too_many)).expect_err("too many comments");
        assert_eq!(error.code(), IpcErrorCode::InvalidInput);
    }
}
