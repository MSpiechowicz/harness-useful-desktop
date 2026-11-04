//! The documents agents proposed through `knowledge_add`, waiting in this computer's review queue
//! (`knowledge_held_list`, `knowledge_held_get`, `knowledge_held_decide`,
//! `knowledge_held_request_changes`); the counts the live view follows are
//! [`knowledge_pulse`](super::knowledge_pulse)'s.
//!
//! The queue belongs to the server: the app reads and settles it only through
//! `harness-knowledge held list|get|approve|reject|request-changes --json`, and needs `held` in the
//! server's features. The commands are exactly:
//!
//! - `held list --json` prints `{"held": [...]}` (at most 100, newest first, each with a `preview`
//!   of its text), `held get --json ID` prints `{"held": {...}}` with the whole `content`. An item
//!   carries `addedBy` (the person it is written for), and after a request for changes `status:
//!   "changesRequested"` with the `comment`. The queue is small and has no paging of its own, so a
//!   [`ListWindow`] filters it (`query` finds its words in the title or the file name), orders it and
//!   cuts the page here.
//! - `held approve --json ID [--expect REVISION] [--place remote|local]` prints `{"approved": {heldId, docId, status,
//!   title, chunks, embedding, draft}}` (the status may be `pending`, `draft` is true while team
//!   approval is on), `held reject --json ID [--expect REVISION] [--comment TEXT]` `{"rejected": ID}`,
//!   `held request-changes --json ID [--expect REVISION] --comment TEXT` marks the item `changesRequested`.
//!   The id is the proposal's UUID, the revision the one its item showed.
//! - A refusal exits 1 and prints `{"error": "notFound" | "changed" | "commentRequired" | "refused",
//!   "message"}` on stdout: [`decide_knowledge_held`] answers it as a [`KnowledgeDecided`] outcome,
//!   [`get_knowledge_held`] as `notFound`. Any other failure is an error.
//!
//! What the server prints is read strictly: an item that does not have every field fails the read
//! (`protocol`) instead of being skipped. Neither the proposals' text nor their titles are logged.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use super::catalog::KNOWLEDGE_SIDECAR;
use super::knowledge_cli::{CliFailure, availability_of, availability_of_failure, call, field, run_typed, words};
use super::knowledge_features::{KnowledgeFeatures, KnowledgeSupport};
use super::knowledge_held_window::{HeldSort, in_scope, sort_items};
use super::knowledge_refusal::{Answer, run_refusing};
use super::sidecar_cli::ListWindow;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{
    KnowledgeApproval, KnowledgeAvailability, KnowledgeDecided, KnowledgeDecision, KnowledgeEmbedding,
    KnowledgeHeldItem, KnowledgeHeldPage, KnowledgeHeldStatus, KnowledgeId, KnowledgeLink, KnowledgePlace,
    KnowledgeRefusalCode, KnowledgeScope, MAX_AUTHOR_NAME_CHARS, display_text,
};
use crate::process::ProcessRunner;

/// The most characters of the comment of a rejection or of a request for changes.
const COMMENT_MAX_CHARS: usize = 500;

/// The installed server when it lists `held`.
pub async fn knowledge_held_server(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &KnowledgeFeatures,
) -> Result<PathBuf> {
    features
        .feature_server(env, runner, KnowledgeSupport::held, "the review queue")
        .await
}

/// One page of the review queue, newest first unless the window asks otherwise. Never fails: what
/// went wrong is the page's availability.
pub async fn list_knowledge_held(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &KnowledgeFeatures,
    window: &ListWindow,
    sort: HeldSort,
    scope: &KnowledgeScope,
) -> KnowledgeHeldPage {
    let server = match knowledge_held_server(env, runner, features).await {
        Ok(server) => server,
        Err(error) => return KnowledgeHeldPage::unavailable(availability_of(error)),
    };

    match read_page(env, runner, &server, window, sort, scope).await {
        Ok(page) => page,
        Err(failure) => KnowledgeHeldPage::unavailable(availability_of_failure(failure)),
    }
}

