//! The user's own saves through the commands of a server with `save-guards` (see
//! [`memory_features`](super::memory_features)), which agents cannot reach through MCP, so no save
//! guard applies to them. Each reads its draft as JSON on stdin and answers JSON:
//!
//! - `notes save --json --client harness-desktop [--project P]` takes `{title, body, kind, tags, entities, scope}` and
//!   answers `{"id", "title", "scope"}`;
//! - `notes update <id> --json --client harness-desktop [--project P] [--expect-revision N]` takes `{title, body,
//!   kind, tags}` and answers `{"id", "title"}` (`title` is `null` when the change gives none); while the
//!   team database cannot be reached a change to a team note is kept on this computer and answered
//!   `{"id", "title", "queued": true, "message"}`, which is [`NoteSaved::queued`]; a note it cannot see (a repository note needs `--project`) is
//!   `notFound`. With `--expect-revision` (a server that lists `note-expect-revision`) it saves only the note at
//!   revision `N`: another revision is refused `changed` (and a note that is gone `notFound`), which the app
//!   answers as a `conflict`;
//! - `notes save` and `scripts save` take `--place remote|local` (a server that lists `save-place`, see
//!   [`SaveRoute::places`]) for a new entry of a scope routed `remote+local`, and answer `"place"` with where it
//!   went ([`NoteSaved::place`]); a team database that cannot be reached is refused with its own text and nothing
//!   is saved;
//! - `scripts save --json --client harness-desktop [--project P]` takes `{name, description, language, body, timeoutSecs,
//!   scope}` and answers `{"name", "scope", "id", "created", "approval"}`, trusted as
//!   `script_save` trusts it (never code waiting for approval: the caller approves the user's code);
//! - `held save|skip` ([`memory_held`](super::memory_held)) use the same answers; a held change to a team
//!   note kept offline is `{"saved": {"kind": "note", "id", "queued": true, "message"}}`
//!   ([`HeldSaved::Note`](crate::model::HeldSaved::Note)).
//!
//! A refusal prints `{"error": CODE, "message"}` on stdout and exits 1, `CODE` one of `notFound`,
//! `gone`, `invalid` and `refused` (see [`refusal`]); a title another note has is `refused` with
//! the tool's own text (`note <id> already has this title …`). Each names the app as its client
//! ([`CLIENT_NAME`]), so these saves show the author MCP saves show. A server without `save-guards` saves through MCP as
//! before ([`SaveRoute::Mcp`]).

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use serde_json::{Map, Value};

use super::catalog::MEMORY_BINARY;
use super::mcp_client::CLIENT_NAME;
use super::memory_cli::{CliAnswer, CliCall, LIST_OUTPUT_BYTES, USAGE_EXIT, clean, project_args, run_memory_cli};
use super::memory_features::FeaturesSupport;
use super::memory_notes_save::taken_title;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{MemorySavePlace, NoteSaveOutcome, NoteSaved};
use crate::process::ProcessRunner;

/// How long one save may take (30 s: a team database far away).
pub const SAVE_TIMEOUT: Duration = Duration::from_secs(30);

/// What the app says for a held note change whose note was deleted since (`gone`).
pub const GONE_MESSAGE: &str = "the note was deleted after the agent changed it, so nothing was saved";

/// What the app says for a held save the agent changed after the user saw it (`changed`).
pub const CHANGED_MESSAGE: &str = "the agent changed this save after you saw it, so nothing was saved";

/// What the app says for an edit of a note that changed, or was deleted, since the editor opened it.
pub const NOTE_CHANGED_MESSAGE: &str = "the note changed since you opened it";

