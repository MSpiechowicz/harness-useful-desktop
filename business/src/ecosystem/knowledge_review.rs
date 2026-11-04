//! The review of drafts and pending versions: `harness-knowledge docs ask|request-changes|approve|
//! go-live|review|history|compare|comments|comment|download` and `approval get|set`
//! (`knowledge_doc_transition`, `knowledge_doc_review`, `knowledge_doc_history`,
//! `knowledge_doc_compare`, `knowledge_doc_comments`, `knowledge_doc_comment_reply`,
//! `knowledge_doc_comment_resolve`, `knowledge_doc_comment_delete`, `knowledge_doc_download`, `knowledge_approval_get`,
//! `knowledge_approval_set`). Needs `docs-drafts` in the server's features, approval needs
//! `docs-approval`.
//!
//! - A transition is `docs ask|request-changes|approve|go-live --json ID --expect REVISION
//!   [--comment TEXT]` and prints `{"id", "status", "revision", "promoted"}`. A review is `docs
//!   review --json ID --expect REVISION --verdict comment|approve|requestChanges [--body TEXT]
//!   [--comments FILE]`, the file holding the line comments as JSON, and prints the same.
//! - A refusal exits 1 and prints `{"error": CODE, "message"}`: it is an outcome
//!   ([`KnowledgeOutcome::Refused`]), see [`knowledge_refusal`](super::knowledge_refusal).
//! - `docs history --json ID` prints `{"events", "total"}`, `docs comments --json ID` `{"comments"}`,
//!   `docs compare --json ID [--since REVISION]` `{"live", "pending", "revision", "sections",
//!   "truncated"}` with a [`FileDiff`] and an `occurrence` per section, `docs download --json ID --to PATH [--live]` `{"path", "original"}`.
//! - `approval get --json` prints `{"approval", "place", "writable"}`, `approval set --json on|off`
//!   the same.
//!
//! What goes to the server is checked first (`invalidInput`): ids, revisions, texts and lines.
//! Neither the documents' text nor the comments are logged.

use std::path::Path;

use serde_json::Value;

use super::knowledge_cli::{CliCall, call, field, words};
use super::knowledge_docs::check_id;
use super::knowledge_features::{KnowledgeFeatures, KnowledgeSupport};
use super::knowledge_refusal::{Answer, run_refusing};
use super::knowledge_review_check::{
    CommentsFile, check_comment_id, check_review, check_revision, check_text, invalid,
};
use super::knowledge_review_read::{
    comment_required, read_changed, read_compare, read_deleted, read_transitioned, renamed_lines,
};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{
    KnowledgeComment, KnowledgeCommentChanged, KnowledgeCommentsDeleted, KnowledgeCompare, KnowledgeDownloaded,
    KnowledgeHistory, KnowledgeId, KnowledgeOutcome, KnowledgeRefusalCode, KnowledgeReviewSubmit, KnowledgeTransition,
    KnowledgeTransitioned, KnowledgeVerdict,
};
use crate::process::ProcessRunner;

/// The most characters of the comment of a request for changes.
const REQUEST_MAX_CHARS: usize = 500;

/// The output kept of a compare or a list of comments: the server caps the texts at 1 MiB, and
/// each line carries its numbers and kind besides.
const COMPARE_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

/// The installed server when it lists `docs-drafts`.
pub async fn drafts_server(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &KnowledgeFeatures,
) -> Result<std::path::PathBuf> {
    features
        .feature_server(env, runner, KnowledgeSupport::docs_drafts, "drafts")
        .await
}

/// The installed server when it lists `docs-approval`.
pub async fn approval_server(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &KnowledgeFeatures,
) -> Result<std::path::PathBuf> {
    features
        .feature_server(env, runner, KnowledgeSupport::docs_approval, "team approval")
        .await
}

/// Which server feature a transition needs: asking, approving and requesting changes need team
/// approval, making live and taking an ask back do not.
pub fn transition_needs_approval(action: KnowledgeTransition) -> bool {
    !matches!(action, KnowledgeTransition::GoLive | KnowledgeTransition::TakeBack)
}

/// The installed server for a review with `verdict`: one that only comments needs `docs-drafts`, an
/// approval or a request for changes also needs `docs-approval` (`unsupported` otherwise).
pub async fn review_server(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &KnowledgeFeatures,
    verdict: KnowledgeVerdict,
) -> Result<std::path::PathBuf> {
    match verdict {
        KnowledgeVerdict::Comment => drafts_server(env, runner, features).await,
        KnowledgeVerdict::Approve | KnowledgeVerdict::RequestChanges => approval_server(env, runner, features).await,
    }
}