async fn read_page(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    window: &ListWindow,
    sort: HeldSort,
    scope: &KnowledgeScope,
) -> std::result::Result<KnowledgeHeldPage, CliFailure> {
    let data = run_typed(
        env,
        runner,
        server,
        call("held list", words(&["held", "list", "--json"])),
    )
    .await?;

    let mut items = field::<Vec<Value>>(&data, "held", "held list")?
        .iter()
        .map(|entry| parse_item(entry, "preview", "held list"))
        .collect::<Result<Vec<_>>>()?;

    if let Some(query) = window.query() {
        let query = query.to_lowercase();

        items.retain(|item| item.title.to_lowercase().contains(&query) || item.name.to_lowercase().contains(&query));
    }

    items.retain(|item| in_scope(item, scope));
    sort_items(&mut items, sort, window.descending(true));

    let total = u32::try_from(items.len()).unwrap_or(u32::MAX);

    Ok(KnowledgeHeldPage {
        availability: KnowledgeAvailability::Available,
        items: window.page_of(items),
        total,
    })
}

/// One proposal with all of its text (`held get --json ID`). `notFound` when there is no such
/// proposal (or another decision took it).
pub async fn get_knowledge_held(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    id: &str,
) -> Result<KnowledgeHeldItem> {
    let mut args = words(&["held", "get", "--json"]);
    args.push(check_id(id)?.into());

    match run_refusing(env, runner, server, call("held get", args)).await? {
        Answer::Done(data) => parse_item(&field::<Value>(&data, "held", "held get")?, "content", "held get"),
        Answer::Refused(KnowledgeRefusalCode::NotFound, message) => Err(BusinessError::NotFound(message)),
        Answer::Refused(_, message) => Err(BusinessError::Process(message)),
    }
}

/// Approves (imports) or rejects (throws away) proposal `id` (`held approve|reject --json ID
/// [--expect REVISION]`). With `expect` (the `revision` the item showed) only that version is
/// decided: another one is [`KnowledgeDecided::Changed`]. A refusal is an outcome, not an error.
pub async fn decide_knowledge_held(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    id: &str,
    decision: KnowledgeDecision,
    expect: Option<&str>,
    place: Option<KnowledgePlace>,
) -> Result<KnowledgeDecided> {
    let extras = DecisionExtras {
        expect,
        place,
        comment: None,
    };

    decide(env, runner, server, id, decision, &extras).await
}

/// Rejects proposal `id` like [`decide_knowledge_held`] and tells its agent why (`held reject
/// --comment TEXT`, 1 to 500 characters).
pub async fn reject_knowledge_held(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    id: &str,
    expect: Option<&str>,
    comment: &str,
) -> Result<KnowledgeDecided> {
    let extras = DecisionExtras {
        expect,
        place: None,
        comment: Some(comment),
    };

    decide(env, runner, server, id, KnowledgeDecision::Reject, &extras).await
}

/// What a decision carries besides its proposal.
#[derive(Clone, Copy)]
struct DecisionExtras<'a> {
    expect: Option<&'a str>,
    place: Option<KnowledgePlace>,
    comment: Option<&'a str>,
}

