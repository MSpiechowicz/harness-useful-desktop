//! A `harness-knowledge` command that may refuse, for [`knowledge_held`](super::knowledge_held) and
//! [`knowledge_review`](super::knowledge_review).
//!
//! A refusal exits 1 and prints `{"error": CODE, "message"}` on stdout. It is an answer
//! ([`Answer::Refused`] with the server's sentence, redacted), not a failure: the thing refused is
//! as it was. The codes are [`KnowledgeRefusalCode`]s. A remote database that cannot be reached
//! (`{"error": "unreachable"}` or the server's line saying so) is a `BusinessError::Unreachable`, an
//! exit 2 an `unsupported` error (the server predates the command), any other failure a `process`
//! error with the server's line.

use std::path::Path;

use serde_json::Value;

use super::catalog::KNOWLEDGE_SIDECAR;
use super::knowledge_cli::{CliCall, stdout_object, unreachable_in};
use super::sidecar_cli::{CliAnswer, USAGE_EXIT, clean, run_sidecar_cli};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::KnowledgeRefusalCode;
use crate::process::ProcessRunner;

/// The exit code of a command that refused.
const REFUSED_EXIT: i32 = 1;

/// What a command answered: its object, or its refusal.
pub(super) enum Answer {
    Done(Value),
    Refused(KnowledgeRefusalCode, String),
}

/// Runs `call` and reads its stdout as JSON, with a refusal as an [`Answer::Refused`] (see the module
/// docs). `protocol` when the answer is not JSON.
pub(super) async fn run_refusing(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    call: CliCall<'_>,
) -> Result<Answer> {
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
            Some((code, sentence)) => Ok(Answer::Refused(code, sentence.unwrap_or(message))),
            None => Err(unreachable_in(&stdout, &message)
                .map_or_else(|| BusinessError::Process(message), BusinessError::Unreachable)),
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
/// output (and for a code this app does not know).
fn refusal_of(stdout: &str) -> Option<(KnowledgeRefusalCode, Option<String>)> {
    let data = stdout_object(stdout)?;

    let code = match data.get("error")?.as_str()? {
        "notFound" => KnowledgeRefusalCode::NotFound,
        "changed" => KnowledgeRefusalCode::Changed,
        "wrongState" => KnowledgeRefusalCode::WrongState,
        "approvalOn" => KnowledgeRefusalCode::ApprovalOn,
        "approvalOff" => KnowledgeRefusalCode::ApprovalOff,
        "selfApproval" => KnowledgeRefusalCode::SelfApproval,
        "notAuthor" => KnowledgeRefusalCode::NotAuthor,
        "noName" => KnowledgeRefusalCode::NoName,
        "readOnly" => KnowledgeRefusalCode::ReadOnly,
        "commentRequired" => KnowledgeRefusalCode::CommentRequired,
        "sinceUnavailable" => KnowledgeRefusalCode::SinceUnavailable,
        "localPath" => KnowledgeRefusalCode::LocalPath,
        "refused" => KnowledgeRefusalCode::Refused,
        _ => return None,
    };

    Some((code, data.get("message").and_then(Value::as_str).map(clean)))
}
