//! The documents agents proposed through `knowledge_add`, waiting in this computer's review queue
//! (`knowledge_held_list`, `knowledge_held_get`, `knowledge_held_decide`), and the counts the live
//! view follows ([`knowledge_pulse`]).
//!
//! The queue belongs to the server: the app reads and settles it only through
//! `harness-knowledge held list|get|approve|reject --json`, and needs `held` in the server's
//! features. The commands are exactly:
//!
//! - `held list --json` prints `{"held": [...]}` (at most 100, newest first, each with a `preview`
//!   of its text), `held get --json ID` prints `{"held": {...}}` with the whole `content`. The
//!   queue is small and has no paging of its own, so a [`ListWindow`] filters it (`query` finds
//!   its words in the title or the file name), orders it and cuts the page here.
//! - `held approve --json ID [--expect REVISION] [--place remote|local]` prints `{"approved": {heldId, docId, status,
//!   title, chunks, embedding}}`, `held reject --json ID [--expect REVISION]` `{"rejected": ID}`.
//!   The id is the proposal's UUID, the revision the one its item showed.
//! - A refusal exits 1 and prints `{"error": "notFound" | "changed" | "refused", "message"}` on
//!   stdout: [`decide_knowledge_held`] answers it as a [`KnowledgeDecided`] outcome, [`get_knowledge_held`]
//!   as `notFound`. Any other failure is an error.
//!
//! What the server prints is read strictly: an item that does not have every field fails the read
//! (`protocol`) instead of being skipped. Neither the proposals' text nor their titles are logged.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use super::catalog::KNOWLEDGE_SIDECAR;
use super::knowledge_cli::{
    CliCall, CliFailure, availability_of, availability_of_failure, call, field, run_json, run_typed, words,
};
use super::knowledge_docs::{docs_server, read_stats};
use super::knowledge_features::{KnowledgeFeatures, KnowledgeSupport};
use super::sidecar_cli::{CliAnswer, ListWindow, USAGE_EXIT, clean, run_sidecar_cli};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{
    KnowledgeApproval, KnowledgeAvailability, KnowledgeDecided, KnowledgeDecision, KnowledgeEmbedding,
    KnowledgeHeldItem, KnowledgeHeldPage, KnowledgeId, KnowledgeLink, KnowledgePlace, KnowledgePulse,
};
use crate::process::ProcessRunner;

/// The exit code of a command that refused (see the module docs).
const REFUSED_EXIT: i32 = 1;

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
) -> KnowledgeHeldPage {
    let server = match knowledge_held_server(env, runner, features).await {
        Ok(server) => server,
        Err(error) => return KnowledgeHeldPage::unavailable(availability_of(error)),
    };

    match read_page(env, runner, &server, window).await {
        Ok(page) => page,
        Err(failure) => KnowledgeHeldPage::unavailable(availability_of_failure(failure)),
    }
}

async fn read_page(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    window: &ListWindow,
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

    if !window.descending(true) {
        items.reverse();
    }

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

    match run_held(env, runner, server, call("held get", args)).await? {
        Answer::Done(data) => parse_item(&field::<Value>(&data, "held", "held get")?, "content", "held get"),
        Answer::Refused(Refusal::NotFound, message) => Err(BusinessError::NotFound(message)),
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

    let data = match run_held(env, runner, server, call(name, args)).await? {
        Answer::Done(data) => data,
        Answer::Refused(Refusal::NotFound, message) => return Ok(KnowledgeDecided::NotFound { message }),
        Answer::Refused(Refusal::Changed, message) => return Ok(KnowledgeDecided::Changed { message }),
        Answer::Refused(Refusal::Refused, message) => return Ok(KnowledgeDecided::Refused { message }),
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
            })
        }
        KnowledgeDecision::Reject => {
            field::<String>(&data, "rejected", name)?;

            Ok(KnowledgeDecided::Rejected)
        }
    }
}

/// The counts the live view follows, all from one `stats --json` call: the documents, the
/// generation and how many proposals wait (`held`). A server that predates that field is asked for
/// `held list --json` instead, while it has a review queue. `None` while the server cannot answer
/// `stats`.
pub async fn knowledge_pulse(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &KnowledgeFeatures,
) -> Option<KnowledgePulse> {
    let server = docs_server(env, runner, features).await.ok()?;
    let stats = read_stats(env, runner, &server).await.ok()?;

    let held = match stats.held {
        Some(held) => Some(held),
        None => match knowledge_held_server(env, runner, features).await {
            Ok(server) => held_total(env, runner, &server).await,
            Err(_) => None,
        },
    };

    Some(KnowledgePulse {
        documents: stats.documents,
        generation: stats.generation,
        held,
    })
}

/// How many proposals wait: the length of `held list`'s `held`.
async fn held_total(env: &Env, runner: &dyn ProcessRunner, server: &Path) -> Option<u32> {
    let data = run_json(
        env,
        runner,
        server,
        call("held list", words(&["held", "list", "--json"])),
    )
    .await
    .ok()?;

    let held = data.get("held")?.as_array()?;

    Some(u32::try_from(held.len()).unwrap_or(u32::MAX))
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
    })
}

/// What a command answered: its object, or its refusal.
enum Answer {
    Done(Value),
    Refused(Refusal, String),
}

/// The `error` codes of a refusal.
enum Refusal {
    NotFound,
    Changed,
    Refused,
}

/// Runs `call` like [`run_typed`], except that a refusal (exit 1 and `{"error": CODE, "message"}`
/// on stdout) is an [`Answer::Refused`] with the server's sentence (redacted).
async fn run_held(env: &Env, runner: &dyn ProcessRunner, server: &Path, call: CliCall<'_>) -> Result<Answer> {
    let name = call.name.to_owned();

    match run_sidecar_cli(&KNOWLEDGE_SIDECAR, env, runner, server, call).await? {
        CliAnswer::Done(stdout) => serde_json::from_str(&stdout).map(Answer::Done).map_err(|_| {
            BusinessError::Protocol(format!(
                "{} {name} answered output this app cannot read",
                KNOWLEDGE_SIDECAR.binary
            ))
        }),
        CliAnswer::Failed {
            status: Some(REFUSED_EXIT),
            message,
            stdout,
        } => match refusal_of(&stdout) {
            Some((refusal, sentence)) => Ok(Answer::Refused(refusal, sentence.unwrap_or(message))),
            None => Err(BusinessError::Process(message)),
        },
        CliAnswer::Failed {
            status: Some(USAGE_EXIT),
            ..
        } => Err(BusinessError::Unsupported(format!(
            "this {} predates `{name}`, so update {}",
            KNOWLEDGE_SIDECAR.binary, KNOWLEDGE_SIDECAR.product
        ))),
        CliAnswer::Failed { message, .. } => Err(BusinessError::Process(message)),
    }
}

/// The refusal and the server's sentence in `{"error": CODE, "message"}`; `None` for any other
/// output.
fn refusal_of(stdout: &str) -> Option<(Refusal, Option<String>)> {
    let data: Value = stdout
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with('{'))
        .and_then(|line| serde_json::from_str(line).ok())?;

    let refusal = match data.get("error")?.as_str()? {
        "notFound" => Refusal::NotFound,
        "changed" => Refusal::Changed,
        "refused" => Refusal::Refused,
        _ => return None,
    };

    Some((refusal, data.get("message").and_then(Value::as_str).map(clean)))
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
