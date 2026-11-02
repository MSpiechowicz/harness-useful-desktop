//! The stored documents the files of an import selection are probably versions of
//! ([`similar_docs`], `knowledge_similar`), through `harness-knowledge docs similar --json
//! [--repo KEY | --global] PATH...` (15 s per call, see [`call`]).
//!
//! - Needs `docs-similar`: `unsupported` for a server without it, `notFound` without the server.
//! - The paths are the selection's own, taken back from the intake by its token
//!   ([`KnowledgeIntakes::get`]): the caller never names one, so nothing else is read. A folder is
//!   given to the server as it is and expands there, as it does in the import.
//! - The link scope is the import's: the repository of its first link, else global (a title only
//!   matches inside its scope).
//! - Many paths go in calls of at most [`BATCH_FILES`] paths and [`BATCH_BYTES`] bytes of arguments,
//!   so no command line is too long for the OS.
//! - The answer is read strictly: a shape this app does not know, a document id that is no
//!   document's, a path over [`MAX_PATH_CHARS`] or more than [`MAX_ENTRIES`] entries is a `protocol`
//!   error. The strings shown are cleaned (a title and an error without control and invisible
//!   characters, an error also redacted and clipped).

use std::ffi::OsString;

use serde_json::Value;

use super::knowledge_cli::{call, run_json, words};
use super::knowledge_docs::check_links;
use super::knowledge_features::{KnowledgeFeatures, KnowledgeSupport};
use super::knowledge_intake::{IntakePaths, KnowledgeIntakes, MAX_INTAKE_FILES};
use super::sidecar_cli::clean;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{KnowledgeLink, KnowledgeSimilar, KnowledgeSimilarEntry, display_text};
use crate::process::ProcessRunner;

/// The most paths one call is given.
const BATCH_FILES: usize = 50;

/// The most bytes of paths one call is given.
const BATCH_BYTES: usize = 16 * 1024;

/// The most entries kept of all calls: the most files an import takes.
pub const MAX_ENTRIES: usize = MAX_INTAKE_FILES;

/// The longest path of an entry, in characters.
const MAX_PATH_CHARS: usize = 4096;

/// The longest title of a match, in characters.
const MAX_TITLE_CHARS: usize = 300;

/// The longest time text of a match (RFC 3339 is 35 at most), in characters.
const MAX_TIME_CHARS: usize = 40;

/// The output kept of each call: 5000 entries with a match are about 2 MiB.
const OUTPUT_BYTES: usize = 4 * 1024 * 1024;

/// What the files of intake `token` are probably versions of, one entry per file in the order of
/// the selection (see the module docs). `notFound` for a token that is gone, `invalidInput` for
/// links the server would refuse.
pub async fn similar_docs(
    env: &Env,
    runner: &dyn ProcessRunner,
    features: &KnowledgeFeatures,
    intakes: &KnowledgeIntakes,
    token: &str,
    links: &[KnowledgeLink],
) -> Result<KnowledgeSimilar> {
    check_links(links)?;

    let paths = intakes.get(token)?;
    let server = features
        .feature_server(env, runner, KnowledgeSupport::docs_similar, "finding similar documents")
        .await?;

    let mut similar = KnowledgeSimilar::default();

    for batch in batches(&paths) {
        let mut command = call("docs similar", similar_args(links, batch));
        command.output_bytes = OUTPUT_BYTES;

        let data = run_json(env, runner, &server, command).await?;
        read_batch(&data, &mut similar)?;
    }

    Ok(similar)
}

/// `docs similar --json [--repo KEY | --global] PATH...`. The repository key is checked by
/// [`check_links`].
fn similar_args(links: &[KnowledgeLink], paths: Vec<&str>) -> Vec<OsString> {
    let mut args = words(&["docs", "similar", "--json"]);

    match links.first() {
        Some(link) => args.extend(["--repo".into(), link.repo.clone().into()]),
        None => args.push("--global".into()),
    }

    args.extend(paths.into_iter().map(OsString::from));

    args
}

/// The files and folders of the selection in calls that fit the limits.
fn batches(paths: &IntakePaths) -> Vec<Vec<&str>> {
    let mut batches = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut bytes = 0;

    for path in paths.files.iter().chain(&paths.folders).map(String::as_str) {
        if !current.is_empty() && (current.len() >= BATCH_FILES || bytes + path.len() > BATCH_BYTES) {
            batches.push(std::mem::take(&mut current));
            bytes = 0;
        }

        bytes += path.len();
        current.push(path);
    }

    if !current.is_empty() {
        batches.push(current);
    }

    batches
}

/// Adds the entries of one answer to `similar`.
fn read_batch(data: &Value, similar: &mut KnowledgeSimilar) -> Result<()> {
    let unreadable =
        || BusinessError::Protocol("harness-knowledge docs similar answered what this app cannot read".to_owned());

    let entries = data.get("similar").and_then(Value::as_array).ok_or_else(unreadable)?;

    if similar.items.len() + entries.len() > MAX_ENTRIES {
        return Err(BusinessError::Protocol(
            "harness-knowledge docs similar answered more files than an import takes".to_owned(),
        ));
    }

    for entry in entries {
        let entry: KnowledgeSimilarEntry = serde_json::from_value(entry.clone()).map_err(|_| unreadable())?;
        similar.items.push(cleaned(entry).ok_or_else(unreadable)?);
    }

    similar.offline |= data.get("offline").and_then(Value::as_bool).unwrap_or(false);

    Ok(())
}

/// The most characters of an error read before it is redacted and clipped.
const MAX_ERROR_CHARS: usize = 2000;

/// `error` on one line without control and invisible characters (a line break becomes a space), redacted and clipped.
fn clean_error(error: &str) -> String {
    let one_line = error.replace(['\r', '\n', '\t'], " ");

    clean(&display_text(&one_line, MAX_ERROR_CHARS))
}

/// `entry` with its strings cleaned, `None` when it is out of bounds (a path or time too long, an
/// id that is no document's).
fn cleaned(mut entry: KnowledgeSimilarEntry) -> Option<KnowledgeSimilarEntry> {
    if entry.path.chars().count() > MAX_PATH_CHARS {
        return None;
    }

    entry.error = entry.error.as_deref().map(clean_error);

    if let Some(found) = entry.found.as_mut() {
        if !found.id.is_document() || found.updated_at.chars().count() > MAX_TIME_CHARS {
            return None;
        }

        found.title = display_text(&found.title, MAX_TITLE_CHARS);
    }

    Some(entry)
}