/// How the user's notes and scripts are saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveRoute {
    /// Through the pooled `harness-memory mcp` connection (a server without `save-guards`).
    Mcp,
    /// Through the user-only commands (a server with `save-guards`).
    Cli,
    /// [`SaveRoute::Cli`] of a server that also lists `note-expect-revision`: an edit of a note
    /// names the revision the editor opened, and the server refuses it when the note is another.
    CliExpectRevision,
    /// [`SaveRoute::Cli`] of a server that also lists `save-place`: a new note or script may name
    /// where it goes (`--place`).
    CliPlace,
    /// [`SaveRoute::CliExpectRevision`] of a server that also lists `save-place`.
    CliExpectRevisionPlace,
}

impl SaveRoute {
    /// The route of a server that lists `save-guards`, with `note-expect-revision` and
    /// `save-place` when it lists them too; [`SaveRoute::Mcp`] when it lacks `save-guards`.
    pub fn of(features: FeaturesSupport) -> Self {
        if !features.save_guards() {
            return Self::Mcp;
        }

        match (features.note_expect_revision(), features.save_place()) {
            (false, false) => Self::Cli,
            (true, false) => Self::CliExpectRevision,
            (false, true) => Self::CliPlace,
            (true, true) => Self::CliExpectRevisionPlace,
        }
    }

    /// Whether the user's saves go through the user-only commands.
    pub fn through_cli(self) -> bool {
        self != Self::Mcp
    }

    /// Whether an edit of a note may name the revision it opened.
    pub fn expects_revision(self) -> bool {
        matches!(self, Self::CliExpectRevision | Self::CliExpectRevisionPlace)
    }

    /// Whether a new note or script may name where it is saved.
    pub fn places(self) -> bool {
        matches!(self, Self::CliPlace | Self::CliExpectRevisionPlace)
    }

    /// The place to pass on for a new entry: `Ok(None)` when none was asked, `unsupported` when
    /// the server does not take one, `invalidInput` for an edit (`edit`), which keeps its entry
    /// where it is.
    pub(crate) fn place_for(self, place: Option<MemorySavePlace>, edit: bool) -> Result<Option<MemorySavePlace>> {
        let Some(place) = place else {
            return Ok(None);
        };

        if edit {
            return Err(BusinessError::InvalidInput(
                "an edit keeps the note or script where it is, so it takes no place".to_owned(),
            ));
        }

        if !self.places() {
            return Err(BusinessError::Unsupported(format!(
                "this {MEMORY_BINARY} cannot save to a chosen place; update Harness Memory"
            )));
        }

        Ok(Some(place))
    }
}

/// `--place remote|local`.
pub(crate) fn place_args(place: Option<MemorySavePlace>) -> Vec<OsString> {
    match place {
        Some(MemorySavePlace::Remote) => vec!["--place".into(), "remote".into()],
        Some(MemorySavePlace::Local) => vec!["--place".into(), "local".into()],
        None => Vec::new(),
    }
}

/// The `place` of a save answer, when it names one.
pub(crate) fn answered_place(answer: &Value) -> Option<MemorySavePlace> {
    match answer.get("place").and_then(Value::as_str) {
        Some("remote") => Some(MemorySavePlace::Remote),
        Some("local") => Some(MemorySavePlace::Local),
        _ => None,
    }
}

/// `notes save --json --client harness-desktop [--project P] [--place remote|local]`.
pub fn note_save_args(project: Option<&Path>, place: Option<MemorySavePlace>) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["notes", "save", "--json", "--client", CLIENT_NAME]
        .map(OsString::from)
        .into();
    args.extend(project_args(project));
    args.extend(place_args(place));

    args
}

/// `notes update <id> --json --client harness-desktop [--project P] [--expect-revision N]`.
pub fn note_update_args(id: i64, project: Option<&Path>, expected_revision: Option<i64>) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "notes".into(),
        "update".into(),
        id.to_string().into(),
        "--json".into(),
        "--client".into(),
        CLIENT_NAME.into(),
    ];
    args.extend(project_args(project));

    if let Some(revision) = expected_revision {
        args.extend(["--expect-revision".into(), revision.to_string().into()]);
    }

    args
}

