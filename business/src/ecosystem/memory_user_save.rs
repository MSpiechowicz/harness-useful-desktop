//! The user's own saves through the commands of a server with `save-guards` (see
//! [`memory_features`](super::memory_features)), which agents cannot reach through MCP, so no save
//! guard applies to them. Each reads its draft as JSON on stdin and answers JSON:
//!
//! - `notes save --json --client harness-desktop [--project P]` takes `{title, body, kind, tags, entities, scope}` and
//!   answers `{"id", "title", "scope"}`;
//! - `notes update <id> --json --client harness-desktop [--project P]` takes `{title, body, kind, tags}` and answers
//!   `{"id", "title"}`; a note it cannot see (a repository note needs `--project`) is `notFound`;
//! - `scripts save --json --client harness-desktop [--project P]` takes `{name, description, language, body, timeoutSecs,
//!   scope}` and answers `{"name", "scope", "id", "created", "approval"}`, trusted as
//!   `script_save` trusts it (never code waiting for approval: the caller approves the user's code);
//! - `held save|skip` ([`memory_held`](super::memory_held)) use the same answers.
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
use crate::model::{NoteSaveOutcome, NoteSaved};
use crate::process::ProcessRunner;

/// How long one save may take (30 s: a team database far away).
pub const SAVE_TIMEOUT: Duration = Duration::from_secs(30);

/// What the app says for a held note change whose note was deleted since (`gone`).
pub const GONE_MESSAGE: &str = "the note was deleted after the agent changed it, so nothing was saved";

/// What the app says for a held save the agent changed after the user saw it (`changed`).
pub const CHANGED_MESSAGE: &str = "the agent changed this save after you saw it, so nothing was saved";

/// How the user's notes and scripts are saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveRoute {
    /// Through the pooled `harness-memory mcp` connection (a server without `save-guards`).
    Mcp,
    /// Through the user-only commands (a server with `save-guards`).
    Cli,
}

impl SaveRoute {
    /// [`SaveRoute::Cli`] when the installed server lists `save-guards`.
    pub fn of(features: FeaturesSupport) -> Self {
        if features.save_guards() { Self::Cli } else { Self::Mcp }
    }
}

/// `notes save --json --client harness-desktop [--project P]`.
pub fn note_save_args(project: Option<&Path>) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["notes", "save", "--json", "--client", CLIENT_NAME]
        .map(OsString::from)
        .into();
    args.extend(project_args(project));

    args
}

/// `notes update <id> --json --client harness-desktop [--project P]`.
pub fn note_update_args(id: i64, project: Option<&Path>) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "notes".into(),
        "update".into(),
        id.to_string().into(),
        "--json".into(),
        "--client".into(),
        CLIENT_NAME.into(),
    ];
    args.extend(project_args(project));

    args
}

/// `scripts save --json --client harness-desktop [--project P]`.
pub fn script_save_args(project: Option<&Path>) -> Vec<OsString> {
    let mut args: Vec<OsString> = ["scripts", "save", "--json", "--client", CLIENT_NAME]
        .map(OsString::from)
        .into();
    args.extend(project_args(project));

    args
}

/// Saves note `fields` (`{title, body, kind, tags, entities, scope}`) with `notes save`, or with
/// `notes update <id>` (`{title, body, kind, tags}`) over note `replace_id`. A title another note
/// has is `exists` with that note's id; a note it cannot see is `notFound`.
pub(crate) async fn save_note_cli(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    project: Option<&Path>,
    mut fields: Map<String, Value>,
    replace_id: Option<i64>,
) -> Result<NoteSaved> {
    let (name, args, outcome) = match replace_id {
        Some(id) => {
            for field in ["id", "entities", "scope"] {
                fields.remove(field);
            }

            ("notes update", note_update_args(id, project), NoteSaveOutcome::Updated)
        }
        None => ("notes save", note_save_args(project), NoteSaveOutcome::Created),
    };

    let refusal = match run_save(env, runner, server, name, args, Some(&Value::Object(fields))).await {
        Ok(data) => {
            return Ok(NoteSaved {
                outcome,
                id: data.get("id").and_then(Value::as_i64),
            });
        }
        Err(refusal) => refusal,
    };

    if let Some(id) = taken_title(&refusal.message) {
        return Ok(NoteSaved {
            outcome: NoteSaveOutcome::Exists,
            id: Some(id),
        });
    }

    match (refusal.code.as_deref(), replace_id) {
        (Some("notFound"), Some(id)) => Err(BusinessError::NotFound(format!("note {id}"))),
        _ => Err(refusal.error),
    }
}

