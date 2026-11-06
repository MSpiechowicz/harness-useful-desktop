//! What a start launches: only harnesses with chat, resuming only a session of the project, in a
//! project the harness trusts or trusts once, run through the harness's command line with its
//! spawn environment and the launcher search path of its binary. A chat with a memory identity
//! has its guards written first ([`prepare_memory_chat`]) and, once they are, its key passed as
//! `HARNESS_MEMORY_SESSION`, which the harness's MCP servers inherit (Codex, which clears their
//! environment, is told per thread to pass it on, see `codex/memory.rs`); any other launch has
//! that variable removed, so a value the app itself inherited never names a chat.

use std::path::Path;

use super::manager::AgentDeps;
use crate::ecosystem::knowledge_binary::KNOWLEDGE_SESSION_VAR;
use crate::ecosystem::memory_guards::{
    SESSION_VAR, chat_repo_root, check_chat_key, ensure_global, guards_data_dir, rules_missing, version_one_remains,
    write_chat,
};
use crate::ecosystem::{mcp_target, memory_data_dir, memory_mcp_listed};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::harness::{Harness, LaunchRequest};
use crate::model::{AgentStartRequest, ApprovalMode, MemoryChatStart};
use crate::pi_family::sessions::is_safe_session_id;
use crate::process::{ExecSpec, launcher_search_dirs};

/// Refuses a harness whose chat the app cannot drive yet, before anything is resolved or spawned.
pub(super) fn ensure_chat(harness: &dyn Harness) -> Result<()> {
    if harness.capabilities().chat {
        return Ok(());
    }

    Err(BusinessError::Unsupported(format!(
        "chat with {} is coming soon",
        harness.info().name
    )))
}

/// The approval override `requested` for a chat of `harness`: dropped where the harness has no
/// approval modes, refused when it is Yolo or Auto and the harness does not offer it (Yolo on
/// Claude Code, Auto everywhere else).
pub(super) fn approval_override(
    harness: &dyn Harness,
    requested: Option<ApprovalMode>,
) -> Result<Option<ApprovalMode>> {
    let capabilities = harness.capabilities();

    if !capabilities.approval_modes {
        return Ok(None);
    }

    let refused = match requested {
        Some(ApprovalMode::Yolo) if !capabilities.yolo_mode => Some("Yolo"),
        Some(ApprovalMode::Auto) if !capabilities.auto_mode => Some("Auto"),
        _ => None,
    };

    if let Some(mode) = refused {
        return Err(BusinessError::Unsupported(format!(
            "{} chats have no {mode} mode: choose {}",
            harness.info().name,
            offered_modes(capabilities.yolo_mode, capabilities.auto_mode)
        )));
    }

    Ok(requested)
}

/// The approval modes a harness offers, as "Always ask, Write or Yolo".
fn offered_modes(yolo: bool, auto: bool) -> String {
    let mut modes = vec!["Always ask", "Write"];

    if yolo {
        modes.push("Yolo");
    }

    if auto {
        modes.push("Auto");
    }

    let last = modes.pop().unwrap_or_default();

    format!("{} or {last}", modes.join(", "))
}

/// The session id `request` resumes in `project`. Refused (`invalidInput`) when it names another
/// harness, is not a safe id or comes with a utility agent, and refused before anything is spawned
/// when the harness finds it is not one of the project's sessions ([`Harness::check_resume`]).
pub(super) fn resumed_session<'a>(
    harness: &dyn Harness,
    env: &Env,
    project: &Path,
    request: &'a AgentStartRequest,
) -> Result<Option<&'a str>> {
    let Some(session) = &request.resume else {
        return Ok(None);
    };

    if session.harness != harness.id() || !is_safe_session_id(&session.id) {
        return Err(BusinessError::InvalidInput("invalid session to resume".to_owned()));
    }

    if request.utility {
        return Err(BusinessError::InvalidInput(
            "a utility agent cannot resume a session".to_owned(),
        ));
    }

    harness.check_resume(env, project, session)?;

    Ok(Some(&session.id))
}

/// Whether a chat of `harness` in `project` trusts the project once: only when `requested` and the
/// harness has project trust. A launch that does not is refused in a folder the harness does not
/// trust yet ([`Harness::check_project_trust`]), before anything is spawned.
pub(super) fn trust_once(harness: &dyn Harness, env: &Env, project: &Path, requested: bool) -> Result<bool> {
    let trust_once = requested && harness.capabilities().project_trust;

    if !trust_once {
        harness.check_project_trust(env, project)?;
    }

    Ok(trust_once)
}

/// The command line of `request` for the harness of `deps`, naming chat `memory_session` to
/// Harness Memory and Harness Knowledge when there is one.
pub(super) fn exec_spec(
    deps: &AgentDeps<'_>,
    request: &LaunchRequest<'_>,
    memory_session: Option<&str>,
) -> Result<ExecSpec> {
    let mut spec = deps
        .harness
        .spawn_spec(deps.env, deps.harness.launch_spec(request)?)
        .search_dirs(launcher_search_dirs(deps.env, &deps.binary.path))
        .env_remove_prefix(SESSION_VAR)
        .env_remove_prefix(KNOWLEDGE_SESSION_VAR);

    if let Some(key) = memory_session {
        spec = spec.env(SESSION_VAR, key).env(KNOWLEDGE_SESSION_VAR, key);
    }

    Ok(spec)
}