/// `scripts save --json --client harness-desktop [--project P] [--place remote|local]`.
pub fn script_save_args(project: Option<&Path>, place: Option<MemorySavePlace>) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["scripts", "save", "--json", "--client", CLIENT_NAME]
        .map(OsString::from)
        .into();
    args.extend(project_args(project));
    args.extend(place_args(place));

    args
}

/// Saves note `fields` (`{title, body, kind, tags, entities, scope}`) with `notes save`, or with
/// `notes update <id>` (`{title, body, kind, tags}`) over note `replace_id`. A title another note
/// has is `exists` with that note's id; a note it cannot see is `notFound`. With `expected_revision`
/// (an edit, `notes update --expect-revision`) a note that changed since, or is gone, is a
/// `conflict` and nothing is saved (a `changed` for the project of the save is not one). A new note
/// with a `place` is saved there (`notes save --place`), and the answer says where it went.
#[expect(
    clippy::too_many_arguments,
    reason = "one argument per part of the request, as the save takes them"
)]
pub(crate) async fn save_note_cli(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    project: Option<&Path>,
    mut fields: Map<String, Value>,
    replace_id: Option<i64>,
    expected_revision: Option<i64>,
    place: Option<MemorySavePlace>,
) -> Result<NoteSaved> {
    let (name, args, outcome) = match replace_id {
        Some(id) => {
            for field in ["id", "entities", "scope"] {
                fields.remove(field);
            }

            let args = note_update_args(id, project, expected_revision);

            ("notes update", args, NoteSaveOutcome::Updated)
        }
        None => ("notes save", note_save_args(project, place), NoteSaveOutcome::Created),
    };

    let refusal = match run_save(env, runner, server, name, args, Some(&Value::Object(fields))).await {
        Ok(data) => {
            return Ok(NoteSaved {
                outcome,
                id: data.get("id").and_then(Value::as_i64),
                queued: data.get("queued").and_then(Value::as_bool) == Some(true),
                place: answered_place(&data),
            });
        }
        Err(refusal) => refusal,
    };

    if let Some(id) = taken_title(&refusal.message) {
        return Ok(NoteSaved {
            outcome: NoteSaveOutcome::Exists,
            id: Some(id),
            queued: false,
            place: None,
        });
    }

    // A refusal for the folder of the save (`changed` with the reason `project`) says nothing of the
    // note's revision: it stays the server's own refusal.
    let stale = match refusal.code.as_deref() {
        Some("changed") => !refusal.moved_project,
        Some("notFound") => true,
        _ => false,
    };

    match (refusal.code.as_deref(), replace_id) {
        (Some("changed" | "notFound"), Some(id)) if stale && expected_revision.is_some() => {
            tracing::info!(id, "the edited memory note changed since it was opened");
            Err(BusinessError::Conflict(NOTE_CHANGED_MESSAGE.to_owned()))
        }
        (Some("notFound"), Some(id)) => Err(BusinessError::NotFound(format!("note {id}"))),
        _ => Err(refusal.error),
    }
}

/// Saves script `fields` (as `script_save` takes them, `timeout_secs` included) with `scripts
/// save`, to `place` when given; its answer.
pub(crate) async fn save_script_cli(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    project: Option<&Path>,
    mut fields: Map<String, Value>,
    place: Option<MemorySavePlace>,
) -> Result<Value> {
    if let Some(timeout) = fields.remove("timeout_secs") {
        fields.insert("timeoutSecs".to_owned(), timeout);
    }

    let input = Value::Object(fields);

    run_save(
        env,
        runner,
        server,
        "scripts save",
        script_save_args(project, place),
        Some(&input),
    )
    .await
    .map_err(|refusal| refusal.error)
}

