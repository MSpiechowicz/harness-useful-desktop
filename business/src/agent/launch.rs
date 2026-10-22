//! What a start launches: only harnesses with chat, resuming only a session of the project, in a
//! project the harness trusts or trusts once, run through the harness's command line with its
//! spawn environment and the launcher search path of its binary. A chat with a memory identity
//! has its guards written first ([`prepare_memory_chat`]) and, once they are, its key passed as
//! `HARNESS_MEMORY_SESSION`, which the harness's MCP servers inherit; any other launch has that
//! variable removed, so a value the app itself inherited never names a chat.

use std::path::Path;

use super::manager::AgentDeps;
use crate::ecosystem::memory_data_dir;
use crate::ecosystem::memory_guards::{SESSION_VAR, chat_repo_root, check_chat_key, write_chat};
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
/// approval modes, refused when it is Yolo and the harness does not offer it (Claude Code).
pub(super) fn approval_override(
    harness: &dyn Harness,
    requested: Option<ApprovalMode>,
) -> Result<Option<ApprovalMode>> {
    let capabilities = harness.capabilities();

    if !capabilities.approval_modes {
        return Ok(None);
    }

    if requested == Some(ApprovalMode::Yolo) && !capabilities.yolo_mode {
        return Err(BusinessError::Unsupported(format!(
            "{} chats have no Yolo mode: choose Always ask or Write",
            harness.info().name
        )));
    }

    Ok(requested)
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
/// Harness Memory when there is one.
pub(super) fn exec_spec(
    deps: &AgentDeps<'_>,
    request: &LaunchRequest<'_>,
    memory_session: Option<&str>,
) -> Result<ExecSpec> {
    let mut spec = deps
        .harness
        .launch_spec(request)?
        .envs(deps.harness.spawn_env().iter().copied())
        .search_dirs(launcher_search_dirs(deps.env, &deps.binary.path))
        .env_remove_prefix(SESSION_VAR);

    if let Some(key) = memory_session {
        spec = spec.env(SESSION_VAR, key);
    }

    Ok(spec)
}

/// Writes the guards of `chat` in `project` (with the project root there) to Harness Memory's data
/// folder before its harness starts, and returns its key for `HARNESS_MEMORY_SESSION`.
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