async fn decide(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    id: &str,
    decision: KnowledgeDecision,
    extras: &DecisionExtras<'_>,
) -> Result<KnowledgeDecided> {
    let DecisionExtras { expect, place, comment } = *extras;
    let (verb, name) = match decision {
        KnowledgeDecision::Approve => ("approve", "held approve"),
        KnowledgeDecision::Reject => ("reject", "held reject"),
    };

    let mut args = words(&["held", verb, "--json"]);
    args.push(check_id(id)?.into());

    if let Some(revision) = expect {
        args.extend(["--expect".into(), check_revision(revision)?.into()]);
    }

    if let Some(place) = place.filter(|_| decision == KnowledgeDecision::Approve) {
        let name = match place {
            KnowledgePlace::Remote => "remote",
            KnowledgePlace::Local => "local",
        };

        args.extend(["--place".into(), name.into()]);
    }

    if let Some(comment) = comment.filter(|_| decision == KnowledgeDecision::Reject) {
        args.extend(["--comment".into(), check_comment(comment)?.into()]);
    }

    let data = match run_refusing(env, runner, server, call(name, args)).await? {
        Answer::Done(data) => data,
        Answer::Refused(code, message) => return Ok(refused_outcome(code, message)),
    };

    tracing::info!(verb, "a proposed knowledge document was decided");

    match decision {
        KnowledgeDecision::Approve => {
            let approved: Approved = field(&data, "approved", name)?;

            Ok(KnowledgeDecided::Approved {
                doc_id: approved.doc_id,
                status: approved.status,
                title: approved.title,
                chunks: approved.chunks,
                embedding: approved.embedding,
                draft: approved.draft,
            })
        }
        KnowledgeDecision::Reject => {
            field::<String>(&data, "rejected", name)?;

            Ok(KnowledgeDecided::Rejected)
        }
    }
}

/// Asks the agent of proposal `id` for changes (`held request-changes --json ID --expect REVISION
/// --comment TEXT`): the proposal stays in the queue as `changesRequested` with the comment. The
/// comment is 1 to 500 characters (an empty one is [`KnowledgeDecided::CommentRequired`] without
/// asking the server). A refusal is an outcome, not an error.
pub async fn request_changes_knowledge_held(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    id: &str,
    expect: Option<&str>,
    comment: &str,
) -> Result<KnowledgeDecided> {
    let mut args = words(&["held", "request-changes", "--json"]);
    args.push(check_id(id)?.into());

    if let Some(revision) = expect {
        args.extend(["--expect".into(), check_revision(revision)?.into()]);
    }

    if comment.trim().is_empty() {
        return Ok(KnowledgeDecided::CommentRequired {
            message: "say what should change".to_owned(),
        });
    }

    args.extend(["--comment".into(), check_comment(comment)?.into()]);

    match run_refusing(env, runner, server, call("held request-changes", args)).await? {
        Answer::Done(_) => {
            tracing::info!("changes were requested on a proposed knowledge document");

            Ok(KnowledgeDecided::ChangesRequested)
        }
        Answer::Refused(code, message) => Ok(refused_outcome(code, message)),
    }
}

/// The outcome a refusal of a decision is: the codes of the queue, any other as `refused`.
fn refused_outcome(code: KnowledgeRefusalCode, message: String) -> KnowledgeDecided {
    match code {
        KnowledgeRefusalCode::NotFound => KnowledgeDecided::NotFound { message },
        KnowledgeRefusalCode::Changed => KnowledgeDecided::Changed { message },
        KnowledgeRefusalCode::CommentRequired => KnowledgeDecided::CommentRequired { message },
        KnowledgeRefusalCode::LocalPath => KnowledgeDecided::LocalPath { message },
        _ => KnowledgeDecided::Refused { message },
    }
}

/// `comment` trimmed, when it has 1 to 500 characters.
fn check_comment(comment: &str) -> Result<&str> {
    let comment = comment.trim();
    let length = comment.chars().count();

    if (1..=COMMENT_MAX_CHARS).contains(&length) {
        Ok(comment)
    } else {
        Err(BusinessError::InvalidInput(format!(
            "a comment has 1 to {COMMENT_MAX_CHARS} characters"
        )))
    }
}

/// `place` for `held approve --place`, or none when the server cannot route documents (it predates
/// `routing`, so it knows no `--place` and keeps everything in one place).
pub async fn approval_place(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &KnowledgeFeatures,
    place: Option<KnowledgePlace>,
) -> Option<KnowledgePlace> {
    if place.is_some() && features.support(env, runner).await.routing() {
        place
    } else {
        None
    }
}

