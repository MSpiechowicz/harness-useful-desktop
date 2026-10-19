//! Creating and editing a script from the Scripts view (`memory_script_save`), through the
//! pooled `harness-memory mcp` connection (see
//! [`MemoryScripts::call_tool`](super::memory_scripts::MemoryScripts::call_tool)), passing the
//! project root as `project_path` when there is one.
//!
//! `script_save` replaces the script of the same name in the scope without asking, so
//! `script_get {name, scope}` comes first: unless the caller asks to replace it, a script found
//! there is [`ScriptSaveOutcome::Exists`] and nothing is saved; the server's `no script named …`
//! lets the save go on, and so does a read-only team script (scripts routed `remote+local`: the
//! save adds a script of that name on this machine, which agents find first). An edit names the version the editor opened (`base`, its
//! [`script_sha256`]): when the script found there is gone or has another hash, the save is
//! refused (`conflict`) and nothing is saved. `script_save {scope, name, description, language,
//! body, timeout_secs}` answers `{script, created}` as structured content: the script saved here
//! is trusted on this machine (unless the server says why not, as its approval).
//!
//! The server does not trust the code of a script waiting for approval, or of a copy of one, when
//! it is saved again. When the answer is `needed`, the app approves the saved version as the
//! review does ([`trust_scripts`] with the [`script_sha256`] of the draft) and reads the script
//! again, but only when the user put that code in:
//!
//! - an edit changed the code of the version it opened, compared as the editor shows it (line
//!   endings as LF, no trailing spaces or tabs on a line, no trailing blank lines): an edit of the
//!   description, the timeout, the language, the line endings or trailing whitespace alone leaves
//!   an agent's code waiting for review (such a body is saved with the bytes it had, since the
//!   server only keeps a byte-for-byte copy of waiting code untrusted);
//! - an add that replaces the script of that name (the user chose "Replace it") holds other code
//!   than that script, or that script was trusted;
//! - an add of a new name: the user typed or pasted it, even a copy of code waiting for approval
//!   elsewhere (the user's choice);
//! - and the stored script is the draft: its hash, name and scope, no repository for a global
//!   script, and for a repository script the repository of the script it replaces (when there was
//!   one).
//!
//! Otherwise (or when the server answers `conflict`) the save answers `needed` as the server did.
//!
//! The draft is checked against the server's limits first, and a repository script needs a
//! project (`invalidInput`). Neither the description nor the body is logged.

use std::path::Path;

use serde_json::{Map, Value, json};

use super::mcp_client::ToolOutput;
use super::memory_scripts::{MemoryScripts, SCRIPT_GET_TOOL, parse, tool_error_text, with_project};
use super::memory_trust::{script_sha256, trust_scripts};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{
    ScriptApproval, ScriptDetail, ScriptDraft, ScriptSaveOutcome, ScriptSaved, ScriptScope, ScriptSummary,
    ScriptTrustItem,
};
use crate::process::ProcessRunner;

/// Harness Memory's `SCRIPT_NAME_MAX`, in characters.
pub const SCRIPT_NAME_MAX: usize = 64;
/// Its `SCRIPT_DESCRIPTION_MAX`, in characters.
pub const SCRIPT_DESCRIPTION_MAX: usize = 500;
/// Its `SCRIPT_BODY_MAX`, in bytes.
pub const SCRIPT_BODY_MAX: usize = 64 * 1024;
/// Its `SCRIPT_TIMEOUT_MAX`, in seconds.
pub const SCRIPT_TIMEOUT_MAX: u32 = 600;

const SCRIPT_SAVE_TOOL: &str = "script_save";

/// What the server answers `script_get` for a name it does not have.
const MISSING_PREFIX: &str = "no script named";

/// Why an edit is refused when the script is not the version the editor opened.
const CHANGED_MESSAGE: &str = "the script changed since you opened it";

impl MemoryScripts {
    /// Saves `draft` (see the module docs): `exists` when a script of that name is in that scope
    /// and `replace` is false. An edit passes `base`, the hash of the version it opened, and
    /// `replace`: `conflict` (nothing saved) when that script is gone or changed. `notFound` when
    /// the server is not installed, `unsupported` when it predates saving from the app,
    /// `invalidInput` for a draft over a limit or a repository script without a project,
    /// `process` with the server's (redacted) text when it refuses.
    /// A saved script the server answers `needed` for is approved with `runner` (see the module
    /// docs); failing that, it is still saved and answered as the server did.
    pub async fn save(
        &self,
        env: &Env,
        runner: &dyn ProcessRunner,
        project: Option<&Path>,
        draft: &ScriptDraft,
        replace: bool,
        base: Option<&str>,
    ) -> Result<ScriptSaved> {
        let mut fields = checked(draft, project)?;
        let existing = self.existing(env, project, &fields).await?;

        if let Some(base) = base {
            unchanged(existing.as_ref(), base)?;
        }

        // The server keeps code waiting for approval only when it is saved byte for byte: code the
        // editor only re-spaced is saved as it was, so it keeps waiting there too.
        let kept;
        let draft = match existing.as_ref().filter(|script| respaced(draft, script)) {
            Some(script) => {
                kept = ScriptDraft {
                    body: script.body.clone(),
                    ..draft.clone()
                };
                fields = checked(&kept, project)?;
                &kept
            }
            None => draft,
        };

        if !replace && let Some(script) = existing {
            return Ok(ScriptSaved {
                outcome: ScriptSaveOutcome::Exists,
                id: script.summary.id,
                script: None,
            });
        }

        let output = self
            .call_tool(env, SCRIPT_SAVE_TOOL, with_project(fields, project))
            .await?;
        let saved = saved_from(&output)?;
        tracing::info!(id = saved.id, outcome = ?saved.outcome, "memory script saved");

        let Some(script) = saved.script.as_ref() else {
            return Ok(saved);
        };

        if script.approval != ScriptApproval::Needed {
            return Ok(saved);
        }

        if !users_code(draft, script, existing.as_ref(), base.is_some()) {
            tracing::info!(
                id = script.id,
                "the saved memory script is not code the user wrote, left for review"
            );
            return Ok(saved);
        }

        let script = self.approve_draft(env, runner, project, draft, script).await;

        Ok(ScriptSaved {
            script: Some(script),
            ..saved
        })
    }