/// How the failure of [`keep_guards_current`] begins: "Harness Memory settings could not be
/// updated: <reason>".
pub const SETTINGS_NOT_UPDATED: &str = "Harness Memory settings could not be updated";

/// Brings the guards of every chat in Harness Memory's data folder up to date ([`ensure_global`])
/// before an agent starts, whether it is a chat, a routine's run or a utility one: the server reads
/// them for every one of them. It runs first in a start, so the file the rules are read against is
/// current for every agent that starts.
///
/// Nothing happens (no folder or file is created) while this computer does not use Harness Memory:
/// no data folder and no server ([`guards_data_dir`]), unless the start has a memory chat, whose
/// guards are written to that folder anyway and must come with the marker. Otherwise the work runs
/// off the runtime's workers.
///
/// A failure fails the start when the server would then apply other rules than the user's: the
/// start has a memory chat (whose rules would not be current), a readable version 1 file remains
/// (the server would keep its deny list, with saving on), or neither `global.json` nor the marker
/// exists ([`rules_missing`]: a folder that could not be taken or written leaves neither, and the
/// server then applies no rules at all). It never fails a utility agent. While Project memory is
/// off (`memory_enabled`) the app removed its entry, so usually no harness runs the server and the
/// alert in Settings would be greyed out: a failure is then logged and the agent starts, except
/// when the rules are missing or a readable version 1 file remains, and this harness's MCP files,
/// the ones it also reads included, still list a `harness-memory` server ([`memory_mcp_listed`]):
/// an OpenCode config with comments, an entry the app does not own, a file change that failed or a
/// harness that was not detected at switch-off leave it configured, and it would save anywhere
/// (with no rules) or with the old deny list. A file that cannot be read counts as listing it. Any other failure is logged and the agent starts. The error is `io` with a
/// sentence for the user: "Harness Memory settings could not be updated: <reason>".
pub(super) async fn keep_guards_current(
    env: &Env,
    harness: &dyn Harness,
    request: &AgentStartRequest,
    memory_enabled: bool,
) -> Result<()> {
    // Only asked while Project memory is off: the files that may still start the server.
    let mcp_file = if memory_enabled { None } else { mcp_target(env, harness) };
    let env = env.clone();
    let memory_chat = request.memory_chat.is_some();

    let outcome = tokio::task::spawn_blocking(move || {
        let data_dir = if memory_chat {
            memory_data_dir(&env)?
        } else {
            guards_data_dir(&env)?
        };
        let result = ensure_global(&data_dir);
        let missing = result.is_err() && rules_missing(&data_dir);
        let stale = result.is_err() && (missing || version_one_remains(&data_dir));
        let server_left = stale && mcp_file.as_ref().is_some_and(memory_mcp_listed);

        Some((result, stale, server_left))
    })
    .await
    .map_err(|error| BusinessError::Internal(format!("updating the memory settings failed: {error}")))?;

    let Some((Err(error), stale, server_left)) = outcome else {
        return Ok(());
    };

    let fails = if memory_enabled {
        memory_chat || stale
    } else {
        server_left
    };

    if request.utility || !fails {
        tracing::warn!(code = ?error.code(), "the memory save guards could not be brought up to date");
        return Ok(());
    }

    Err(settings_not_updated(&error))
}

/// `io` (the UI shows its text as it is, and `forbidden` would be replaced by a generic one) with
/// the sentence "Harness Memory settings could not be updated: <reason>".
fn settings_not_updated(error: &BusinessError) -> BusinessError {
    let reason = match error {
        BusinessError::InvalidInput(message) | BusinessError::Forbidden(message) | BusinessError::Protocol(message) => {
            message.clone()
        }
        other => other.to_string(),
    };

    BusinessError::Io {
        context: SETTINGS_NOT_UPDATED.to_owned(),
        source: std::io::Error::other(reason),
    }
}

/// Writes the guards of `chat` in `project` (with the project root there) to Harness Memory's data
/// folder before its harness starts, and returns its key for `HARNESS_MEMORY_SESSION`. The guards
/// of every chat are current by then ([`keep_guards_current`]).
/// `invalidInput` for a key that is not a lowercase UUID; a guards file that cannot be written
/// fails the start, so a chat never runs without the rules the user set. While the data folder is
/// unknown (Windows without `LOCALAPPDATA`) nothing is written and `None` comes back: the harness
/// gets no session variable, as the server would find no chat file for it.
pub(super) fn prepare_memory_chat<'a>(env: &Env, chat: &'a MemoryChatStart, project: &Path) -> Result<Option<&'a str>> {
    check_chat_key(&chat.key)?;

    let Some(data_dir) = memory_data_dir(env) else {
        tracing::warn!("the Harness Memory data folder is unknown; the chat starts without memory guards");
        return Ok(None);
    };

    write_chat(&data_dir, &chat.key, &chat.guards, chat_repo_root(project).as_deref())?;

    Ok(Some(&chat.key))
}
