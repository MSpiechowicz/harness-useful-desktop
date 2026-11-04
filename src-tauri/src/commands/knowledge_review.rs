//! Drafts and team approval in the Knowledge view: moving a document one step
//! (`knowledge_doc_transition`), reviewing a pending version (`knowledge_doc_review`,
//! `knowledge_doc_compare`, `knowledge_doc_comments`, `knowledge_doc_comment_reply`,
//! `knowledge_doc_comment_resolve`, `knowledge_doc_comment_delete`, `knowledge_doc_history`), saving a document as a file
//! (`knowledge_doc_download`, `knowledge_reveal_download`), the team approval switch (`knowledge_approval_get`,
//! `knowledge_approval_set`) and asking the agent that proposed a document for changes
//! (`knowledge_held_request_changes`, `knowledge_held_follow_up_available`), through
//! `harness-knowledge docs …`, `approval` and `held request-changes`. See
//! `harness_business::ecosystem::knowledge_review`.
//!
//! The commands need `docs-drafts` (approval and the approving transitions `docs-approval`): `notFound`
//! when Harness Knowledge is not installed, `unsupported` when it predates them. A refusal the
//! server explains with a code is an answer (`refused`), not an error.

use std::path::Path;

use harness_business::BusinessError;
use harness_business::ecosystem::{
    approval_server, compare_doc, delete_comment, doc_comments, doc_history, download_doc, drafts_server, get_approval,
    get_knowledge_held, knowledge_held_server, reply_comment, request_changes_knowledge_held, resolve_comment,
    review_doc, review_server, set_approval, transition_doc, transition_needs_approval,
};
use harness_business::model::{
    KnowledgeChangesRequested, KnowledgeComment, KnowledgeCommentChanged, KnowledgeCommentsDeleted, KnowledgeCompare,
    KnowledgeDecided, KnowledgeDownloaded, KnowledgeHistory, KnowledgeId, KnowledgeOutcome, KnowledgeReviewSubmit,
    KnowledgeTeamApproval, KnowledgeTransition, KnowledgeTransitioned, display_text,
};
use tauri::{AppHandle, Manager, State};

use super::{blocking, saved_language};
use crate::dialogs::{self, FileFilter};
use crate::error::CommandResult;
use crate::knowledge_follow_up;
use crate::native_text::native_text;
use crate::state::AppState;

/// The name the save dialog suggests when the page gives none.
const DEFAULT_DOWNLOAD_NAME: &str = "document.md";

/// The most characters of the file name the save dialog suggests.
const DOWNLOAD_NAME_MAX_CHARS: usize = 200;

/// The longest session id of a proposal the app looks up (the server's own are UUIDs).
const SESSION_MAX_CHARS: usize = 200;

/// Moves a document one step, only while its `revision` is the one the page showed: `ask` (draft or
/// changes requested to waiting), `requestChanges` (waiting to changes requested, needs a `comment`
/// of 1 to 500 characters), `approve` (waiting to live, by a teammate of the author) or `goLive`
/// (by its author, with approval off). The first three need `docs-approval`. Answers the state the
/// document is in, the live id after a promote, or the server's refusal (`changed`, `wrongState`,
/// `approvalOn`, `approvalOff`, `selfApproval`, `noName`, `readOnly`, `commentRequired`, ...).
/// `unreachable` while the team database cannot be reached.
#[tauri::command]
pub async fn knowledge_doc_transition(
    state: State<'_, AppState>,
    id: KnowledgeId,
    action: KnowledgeTransition,
    revision: i64,
    comment: Option<String>,
) -> CommandResult<KnowledgeOutcome<KnowledgeTransitioned>> {
    let server = if transition_needs_approval(action) {
        approval_server(&state.env, &state.runner, &state.knowledge_features).await?
    } else {
        drafts_server(&state.env, &state.runner, &state.knowledge_features).await?
    };

    Ok(transition_doc(
        &state.env,
        &state.runner,
        &server,
        &id,
        action,
        revision,
        comment.as_deref(),
    )
    .await?)
}