    /// `script`, saved from `draft` (the user's code, see [`users_code`]) and waiting for
    /// approval, approved on this machine and read again; as it is when the approval fails.
    async fn approve_draft(
        &self,
        env: &Env,
        runner: &dyn ProcessRunner,
        project: Option<&Path>,
        draft: &ScriptDraft,
        script: &ScriptSummary,
    ) -> ScriptSummary {
        let id = script.id;
        let item = ScriptTrustItem {
            id,
            sha256: script_sha256(draft.language, &draft.body),
        };

        if let Err(error) = trust_scripts(env, runner, &[item], project).await {
            tracing::warn!(id, code = ?error.code(), "the saved memory script was not approved");
            return script.clone();
        }

        match self.get(env, id, project).await {
            Ok(detail) => detail.summary,
            Err(error) => {
                tracing::warn!(id, code = ?error.code(), "the approved memory script was not read again");
                approved(script)
            }
        }
    }

    /// The script named in `fields` in their scope, with its body, or `None` when there is none
    /// or it is a read-only team script (see the module docs).
    async fn existing(
        &self,
        env: &Env,
        project: Option<&Path>,
        fields: &Map<String, Value>,
    ) -> Result<Option<ScriptDetail>> {
        let mut arguments = Map::new();
        arguments.insert("name".to_owned(), fields["name"].clone());
        arguments.insert("scope".to_owned(), fields["scope"].clone());

        let output = self
            .call_tool(env, SCRIPT_GET_TOOL, with_project(arguments, project))
            .await?;

        let existing = found(&output)?;

        Ok(existing.filter(|script| !script.summary.read_only))
    }
}

/// `Ok` when `existing`, the script of the edit's name and scope found before the save, is the
/// version the editor opened (`base`); `conflict` when it is gone or another version.
fn unchanged(existing: Option<&ScriptDetail>, base: &str) -> Result<()> {
    match existing {
        Some(script) if script.summary.sha256 == base => Ok(()),
        Some(script) => {
            tracing::info!(
                id = script.summary.id,
                "the edited memory script changed since it was opened"
            );
            Err(BusinessError::Conflict(CHANGED_MESSAGE.to_owned()))
        }
        None => {
            tracing::info!("the edited memory script was deleted since it was opened");
            Err(BusinessError::Conflict(CHANGED_MESSAGE.to_owned()))
        }
    }
}

/// Whether `stored`, saved from `draft` over `replaced` (the script of that name and scope before
/// the save, if any; for an `edit`, the version the editor opened), holds code the user put in
/// (see the module docs).
fn users_code(draft: &ScriptDraft, stored: &ScriptSummary, replaced: Option<&ScriptDetail>, edit: bool) -> bool {
    if !is_stored_draft(draft, stored, replaced) {
        return false;
    }

    let Some(replaced) = replaced else {
        return !edit;
    };

    let same_code = normalised(&replaced.body) == normalised(&draft.body);

    if edit {
        return !same_code;
    }

    !same_code || replaced.summary.trusted
}

/// Whether `stored` is `draft` saved over `replaced`: its hash, name and scope, no repository for
/// a global script, and for a repository script the repository of `replaced` (when there is one).
fn is_stored_draft(draft: &ScriptDraft, stored: &ScriptSummary, replaced: Option<&ScriptDetail>) -> bool {
    let name = draft.name.trim();
    let is_draft = |script: &ScriptSummary| script.name == name && script.scope == draft.scope;

    if stored.sha256 != script_sha256(draft.language, &draft.body) || !is_draft(stored) {
        return false;
    }

    if replaced.is_some_and(|replaced| !is_draft(&replaced.summary)) {
        return false;
    }

    if draft.scope == ScriptScope::Global {
        return stored.repo.is_none();
    }

    match replaced {
        Some(replaced) => stored.repo.is_some() && stored.repo == replaced.summary.repo,
        None => stored.repo.is_some(),
    }
}

