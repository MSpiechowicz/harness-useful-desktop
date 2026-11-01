//! `harness-knowledge check --json`: what a team database can do ([`knowledge_remote_test`]).
//!
//! Running it, and what it is given through the environment, is
//! [`sidecar_remote_check`](crate::ecosystem::sidecar_remote_check)'s. The server prints one JSON
//! object and exits 0 when the database has everything it needs, 3 when something is missing
//! (`problem` names it), 1 when the check itself failed and 2 for a database it has no support
//! for. [`read_check`] turns the object into a [`KnowledgeCheck`] with a typed problem. Every
//! sentence of the server is masked of the typed and the saved secrets first, so none reaches the
//! webview.

use super::file::read_config;
use super::knowledge_config_path;
use crate::ecosystem::catalog::KNOWLEDGE_SIDECAR;
use crate::ecosystem::knowledge_binary::installed_knowledge;
use crate::ecosystem::knowledge_cli::problem_of;
use crate::ecosystem::sidecar_cli::clean;
use crate::ecosystem::sidecar_remote_check::{CheckRequest, run_check_raw};
use crate::ecosystem::sidecar_remote_url::{
    check_secrets, reason, saved_url_problem, scheme_fits, scrub, validate_token, validate_url,
};
use crate::env::Env;
use crate::model::{KnowledgeCapabilities, KnowledgeCheck, KnowledgeIndexState, MemoryRemoteKind};
use crate::process::ProcessRunner;

use serde_json::Value;

/// Runs `harness-knowledge check --json` (20 s limit) against a database of `kind`: `url` and
/// `token` when given (an empty `token` tests without one), else the saved ones. `ok` false with
/// the reason when the server is not installed, the input is not valid, or the check could not
/// run.
pub async fn knowledge_remote_test(
    env: &Env,
    runner: &dyn ProcessRunner,
    kind: MemoryRemoteKind,
    url: Option<String>,
    token: Option<String>,
) -> KnowledgeCheck {
    if let Some(reason) = super::unsupported_kind(kind) {
        return refused(reason);
    }

    let Some(server) = installed_knowledge(env, runner).await else {
        return refused("Harness Knowledge is not installed".to_owned());
    };

    let url = match url.as_deref().map(|raw| validate_url(raw, kind)).transpose() {
        Ok(url) => url,
        Err(error) => return refused(reason(error)),
    };

    let saved = knowledge_config_path(env)
        .map(|path| read_config(&path))
        .unwrap_or_default();

    if url.is_none()
        && let Some(problem) = saved_url_problem(saved.url.as_deref(), |url| scheme_fits(url, kind), kind)
    {
        return refused(problem);
    }

    // `Some(None)`: test without a token; `None`: use the saved one (and never for other kinds).
    let token = match (kind, token.as_deref()) {
        (MemoryRemoteKind::Libsql, Some(raw)) => match validate_token(raw) {
            Ok(token) => Some(token),
            Err(error) => return refused(reason(error)),
        },
        _ => None,
    };

    // The server may quote the saved values as well as the typed ones.
    let secrets = check_secrets(
        url.as_deref(),
        token.as_ref(),
        saved.url.as_deref(),
        saved.token.as_deref(),
    );

    let request = CheckRequest {
        url: url.as_deref(),
        database_kind: None,
        token: token.as_ref().map(|token| token.as_deref()),
        secrets: &secrets,
        json: true,
    };

    match run_check_raw(&KNOWLEDGE_SIDECAR, env, runner, &server.path, request).await {
        Ok(output) => read_check(&output.stdout, &output.stderr, output.status, &secrets),
        Err(message) => refused(message),
    }
}

/// The check of the server's answer: `stdout` is its JSON, `stderr` where its line is when the
/// check failed without one, `status` the exit code. `secrets` are masked out of every sentence.
pub fn read_check(stdout: &str, stderr: &str, status: Option<i32>, secrets: &[String]) -> KnowledgeCheck {
    let data: Option<Value> = stdout
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with('{'))
        .and_then(|line| serde_json::from_str(line).ok());

    let Some(data) = data else {
        let line = stderr.lines().map(str::trim).rfind(|line| !line.is_empty());
        let message = match line {
            Some(line) => clean(&scrub(line, secrets)),
            None => format!(
                "{} check failed (exit code {})",
                KNOWLEDGE_SIDECAR.binary,
                code_text(status)
            ),
        };

        return refused(message);
    };

    let text = |name: &str| {
        data.get(name)
            .and_then(Value::as_str)
            .map(|text| clean(&scrub(text, secrets)))
    };
    // A check that found a problem names it under `problem`, one that could not open the database
    // (`{"error": "missing", "code"}`, exit 3) under `code`.
    let code = data
        .get("problem")
        .or_else(|| data.get("code"))
        .and_then(Value::as_str)
        .map(problem_of);
    let failed = data.get("error").is_some();

    KnowledgeCheck {
        ok: status == Some(0) && data.get("ok").and_then(Value::as_bool).unwrap_or(false),
        backend: data.get("backend").and_then(Value::as_str).map(str::to_owned),
        problem: code,
        message: text("message"),
        capabilities: capabilities(data.get("capabilities")).filter(|_| !failed),
        model: data.pointer("/model/id").and_then(Value::as_str).map(clean),
    }
}

fn capabilities(value: Option<&Value>) -> Option<KnowledgeCapabilities> {
    let value = value?;
    let flag = |name: &str| value.get(name).and_then(Value::as_bool);

    Some(KnowledgeCapabilities {
        keyword: flag("keyword")?,
        vector: flag("vector")?,
        vector_index: match value.get("vectorIndex").and_then(Value::as_str) {
            Some("ready") => KnowledgeIndexState::Ready,
            Some("building") => KnowledgeIndexState::Building,
            _ => KnowledgeIndexState::Missing,
        },
    })
}

fn code_text(status: Option<i32>) -> String {
    status.map_or_else(|| "none, it was stopped".to_owned(), |code| code.to_string())
}

/// A check that did not run (or whose answer could not be read), with `message` why.
fn refused(message: String) -> KnowledgeCheck {
    KnowledgeCheck {
        ok: false,
        backend: None,
        problem: None,
        message: Some(message),
        capabilities: None,
        model: None,
    }
}
