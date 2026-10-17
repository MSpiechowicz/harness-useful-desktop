//! `tools/call`: argument parsing, the hook drafts written on behalf of the agent, and the JSON
//! replies. Every write goes through [`HookStore`] as [`HookAuthor::Agent`], which keeps the
//! agent rules: pending until approved, the sha256 precondition, removal only of hooks an agent
//! created, and no write to the app's config folder.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use super::Server;
use crate::error::BusinessError;
use crate::hooks::{HookStore, MAX_HOOKS_PER_SCOPE, project_identity};
use crate::model::{HookAction, HookApproval, HookAuthor, HookDraft, HookEvent, HookScope, HookView};
use crate::security::redact_text;

/// Sent with every hook shown: hooks an agent writes wait for the user.
const NOTE: &str = "Hooks run in Harness Desktop chats. A hook an agent added or changed is pending: it runs \
                    only after the user approves that exact version in Harness Desktop's Hooks view.";

/// Sent when there is no hooks folder.
const UNAVAILABLE: &str = "hooks are not available on this computer (no folder for them)";

/// A tool result: the reply object (as text and as `structuredContent`), or a refusal.
pub(super) type Reply = Result<Value, String>;

/// Runs tool `name`; `None` when there is no such tool.
pub(super) fn call(server: &Server, name: &str, arguments: Value) -> Option<Reply> {
    let reply = match name {
        "hook_list" => parse(arguments).and_then(|args| list(server, args)),
        "hook_get" => parse(arguments).and_then(|args| get(server, args)),
        "hook_add" => parse(arguments).and_then(|args| add(server, args)),
        "hook_update" => parse(arguments).and_then(|args| update(server, args)),
        "hook_remove" => parse(arguments).and_then(|args| remove(server, args)),
        _ => return None,
    };

    Some(reply)
}

/// The action types an agent names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ActionKind {
    AddContext,
    FollowUp,
    Command,
}

impl ActionKind {
    fn of(action: &HookAction) -> Self {
        match action {
            HookAction::AddContext { .. } => Self::AddContext,
            HookAction::FollowUp { .. } => Self::FollowUp,
            HookAction::Command { .. } => Self::Command,
        }
    }
}