/// Moves document `id` (a draft, a waiting document, or a live one with a pending version) one
/// step, only while its revision is `expect` (`docs ask|request-changes|approve|go-live|take-back --json ID
/// --expect REVISION [--comment TEXT]`). Requesting changes needs a `comment` of 1 to 500
/// characters (an empty one is refused as `commentRequired` without asking the server); the
/// others take none.
pub async fn transition_doc(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    id: &KnowledgeId,
    action: KnowledgeTransition,
    expect: i64,
    comment: Option<&str>,
) -> Result<KnowledgeOutcome<KnowledgeTransitioned>> {
    let (verb, name) = match action {
        KnowledgeTransition::Ask => ("ask", "docs ask"),
        KnowledgeTransition::RequestChanges => ("request-changes", "docs request-changes"),
        KnowledgeTransition::Approve => ("approve", "docs approve"),
        KnowledgeTransition::GoLive => ("go-live", "docs go-live"),
        KnowledgeTransition::TakeBack => ("take-back", "docs take-back"),
    };

    let mut args = words(&["docs", verb, "--json"]);
    args.extend([check_id(id)?.into(), "--expect".into(), check_revision(expect)?]);

    if action == KnowledgeTransition::RequestChanges {
        let comment = comment.map_or("", str::trim);

        if comment.is_empty() {
            return Ok(comment_required());
        }

        if comment.chars().count() > REQUEST_MAX_CHARS {
            return Err(invalid(&format!("a comment has 1 to {REQUEST_MAX_CHARS} characters")));
        }

        args.extend(["--comment".into(), comment.into()]);
    }

    let outcome = outcome(env, runner, server, call(name, args), read_transitioned).await?;
    tracing::info!(verb, "a knowledge document was moved on");

    Ok(outcome)
}

/// Submits a review of document `id`: the overall `body`, the comments on lines and the verdict
/// (`docs review --json ID --expect REVISION --verdict V [--body TEXT] [--comments FILE]`). The line
/// comments go to the server in a file that is removed afterwards.
pub async fn review_doc(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    id: &KnowledgeId,
    expect: i64,
    review: &KnowledgeReviewSubmit,
) -> Result<KnowledgeOutcome<KnowledgeTransitioned>> {
    check_review(review)?;

    let verdict = match review.verdict {
        KnowledgeVerdict::Comment => "comment",
        KnowledgeVerdict::Approve => "approve",
        KnowledgeVerdict::RequestChanges => "requestChanges",
    };

    let mut args = words(&["docs", "review", "--json"]);
    args.extend([
        check_id(id)?.into(),
        "--expect".into(),
        check_revision(expect)?,
        "--verdict".into(),
        verdict.into(),
    ]);

    if let Some(body) = review.body.as_deref().map(str::trim).filter(|body| !body.is_empty()) {
        args.extend(["--body".into(), body.into()]);
    }

    // Kept until the server answered, removed when it goes out of scope.
    let _file = if review.comments.is_empty() {
        None
    } else {
        let file = CommentsFile::write(&review.comments)?;
        args.extend(["--comments".into(), file.path().as_os_str().to_owned()]);

        Some(file)
    };

    let outcome = outcome(env, runner, server, call("docs review", args), read_transitioned).await?;
    tracing::info!(
        verdict,
        comments = review.comments.len(),
        "a knowledge document was reviewed"
    );

    Ok(outcome)
}

/// The events of document `id`, newest first (`docs history --json ID`). `notFound` for a document
/// that does not exist.
pub async fn doc_history(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    id: &KnowledgeId,
) -> Result<KnowledgeHistory> {
    let mut args = words(&["docs", "history", "--json"]);
    args.push(check_id(id)?.into());

    let data = answered(env, runner, server, call("docs history", args)).await?;

    serde_json::from_value(data).map_err(|_| {
        BusinessError::Protocol("harness-knowledge docs history answered events this app cannot read".to_owned())
    })
}

/// The comments of document `id` in the order the server gives them (`docs comments --json ID`).
/// `notFound` for a document that does not exist.
pub async fn doc_comments(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    id: &KnowledgeId,
) -> Result<Vec<KnowledgeComment>> {
    let mut args = words(&["docs", "comments", "--json"]);
    args.push(check_id(id)?.into());

    let mut data = answered(env, runner, server, large(call("docs comments", args))).await?;

    // The lines of a comment are `lineFrom` and `lineTo` in the store: read them as `from` and `to`.
    if let Some(Value::Array(comments)) = data.get_mut("comments") {
        comments.iter_mut().for_each(renamed_lines);
    }

    field(&data, "comments", "docs comments")
}

/// Answers `body` to comment `comment` of document `id` (`docs comment reply --json ID COMMENT_ID
/// --body TEXT`, 1 to 4000 characters).
pub async fn reply_comment(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    id: &KnowledgeId,
    comment: &KnowledgeId,
    body: &str,
) -> Result<KnowledgeOutcome<KnowledgeCommentChanged>> {
    let body = check_text(body)?;

    let mut args = words(&["docs", "comment", "reply", "--json"]);
    args.extend([
        check_id(id)?.into(),
        check_comment_id(comment)?.into(),
        "--body".into(),
        body.into(),
    ]);

    outcome(env, runner, server, call("docs comment reply", args), read_changed).await
}