/// Submits a review of a pending version: the overall comment, the comments on lines of the diff
/// (at most 200 of 1 to 4000 characters) and the verdict (`comment` changes nothing, `approve`
/// makes the version live, `requestChanges` needs an overall comment). Only while the `revision`
/// is the one reviewed. Answers like `knowledge_doc_transition`. `approve` and `requestChanges`
/// need `docs-approval` like the transitions do, `comment` needs `docs-drafts` only.
#[tauri::command]
pub async fn knowledge_doc_review(
    state: State<'_, AppState>,
    id: KnowledgeId,
    revision: i64,
    review: KnowledgeReviewSubmit,
) -> CommandResult<KnowledgeOutcome<KnowledgeTransitioned>> {
    let server = review_server(&state.env, &state.runner, &state.knowledge_features, review.verdict).await?;

    Ok(review_doc(&state.env, &state.runner, &server, &id, revision, &review).await?)
}

/// The events of a document, newest first (at most 200), merged with the rounds of its promoted
/// pending versions. `notFound` for a document that does not exist.
#[tauri::command]
pub async fn knowledge_doc_history(state: State<'_, AppState>, id: KnowledgeId) -> CommandResult<KnowledgeHistory> {
    let server = drafts_server(&state.env, &state.runner, &state.knowledge_features).await?;

    Ok(doc_history(&state.env, &state.runner, &server, &id).await?)
}

/// The comments of a document (the pending row, or a live row with a pending version), with the
/// replies and whether each is outdated or resolved. `notFound` for a document that does not exist.
#[tauri::command]
pub async fn knowledge_doc_comments(
    state: State<'_, AppState>,
    id: KnowledgeId,
) -> CommandResult<Vec<KnowledgeComment>> {
    let server = drafts_server(&state.env, &state.runner, &state.knowledge_features).await?;

    Ok(doc_comments(&state.env, &state.runner, &server, &id).await?)
}

/// Answers a comment with `body` (1 to 4000 characters). Anybody may reply, the author included.
#[tauri::command]
pub async fn knowledge_doc_comment_reply(
    state: State<'_, AppState>,
    id: KnowledgeId,
    comment_id: KnowledgeId,
    body: String,
) -> CommandResult<KnowledgeOutcome<KnowledgeCommentChanged>> {
    let server = drafts_server(&state.env, &state.runner, &state.knowledge_features).await?;

    Ok(reply_comment(&state.env, &state.runner, &server, &id, &comment_id, &body).await?)
}

/// Resolves a comment (`resolved`), or opens it again.
#[tauri::command]
pub async fn knowledge_doc_comment_resolve(
    state: State<'_, AppState>,
    id: KnowledgeId,
    comment_id: KnowledgeId,
    resolved: bool,
) -> CommandResult<KnowledgeOutcome<KnowledgeCommentChanged>> {
    let server = drafts_server(&state.env, &state.runner, &state.knowledge_features).await?;

    Ok(resolve_comment(&state.env, &state.runner, &server, &id, &comment_id, resolved).await?)
}

/// Deletes a comment the user wrote, and its replies when it starts a thread. Refuses another
/// person's comment (`notAuthor`) and a read-only role (`readOnly`). History is untouched.
#[tauri::command]
pub async fn knowledge_doc_comment_delete(
    state: State<'_, AppState>,
    id: KnowledgeId,
    comment_id: KnowledgeId,
) -> CommandResult<KnowledgeOutcome<KnowledgeCommentsDeleted>> {
    let server = drafts_server(&state.env, &state.runner, &state.knowledge_features).await?;

    Ok(delete_comment(&state.env, &state.runner, &server, &id, &comment_id).await?)
}

/// Compares a pending version (the pending row, or a live row that has one) with its live document,
/// or with the version that was asked when `since` is the revision it was asked at: the sections that
/// differ, each as the Changes view shows a file. A new document compares with nothing, so every
/// line is added. `sinceUnavailable` is answered when that version was not kept.
#[tauri::command]
pub async fn knowledge_doc_compare(
    state: State<'_, AppState>,
    id: KnowledgeId,
    since: Option<i64>,
) -> CommandResult<KnowledgeOutcome<KnowledgeCompare>> {
    let server = drafts_server(&state.env, &state.runner, &state.knowledge_features).await?;

    Ok(compare_doc(&state.env, &state.runner, &server, &id, since).await?)
}