/// What `held approve` prints under `approved`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Approved {
    doc_id: KnowledgeId,
    status: KnowledgeApproval,
    title: String,
    chunks: Option<u32>,
    embedding: Option<KnowledgeEmbedding>,
    /// Stored as a draft because team approval is on (absent from a server without `docs-drafts`).
    #[serde(default)]
    draft: bool,
}

/// One proposal as `held list` and `held get` print it, without the text.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HeldFields {
    held_id: String,
    title: String,
    format: String,
    name: String,
    bytes: u64,
    links: Vec<KnowledgeLink>,
    client: String,
    session: Option<String>,
    project_path: Option<String>,
    repo_key: Option<String>,
    source_path: Option<String>,
    replaces: Option<KnowledgeId>,
    revision: String,
    created_at: String,
    /// The person it is written for: a name, or an author object of a server that prints one.
    #[serde(default)]
    added_by: Option<Value>,
    /// `changesRequested` once the user asked for changes, absent (waiting) before.
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    comment: Option<String>,
}

/// A proposal from the server's JSON, its text under `text_key` (`preview` in a list, `content` in
/// a get). `protocol` (naming `command`) when it does not have every field.
fn parse_item(entry: &Value, text_key: &str, command: &str) -> Result<KnowledgeHeldItem> {
    let unreadable = || {
        BusinessError::Protocol(format!(
            "{} {command} answered a proposal this app cannot read",
            KNOWLEDGE_SIDECAR.binary
        ))
    };

    let fields: HeldFields = serde_json::from_value(entry.clone()).map_err(|_| unreadable())?;
    let text = entry.get(text_key).and_then(Value::as_str).ok_or_else(unreadable)?;

    Ok(KnowledgeHeldItem {
        id: fields.held_id,
        title: fields.title,
        format: fields.format,
        name: fields.name,
        bytes: fields.bytes,
        text: text.to_owned(),
        links: fields.links,
        client: fields.client,
        session: fields.session,
        project_path: fields.project_path,
        repo_key: fields.repo_key,
        source_path: fields.source_path,
        replaces: fields.replaces,
        revision: fields.revision,
        created_at: fields.created_at,
        added_by: fields.added_by.as_ref().and_then(author_name),
        status: match fields.status.as_deref() {
            Some("changesRequested") => KnowledgeHeldStatus::ChangesRequested,
            _ => KnowledgeHeldStatus::Waiting,
        },
        comment: fields
            .comment
            .map(|comment| display_text(&comment, COMMENT_MAX_CHARS))
            .filter(|comment| !comment.is_empty()),
    })
}

/// The cleaned name in `addedBy`: the server prints the name as text, a server that prints the
/// author object `{"name", "via"}` is read too. `None` for anything else and for an empty name.
fn author_name(value: &Value) -> Option<String> {
    let name = match value {
        Value::String(name) => name,
        Value::Object(author) => author.get("name")?.as_str()?,
        _ => return None,
    };

    Some(display_text(name, MAX_AUTHOR_NAME_CHARS)).filter(|name| !name.is_empty())
}

/// `id` when it is a proposal id: a lowercase UUID (`8-4-4-4-12` hex digits).
fn check_id(id: &str) -> Result<&str> {
    let groups: Vec<&str> = id.split('-').collect();
    let fine = groups.len() == 5
        && groups.iter().zip([8, 4, 4, 4, 12]).all(|(group, length)| {
            group.len() == length
                && group
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        });

    if fine {
        Ok(id)
    } else {
        Err(BusinessError::InvalidInput("that is not a proposal id".to_owned()))
    }
}

/// `revision` when it is one: 64 lowercase hex digits (a SHA-256).
fn check_revision(revision: &str) -> Result<&str> {
    let fine = revision.len() == 64
        && revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));

    if fine {
        Ok(revision)
    } else {
        Err(BusinessError::InvalidInput(
            "that is not a proposal revision".to_owned(),
        ))
    }
}