/// Whether `draft` holds the code of `existing`, an untrusted script, with other line endings or
/// trailing whitespace only (the textarea rewrites line endings as soon as it is typed in).
fn respaced(draft: &ScriptDraft, existing: &ScriptDetail) -> bool {
    !existing.summary.trusted && draft.body != existing.body && normalised(&draft.body) == normalised(&existing.body)
}

/// `body` as the editor shows the code: line endings as LF, no trailing spaces or tabs on a line,
/// no trailing blank lines.
fn normalised(body: &str) -> String {
    let unified = body.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = unified
        .split('\n')
        .map(|line| line.trim_end_matches([' ', '\t']))
        .collect();

    lines.join("\n").trim_end_matches('\n').to_owned()
}

/// `script` as the server answers it once the user approved its version.
fn approved(script: &ScriptSummary) -> ScriptSummary {
    ScriptSummary {
        trusted: true,
        approval: ScriptApproval::Approved,
        ..script.clone()
    }
}

/// The fields of `draft` as `script_save` takes them (name and description trimmed), or
/// `invalidInput` for what the server would refuse.
fn checked(draft: &ScriptDraft, project: Option<&Path>) -> Result<Map<String, Value>> {
    let name = draft.name.trim();
    let description = draft.description.trim();

    if !is_valid_name(name) {
        return Err(invalid(format!(
            "a script name is 1 to {SCRIPT_NAME_MAX} of a-z, 0-9, _ and -, starting with a letter or digit"
        )));
    }

    if description.is_empty() || description.chars().count() > SCRIPT_DESCRIPTION_MAX {
        return Err(invalid(format!(
            "a script description is 1 to {SCRIPT_DESCRIPTION_MAX} characters"
        )));
    }

    if draft.body.trim().is_empty() || draft.body.len() > SCRIPT_BODY_MAX {
        return Err(invalid(format!("a script body is 1 to {SCRIPT_BODY_MAX} bytes")));
    }

    if !(1..=SCRIPT_TIMEOUT_MAX).contains(&draft.timeout_secs) {
        return Err(invalid(format!(
            "a script timeout is 1 to {SCRIPT_TIMEOUT_MAX} seconds"
        )));
    }

    if draft.scope == ScriptScope::Repo && project.is_none() {
        return Err(invalid("a repository script needs a project".to_owned()));
    }

    let mut fields = Map::new();
    fields.insert("scope".to_owned(), json!(draft.scope));
    fields.insert("name".to_owned(), json!(name));
    fields.insert("description".to_owned(), json!(description));
    fields.insert("language".to_owned(), json!(draft.language));
    fields.insert("body".to_owned(), json!(draft.body));
    fields.insert("timeout_secs".to_owned(), json!(draft.timeout_secs));

    Ok(fields)
}

/// Harness Memory's script names: `[a-z0-9]`, then up to 63 of `[a-z0-9_-]`.
fn is_valid_name(name: &str) -> bool {
    let mut characters = name.chars();
    let first = characters
        .next()
        .is_some_and(|character| character.is_ascii_lowercase() || character.is_ascii_digit());
    let rest = characters.all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_' || character == '-'
    });

    first && rest && name.len() <= SCRIPT_NAME_MAX
}

fn invalid(message: String) -> BusinessError {
    BusinessError::InvalidInput(message)
}

/// The script a `script_get` by name found; `None` when it answered that there is none.
fn found(output: &ToolOutput) -> Result<Option<ScriptDetail>> {
    if output.is_error {
        let text = tool_error_text(output);

        if text.trim_start().starts_with(MISSING_PREFIX) {
            return Ok(None);
        }

        return Err(BusinessError::Process(format!("{SCRIPT_GET_TOOL}: {text}")));
    }

    let script = output
        .structured
        .as_ref()
        .and_then(|data| data.get("script"))
        .cloned()
        .ok_or_else(|| BusinessError::Protocol(format!("{SCRIPT_GET_TOOL} answered without the script")))?;

    parse(script, SCRIPT_GET_TOOL).map(Some)
}

/// What a `script_save` answer says: the script saved, and whether it is new.
fn saved_from(output: &ToolOutput) -> Result<ScriptSaved> {
    if output.is_error {
        return Err(BusinessError::Process(format!(
            "{SCRIPT_SAVE_TOOL}: {}",
            tool_error_text(output)
        )));
    }

    let data = output.structured.as_ref();
    let field = |name: &str| data.and_then(|data| data.get(name)).cloned();
    let missing = |name: &str| BusinessError::Protocol(format!("{SCRIPT_SAVE_TOOL} answered without `{name}`"));

    let script: ScriptSummary = parse(field("script").ok_or_else(|| missing("script"))?, SCRIPT_SAVE_TOOL)?;
    let created = field("created")
        .and_then(|created| created.as_bool())
        .ok_or_else(|| missing("created"))?;

    let outcome = if created {
        ScriptSaveOutcome::Created
    } else {
        ScriptSaveOutcome::Replaced
    };

    Ok(ScriptSaved {
        outcome,
        id: script.id,
        script: Some(script),
    })
}

#[cfg(test)]
mod tests;