/// Saves script `fields` (as `script_save` takes them, `timeout_secs` included) with `scripts
/// save`; its answer.
pub(crate) async fn save_script_cli(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    project: Option<&Path>,
    mut fields: Map<String, Value>,
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
        script_save_args(project),
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
    /// What the server said.
    pub message: String,
    /// The error the answer stands for.
    pub error: BusinessError,
}

impl From<BusinessError> for Refusal {
    fn from(error: BusinessError) -> Self {
        Self {
            code: None,
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

    let error = match code.as_deref() {
        Some("gone") => BusinessError::Conflict(GONE_MESSAGE.to_owned()),
        Some("changed") => BusinessError::Changed(CHANGED_MESSAGE.to_owned()),
        Some("notFound") => BusinessError::NotFound(message.clone()),
        Some("invalid") => BusinessError::InvalidInput(message.clone()),
        _ => BusinessError::Process(message.clone()),
    };

    Refusal { code, message, error }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    fn strings(args: Vec<OsString>) -> Vec<String> {
        args.into_iter().map(|arg| arg.into_string().unwrap()).collect()
    }

    #[test]
    fn the_save_commands_name_the_app_and_the_project_when_there_is_one() {
        let project = Path::new("/work/repo");

        assert_eq!(
            strings(note_save_args(Some(project))),
            [
                "notes",
                "save",
                "--json",
                "--client",
                "harness-desktop",
                "--project",
                "/work/repo"
            ]
        );
        assert_eq!(
            strings(note_save_args(None)),
            ["notes", "save", "--json", "--client", "harness-desktop"]
        );
        assert_eq!(
            strings(note_update_args(12, Some(project))),
            [
                "notes",
                "update",
                "12",
                "--json",
                "--client",
                "harness-desktop",
                "--project",
                "/work/repo"
            ]
        );
        assert_eq!(
            strings(note_update_args(12, None)),
            ["notes", "update", "12", "--json", "--client", "harness-desktop"]
        );
        assert_eq!(
            strings(script_save_args(Some(project))),
            [
                "scripts",
                "save",
                "--json",
                "--client",
                "harness-desktop",
                "--project",
                "/work/repo"
            ]
        );
    }

    #[test]
    fn a_refusal_is_read_from_its_code() {
        let gone = refusal(r#"{"error": "gone", "message": "note 4 is gone"}"#, "line".to_owned());
        assert!(matches!(gone.error, BusinessError::Conflict(ref text) if text == GONE_MESSAGE));

        let changed = refusal(
            r#"{"error": "changed", "message": "held save changed"}"#,
            "line".to_owned(),
        );
        assert!(matches!(changed.error, BusinessError::Changed(ref text) if text == CHANGED_MESSAGE));

        let missing = refusal(
            r#"{"error": "notFound", "message": ""}"#,
            "harness-memory: no held save".to_owned(),
        );
        assert!(matches!(missing.error, BusinessError::NotFound(ref text) if text == "harness-memory: no held save"));

        let invalid = refusal(
            r#"{"error": "invalid", "message": "unknown field `x`"}"#,
            "line".to_owned(),
        );
        assert!(matches!(invalid.error, BusinessError::InvalidInput(ref text) if text == "unknown field `x`"));

        let taken = refusal(
            r#"{"error": "refused", "message": "note 3 already has this title"}"#,
            "line".to_owned(),
        );
        assert_eq!(taken.code.as_deref(), Some("refused"));
        assert_eq!(taken_title(&taken.message), Some(3));

        let plain = refusal("not json", "harness-memory: database locked".to_owned());
        assert_eq!(plain.code, None);
        assert!(matches!(plain.error, BusinessError::Process(ref text) if text == "harness-memory: database locked"));
    }

    #[test]
    fn only_a_server_with_save_guards_takes_the_user_commands() {
        assert_eq!(SaveRoute::of(FeaturesSupport::Unknown), SaveRoute::Mcp);
        assert_eq!(SaveRoute::of(FeaturesSupport::Unsupported), SaveRoute::Mcp);
    }
}