/// Why a save command failed: its code (`notFound`, `gone`, `invalid`, `refused`) when it
/// answered one, what it said (redacted, clipped), and the error it stands for.
#[derive(Debug)]
pub(crate) struct Refusal {
    /// The `error` field of the answer.
    pub code: Option<String>,
    /// Whether the answer says (`"reason": "project"`) that the refusal is about the folder of the
    /// save, not about the note or its revision.
    pub moved_project: bool,
    /// What the server said.
    pub message: String,
    /// The error the answer stands for.
    pub error: BusinessError,
}

impl From<BusinessError> for Refusal {
    fn from(error: BusinessError) -> Self {
        Self {
            code: None,
            moved_project: false,
            message: String::new(),
            error,
        }
    }
}

/// Runs save command `name` with `args` and `input` as JSON on stdin; its answer as JSON. See
/// [`refusal`] for a failure.
pub(crate) async fn run_save(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    name: &str,
    args: Vec<OsString>,
    input: Option<&Value>,
) -> std::result::Result<Value, Refusal> {
    let stdin = match input {
        Some(input) => Some(
            serde_json::to_vec(input)
                .map_err(|error| BusinessError::Internal(format!("encoding a draft failed: {error}")))?,
        ),
        None => None,
    };

    let call = CliCall {
        name,
        args,
        timeout: SAVE_TIMEOUT,
        output_bytes: LIST_OUTPUT_BYTES,
        stdin,
    };

    run_save_call(env, runner, server, call).await
}

/// [`run_save`] of `call` as given (its own time limit, output kept and stdin).
pub(crate) async fn run_save_call(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    call: CliCall<'_>,
) -> std::result::Result<Value, Refusal> {
    let name = call.name;

    match run_memory_cli(env, runner, server, call).await? {
        CliAnswer::Done(stdout) => serde_json::from_str(&stdout).map_err(|_| unreadable(name).into()),
        CliAnswer::Failed {
            status: Some(USAGE_EXIT),
            ..
        } => Err(predates(name).into()),
        CliAnswer::Failed { message, stdout, .. } => {
            let refusal = refusal(&stdout, message);
            tracing::warn!(command = name, code = ?refusal.code, "a harness-memory save failed");

            Err(refusal)
        }
    }
}

fn unreadable(name: &str) -> BusinessError {
    BusinessError::Protocol(format!("harness-memory {name} answered output this app cannot read"))
}

fn predates(name: &str) -> BusinessError {
    BusinessError::Unsupported(format!("this {MEMORY_BINARY} predates `{name}`; update Harness Memory"))
}

/// The refusal a failure answer stands for; `message` (the server's line) when it printed no
/// JSON: `notFound` is not found, `gone` (the note a held change was for was deleted) a conflict
/// with [`GONE_MESSAGE`], `changed` (the agent held that save again since the user saw it)
/// changed with [`CHANGED_MESSAGE`], `invalid` invalid input, `refused` and anything else a
/// process error with the server's text.
pub(crate) fn refusal(stdout: &str, message: String) -> Refusal {
    let data = serde_json::from_str::<Value>(stdout.trim()).unwrap_or(Value::Null);
    let code = data.get("error").and_then(Value::as_str).map(str::to_owned);

    let message = data
        .get("message")
        .and_then(Value::as_str)
        .map(clean)
        .filter(|text| !text.is_empty())
        .unwrap_or(message);

    let moved_project = data.get("reason").and_then(Value::as_str) == Some("project");

    let error = match code.as_deref() {
        Some("gone") => BusinessError::Conflict(GONE_MESSAGE.to_owned()),
        Some("changed") if moved_project => BusinessError::Process(message.clone()),
        Some("changed") => BusinessError::Changed(CHANGED_MESSAGE.to_owned()),
        Some("notFound") => BusinessError::NotFound(message.clone()),
        Some("invalid") => BusinessError::InvalidInput(message.clone()),
        _ => BusinessError::Process(message.clone()),
    };

    Refusal {
        code,
        moved_project,
        message,
        error,
    }
}

#[cfg(test)]
mod tests;