/// Which hooks `hook_list` shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ListScope {
    Global,
    Project,
    #[default]
    All,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListArgs {
    project: Option<String>,
    #[serde(default)]
    scope: ListScope,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GetArgs {
    id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AddArgs {
    name: String,
    event: HookEvent,
    action: ActionKind,
    text: Option<String>,
    command: Option<String>,
    timeout_secs: Option<u32>,
    matcher: Option<String>,
    scope: HookScope,
    project: Option<String>,
    description: Option<String>,
    enabled: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UpdateArgs {
    id: String,
    sha256: String,
    name: Option<String>,
    event: Option<HookEvent>,
    action: Option<ActionKind>,
    text: Option<String>,
    command: Option<String>,
    timeout_secs: Option<u32>,
    matcher: Option<String>,
    scope: Option<HookScope>,
    project: Option<String>,
    description: Option<String>,
    enabled: Option<bool>,
}

impl UpdateArgs {
    /// True when the call names nothing to change.
    fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.event.is_none()
            && self.action.is_none()
            && self.text.is_none()
            && self.command.is_none()
            && self.timeout_secs.is_none()
            && self.matcher.is_none()
            && self.scope.is_none()
            && self.project.is_none()
            && self.description.is_none()
            && self.enabled.is_none()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoveArgs {
    id: String,
    sha256: String,
}

fn list(server: &Server, args: ListArgs) -> Reply {
    let store = store(server)?;
    let explicit = args.project.is_some();

    let project = match server.project_root(args.project.as_deref()) {
        Ok(root) => Some(root),
        // Outside a usable folder only the global hooks apply.
        Err(_) if !explicit && args.scope != ListScope::Project => None,
        Err(problem) => return Err(problem),
    };

    // Another project's hooks are never shown, not even those over its limit.
    let listing = store.list_for(server.capabilities, project.as_deref());

    if let Some(problem) = listing.folder_problem {
        return Err(format!("the hooks folder is not read: {problem}"));
    }

    let applies = |view: &HookView| match &view.hook {
        Some(hook) => match hook.scope {
            HookScope::Global => args.scope != ListScope::Project,
            HookScope::Project => args.scope != ListScope::Global && hook.project == project,
        },
        // An invalid file (of this chat's scopes when it is over a limit, otherwise of an unknown
        // scope) is shown with every hook, so it can be fixed.
        None => args.scope == ListScope::All,
    };

    let shown: Vec<&HookView> = listing.hooks.iter().filter(|view| applies(view)).collect();
    let count = |scope: HookScope| {
        shown
            .iter()
            .filter(|view| view.hook.as_ref().is_some_and(|hook| hook.scope == scope))
            .count()
    };

    Ok(json!({
        "project": project,
        "hooks": shown.iter().map(|view| hook_json(view)).collect::<Vec<_>>(),
        "global": format!("{} of {MAX_HOOKS_PER_SCOPE}", count(HookScope::Global)),
        "projectHooks": format!("{} of {MAX_HOOKS_PER_SCOPE}", count(HookScope::Project)),
        "note": NOTE,
    }))
}

fn get(server: &Server, args: GetArgs) -> Reply {
    let view = store(server)?.get(&args.id, server.capabilities).map_err(refusal)?;

    Ok(json!({ "hook": hook_json(&view), "note": NOTE }))
}

fn add(server: &Server, args: AddArgs) -> Reply {
    let store = store(server)?;

    let project = match args.scope {
        HookScope::Global => {
            no_project_for_global(args.project.as_deref())?;
            None
        }
        HookScope::Project => Some(server.project_root(args.project.as_deref())?),
    };

    let action = action(args.action, args.text, args.command, args.timeout_secs, None)?;

    let draft = HookDraft {
        id: None,
        expected_sha256: None,
        name: args.name,
        description: args.description,
        scope: args.scope,
        project,
        event: args.event,
        matcher: args.matcher,
        action,
        enabled: args.enabled.unwrap_or(true),
    };

    let view = store
        .save(&draft, HookAuthor::Agent, server.capabilities, SystemTime::now())
        .map_err(refusal)?;

    Ok(json!({ "added": hook_json(&view), "note": NOTE }))
}

fn update(server: &Server, args: UpdateArgs) -> Reply {
    let store = store(server)?;

    if args.is_empty() {
        return Err("name at least one field to change".to_owned());
    }

    let view = store.get(&args.id, server.capabilities).map_err(refusal)?;

    if view.sha256 != args.sha256 {
        return Err(refusal(BusinessError::Conflict(String::new())));
    }

    let Some(current) = view.hook else {
        let problem = view.problem.unwrap_or_default();
        return Err(format!(
            "the hook file is invalid ({problem}): ask the user to fix or delete it in Harness Desktop's Hooks view"
        ));
    };

    let event = args.event.unwrap_or(current.event);
    let kind = args.action.unwrap_or_else(|| ActionKind::of(&current.action));
    let action = action(kind, args.text, args.command, args.timeout_secs, Some(&current.action))?;

    let scope = args.scope.unwrap_or(current.scope);
    let project = match scope {
        HookScope::Global => {
            no_project_for_global(args.project.as_deref())?;
            None
        }
        HookScope::Project => match (args.project.as_deref(), &current.project) {
            (Some(folder), _) => Some(server.project_root(Some(folder))?),
            // The project root it has is kept as it is.
            (None, Some(root)) => Some(root.clone()),
            (None, None) => Some(server.project_root(None)?),
        },
    };

    // Where the user's hook applies stays the user's choice.
    if current.created_by == HookAuthor::Desktop && (scope != current.scope || project != current.project) {
        return Err(
            "this hook was created in Harness Desktop: its scope and project stay as they are, ask the user to \
             change them"
                .to_owned(),
        );
    }

    // A matcher belongs to toolEnd: moving the hook to another event drops it.
    let matcher = match args.matcher {
        Some(matcher) => Some(matcher),
        None if event == HookEvent::ToolEnd => current.matcher,
        None => None,
    };

    let draft = HookDraft {
        id: Some(args.id),
        expected_sha256: Some(args.sha256),
        name: args.name.unwrap_or(current.name),
        description: args.description.or(current.description),
        scope,
        project,
        event,
        matcher,
        action,
        enabled: args.enabled.unwrap_or(current.enabled),
    };

    let view = store
        .save(&draft, HookAuthor::Agent, server.capabilities, SystemTime::now())
        .map_err(refusal)?;

    Ok(json!({ "updated": hook_json(&view), "note": NOTE }))
}

fn remove(server: &Server, args: RemoveArgs) -> Reply {
    store(server)?
        .delete(&args.id, HookAuthor::Agent, Some(&args.sha256))
        .map_err(refusal)?;

    Ok(json!({
        "removed": args.id,
        "note": "The hook is removed. Harness Desktop no longer runs it.",
    }))
}

/// The action an agent describes. `current` is the hook's action on an edit: its text, command
/// and timeout are kept when the type stays and the call leaves them out.
fn action(
    kind: ActionKind,
    text: Option<String>,
    command: Option<String>,
    timeout_secs: Option<u32>,
    current: Option<&HookAction>,
) -> Result<HookAction, String> {
    match kind {
        ActionKind::AddContext | ActionKind::FollowUp => {
            if command.is_some() || timeout_secs.is_some() {
                return Err("command and timeoutSecs belong to the action command, give text instead".to_owned());
            }

            let kept = match current {
                Some(HookAction::AddContext { text } | HookAction::FollowUp { text }) => Some(text.clone()),
                _ => None,
            };
            let Some(text) = text.or(kept) else {
                return Err("addContext and followUp need text".to_owned());
            };

            if kind == ActionKind::AddContext {
                Ok(HookAction::AddContext { text })
            } else {
                Ok(HookAction::FollowUp { text })
            }
        }
        ActionKind::Command => {
            if text.is_some() {
                return Err("text belongs to addContext and followUp, give command instead".to_owned());
            }

            let (kept_command, kept_timeout) = match current {
                Some(HookAction::Command { command, timeout_secs }) => (Some(command.clone()), *timeout_secs),
                _ => (None, None),
            };
            let Some(command) = command.or(kept_command) else {
                return Err("the action command needs command".to_owned());
            };

            Ok(HookAction::Command {
                command,
                timeout_secs: timeout_secs.or(kept_timeout),
            })
        }
    }
}

fn no_project_for_global(project: Option<&str>) -> Result<(), String> {
    match project {
        Some(_) => Err("project belongs to scope project, a global hook applies to every project".to_owned()),
        None => Ok(()),
    }
}

impl Server {
    /// The project root of `project` (absolute) or of the server's working folder.
    fn project_root(&self, project: Option<&str>) -> Result<String, String> {
        let folder = self.folder(project)?;
        let root = project_identity(&folder).map_err(|error| project_problem(&folder, &error))?;

        root.into_os_string()
            .into_string()
            .map_err(|_| "the project path is not valid UTF-8".to_owned())
    }

    fn folder(&self, project: Option<&str>) -> Result<PathBuf, String> {
        match project {
            Some(folder) if Path::new(folder).is_absolute() => Ok(PathBuf::from(folder)),
            Some(_) => Err("project must be an absolute folder path".to_owned()),
            None => self
                .cwd
                .clone()
                .ok_or_else(|| "the server's working folder is unknown: pass project".to_owned()),
        }
    }
}

fn project_problem(folder: &Path, error: &BusinessError) -> String {
    match error {
        BusinessError::InvalidInput(_) => format!("{} is not a folder", folder.display()),
        _ => format!("{} does not exist", folder.display()),
    }
}

fn store(server: &Server) -> Result<&HookStore, String> {
    server.store.as_ref().ok_or_else(|| UNAVAILABLE.to_owned())
}

/// The refusal text of a store error, telling the agent what to do next.
fn refusal(error: BusinessError) -> String {
    match error {
        BusinessError::Conflict(_) => {
            "the hook changed since you read it: call hook_get for its current sha256, check the change, then retry"
                .to_owned()
        }
        BusinessError::NotFound(what) if what.starts_with("the hook") => "no hook has this id".to_owned(),
        BusinessError::InvalidInput(problem) | BusinessError::Forbidden(problem) | BusinessError::NotFound(problem) => {
            problem
        }
        other => other.to_string(),
    }
}

/// One hook as the agent sees it: `id`, `name`, `status`, `approval` (`approved` or `pending`),
/// `sha256`, then the hook's fields, or `problem` for an invalid file.
fn hook_json(view: &HookView) -> Value {
    let approval = match view.approval {
        HookApproval::Approved => "approved",
        HookApproval::Changed | HookApproval::Never => "pending",
    };

    let mut object = Map::new();
    object.insert("id".to_owned(), json!(view.id));
    object.insert("name".to_owned(), json!(view.name));
    object.insert("status".to_owned(), json!(view.status));
    object.insert("approval".to_owned(), json!(approval));
    object.insert("sha256".to_owned(), json!(view.sha256));

    if let Some(Value::Object(fields)) = view.hook.as_ref().and_then(|hook| serde_json::to_value(hook).ok()) {
        for (key, value) in fields {
            object.entry(key).or_insert(value);
        }
    }

    // A secret pasted into a hook is not shown to the agent.
    if let Some(Value::Object(action)) = object.get_mut("action") {
        for field in ["text", "command"] {
            if let Some(Value::String(text)) = action.get_mut(field) {
                *text = redact_text(text);
            }
        }
    }

    if let Some(problem) = &view.problem {
        object.insert("problem".to_owned(), json!(problem));
    }

    Value::Object(object)
}

fn parse<T: DeserializeOwned>(arguments: Value) -> Result<T, String> {
    serde_json::from_value(arguments).map_err(|error| format!("invalid arguments: {error}"))
}