/// Resolves (or, with `resolved` false, reopens) comment `comment` of document `id` (`docs comment
/// resolve|unresolve --json ID COMMENT_ID`).
pub async fn resolve_comment(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    id: &KnowledgeId,
    comment: &KnowledgeId,
    resolved: bool,
) -> Result<KnowledgeOutcome<KnowledgeCommentChanged>> {
    let (verb, name) = if resolved {
        ("resolve", "docs comment resolve")
    } else {
        ("unresolve", "docs comment unresolve")
    };

    let mut args = words(&["docs", "comment", verb, "--json"]);
    args.extend([check_id(id)?.into(), check_comment_id(comment)?.into()]);

    outcome(env, runner, server, call(name, args), read_changed).await
}

/// Deletes comment `comment` of document `id`, written by the one who asks (`docs comment delete
/// --json ID COMMENT_ID`). A comment that starts a thread takes its replies with it. The server
/// refuses another person's comment (`notAuthor`), a comment that is gone (`notFound`), a reader
/// with no git name (`noName`) and a read-only role (`readOnly`).
pub async fn delete_comment(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    id: &KnowledgeId,
    comment: &KnowledgeId,
) -> Result<KnowledgeOutcome<KnowledgeCommentsDeleted>> {
    let mut args = words(&["docs", "comment", "delete", "--json"]);
    args.extend([check_id(id)?.into(), check_comment_id(comment)?.into()]);

    let outcome = outcome(env, runner, server, call("docs comment delete", args), read_deleted).await?;
    tracing::info!("a knowledge comment was deleted");

    Ok(outcome)
}

/// Compares the pending version of document `id` (the pending row, or a live row that has one) with
/// its live document, or with the version that was asked when `since` (a revision) is given
/// (`docs compare --json ID [--since REVISION]`). The sections that differ come as [`FileDiff`]s;
/// `sinceUnavailable` is a refusal when the snapshot of that revision was not kept.
pub async fn compare_doc(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    id: &KnowledgeId,
    since: Option<i64>,
) -> Result<KnowledgeOutcome<KnowledgeCompare>> {
    let mut args = words(&["docs", "compare", "--json"]);
    args.push(check_id(id)?.into());

    if let Some(since) = since {
        args.extend(["--since".into(), check_revision(since)?]);
    }

    outcome(env, runner, server, large(call("docs compare", args)), read_compare).await
}

/// Writes the original file of document `id` (its stored text as Markdown or text when the original
/// was not kept) to `to` (`docs download --json ID --to PATH [--live]`); `live` takes the live
/// document's file instead of the pending row's.
pub async fn download_doc(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    id: &KnowledgeId,
    to: &Path,
    live: bool,
) -> Result<KnowledgeOutcome<KnowledgeDownloaded>> {
    let mut args = words(&["docs", "download", "--json"]);
    args.extend([check_id(id)?.into(), "--to".into(), to.as_os_str().to_owned()]);

    if live {
        args.push("--live".into());
    }

    let requested = to.to_string_lossy().into_owned();

    outcome(env, runner, server, call("docs download", args), move |data| {
        Ok(KnowledgeDownloaded {
            path: data
                .get("path")
                .and_then(Value::as_str)
                .map_or_else(|| requested.clone(), str::to_owned),
            original: data.get("original").and_then(Value::as_bool).unwrap_or(false),
        })
    })
    .await
}

/// Runs `call` and reads its answer with `read`: its refusal is an outcome.
pub(super) async fn outcome<T>(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    call: CliCall<'_>,
    read: impl FnOnce(&Value) -> Result<T>,
) -> Result<KnowledgeOutcome<T>> {
    match run_refusing(env, runner, server, call).await? {
        Answer::Done(data) => Ok(KnowledgeOutcome::Done { value: read(&data)? }),
        Answer::Refused(code, message) => Ok(KnowledgeOutcome::Refused { code, message }),
    }
}

/// Runs `call` for an answer that has no refusal to show: `notFound` for a refusal of that name,
/// `process` with the server's sentence for any other.
pub(super) async fn answered(env: &Env, runner: &dyn ProcessRunner, server: &Path, call: CliCall<'_>) -> Result<Value> {
    match run_refusing(env, runner, server, call).await? {
        Answer::Done(data) => Ok(data),
        Answer::Refused(KnowledgeRefusalCode::NotFound, message) => Err(BusinessError::NotFound(message)),
        Answer::Refused(_, message) => Err(BusinessError::Process(message)),
    }
}

/// `call` with the output limit of a compare.
pub(super) fn large(mut call: CliCall<'_>) -> CliCall<'_> {
    call.output_bytes = COMPARE_OUTPUT_BYTES;

    call
}