/// Lets the user choose where to save a document in a native dialog (suggesting `name`, a file
/// name without a folder) and writes its original file there, or its stored text as Markdown or text
/// when the original was not kept (`original` says which). `live` takes the live document's file
/// instead of its pending version's. `None` when the user cancelled.
#[tauri::command]
pub async fn knowledge_doc_download(
    app: AppHandle,
    state: State<'_, AppState>,
    id: KnowledgeId,
    name: Option<String>,
    live: Option<bool>,
) -> CommandResult<Option<KnowledgeOutcome<KnowledgeDownloaded>>> {
    let server = drafts_server(&state.env, &state.runner, &state.knowledge_features).await?;
    let text = native_text(&saved_language(&state).await?);
    let default_name = download_name(name.as_deref());
    let extension = Path::new(&default_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("md")
        .to_owned();
    let filter = FileFilter {
        name: text.knowledge_files,
        extensions: &[extension.as_str()],
    };

    let downloads = app
        .path()
        .download_dir()
        .ok()
        .filter(|folder| folder.is_dir())
        .unwrap_or_else(|| state.env.home.clone());

    let chosen = dialogs::save(
        &app,
        &state.dialog,
        text.save_knowledge_document,
        Some(&downloads),
        &default_name,
        filter,
    )
    .await?;

    let Some(path) = chosen else {
        return Ok(None);
    };

    let outcome = download_doc(&state.env, &state.runner, &server, &id, &path, live.unwrap_or(false)).await?;

    // Only a file that was written can be shown in its folder afterwards.
    // The sidecar prints the path it wrote: the chosen one, or the stored text beside it (see `remember_download`).
    if let KnowledgeOutcome::Done { value } = &outcome {
        state.saved_downloads.remember_download(&path, &value.path);
    }

    Ok(Some(outcome))
}

/// Opens the file manager with a file `knowledge_doc_download` saved selected. `path` must be exactly
/// what the save dialog returned for a download of this run (the folders `reveal_path` allows are
/// not widened for downloads), else it is `forbidden`. `notFound` when the file is gone since.
#[tauri::command]
pub async fn knowledge_reveal_download(state: State<'_, AppState>, path: String) -> CommandResult<()> {
    let target = state.saved_downloads.reveal_target(Path::new(&path))?;

    blocking(move || {
        tauri_plugin_opener::reveal_item_in_dir(&target)
            .map_err(|error| BusinessError::Process(format!("cannot open the file manager: {error}")).into())
    })
    .await
}

/// The file name the save dialog suggests: the last part of `name` (after a `/` or a `\`) without the characters
/// [`display_text`] drops (control, invisible and direction-changing ones, which could disguise an
/// extension) and without leading dots (no hidden file), else [`DEFAULT_DOWNLOAD_NAME`].
fn download_name(name: Option<&str>) -> String {
    // Both separators count on every system: a name from another computer may carry either.
    let last = name
        .and_then(|name| name.trim().rsplit(['/', '\\']).next())
        .unwrap_or_default();

    let cleaned = display_text(last, DOWNLOAD_NAME_MAX_CHARS);
    let visible = cleaned.trim_start_matches('.').trim();

    if visible.is_empty() {
        DEFAULT_DOWNLOAD_NAME.to_owned()
    } else {
        visible.to_owned()
    }
}

/// Whether team approval is on, where it is kept (the team database, else this computer, which is
/// always off) and whether the user may change it. `unsupported` for a server without `docs-approval`.
#[tauri::command]
pub async fn knowledge_approval_get(state: State<'_, AppState>) -> CommandResult<KnowledgeTeamApproval> {
    let server = approval_server(&state.env, &state.runner, &state.knowledge_features).await?;

    Ok(get_approval(&state.env, &state.runner, &server).await?)
}

/// Turns team approval on or off for the team database. Turning it on changes no document, turning
/// it off leaves waiting documents waiting, and their authors can make them live. `readOnly` is
/// answered for a role that may not change it, `refused` with no team database.
#[tauri::command]
pub async fn knowledge_approval_set(
    state: State<'_, AppState>,
    on: bool,
) -> CommandResult<KnowledgeOutcome<KnowledgeTeamApproval>> {
    let server = approval_server(&state.env, &state.runner, &state.knowledge_features).await?;

    Ok(set_approval(&state.env, &state.runner, &server, on).await?)
}

/// Asks the agent of a proposal for changes: the proposal stays in the review queue marked
/// `changesRequested` with the `comment` (1 to 500 characters), only while its `revision` is the
/// one shown. When the chat of that agent is still open in this app, the comment is also sent to it
/// as a follow-up message and `delivered` says so. `invalidInput` for an id or revision that is not
/// one.
#[tauri::command]
pub async fn knowledge_held_request_changes(
    state: State<'_, AppState>,
    id: String,
    revision: Option<String>,
    comment: String,
) -> CommandResult<KnowledgeChangesRequested> {
    let server = knowledge_held_server(&state.env, &state.runner, &state.knowledge_features).await?;

    // The session and the title are the server's, not the page's.
    let proposal = get_knowledge_held(&state.env, &state.runner, &server, &id).await?;

    let decided =
        request_changes_knowledge_held(&state.env, &state.runner, &server, &id, revision.as_deref(), &comment).await?;

    let delivered = match (&decided, &proposal.session) {
        (KnowledgeDecided::ChangesRequested, Some(session)) => {
            knowledge_follow_up::deliver(&state, session, &proposal, comment.trim()).await
        }
        _ => false,
    };

    Ok(KnowledgeChangesRequested { decided, delivered })
}

/// Whether the agent that made a proposal can be told what to change: the `session` the proposal
/// records belongs to a chat that is open in this app and works in `project_path`, the folder the
/// proposal was about. False without a session or a folder, and for a chat of another project.
#[tauri::command]
pub fn knowledge_held_follow_up_available(
    state: State<'_, AppState>,
    session: Option<String>,
    project_path: Option<String>,
) -> bool {
    session
        .filter(|session| (1..=SESSION_MAX_CHARS).contains(&session.chars().count()))
        .is_some_and(|session| knowledge_follow_up::live_chat(&state, &session, project_path.as_deref()).is_some())
}

/// The page showed the notice that running harnesses must be restarted after Harness Knowledge
/// upgraded its database, and the user dismissed it: the pulse stops carrying it.
#[tauri::command]
pub fn knowledge_restart_notice_dismiss(state: State<'_, AppState>) {
    state.knowledge_pulse.dismiss_restart_notice();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_suggested_name_is_a_file_name_without_a_folder() {
        assert_eq!(download_name(Some("billing-openapi.yaml")), "billing-openapi.yaml");
        assert_eq!(download_name(Some(" docs/api/guide.md ")), "guide.md");
        assert_eq!(download_name(Some("../../etc/passwd")), "passwd");
        assert_eq!(download_name(Some("..\\docs\\guide.md")), "guide.md");
        assert_eq!(download_name(Some("")), DEFAULT_DOWNLOAD_NAME);
        assert_eq!(download_name(Some("..")), DEFAULT_DOWNLOAD_NAME);
        assert_eq!(download_name(None), DEFAULT_DOWNLOAD_NAME);
    }

    #[test]
    fn the_suggested_name_is_never_hidden_and_never_disguised() {
        assert_eq!(download_name(Some(".bashrc")), "bashrc");
        assert_eq!(download_name(Some("...")), DEFAULT_DOWNLOAD_NAME);
        assert_eq!(download_name(Some(".")), DEFAULT_DOWNLOAD_NAME);
        // A right-to-left override would show `a\u{202E}gpj.sh` as `ash.jpg`.
        assert_eq!(download_name(Some("a\u{202E}gpj.sh")), "agpj.sh");
        assert_eq!(download_name(Some("x\u{200B}.md")), "x.md");
        assert_eq!(download_name(Some("a\u{2066}b\u{2069}.md")), "ab.md");
        assert_eq!(download_name(Some("bad\nname.md")), "badname.md");
        assert_eq!(download_name(Some("\u{202E}\u{200B}")), DEFAULT_DOWNLOAD_NAME);
    }
}
