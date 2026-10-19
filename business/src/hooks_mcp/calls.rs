//! `tools/call`: argument parsing, the hook drafts written on behalf of the agent, and the JSON
//! replies. Every write goes through [`HookStore`] as [`HookAuthor::Agent`], which keeps the
//! agent rules: pending until approved, the sha256 precondition, removal only of hooks an agent
//! created, and no write to the app's config folder. `hook_get`, `hook_update` and `hook_remove`
//! see only the global hooks and those of the server's own project: another project's hook has no
//! id here.

use std::time::SystemTime;

use serde::Deserialize;
use serde_json::{Value, json};

use super::Server;
use super::common::{
    DocView, GetArgs, ListArgs, RemoveArgs, Reply, editable, list_reply, parse, refusal, remove, store_of, view_json,
};
use super::routine_calls;
use crate::hooks::{HookKind, HookStore};
use crate::model::{HookAction, HookAuthor, HookDraft, HookEvent, HookScope, HookView};

/// Sent with every hook shown: hooks an agent writes wait for the user.
const NOTE: &str = "Hooks run in Harness Desktop chats. A hook an agent added or changed is pending: it runs \
                    only after the user approves that exact version in Harness Desktop's Hooks view.";

/// Runs tool `name` (a hook tool or a routine tool); `None` when there is no such tool.
pub(super) fn call(server: &Server, name: &str, arguments: Value) -> Option<Reply> {
    let reply = match name {
        "hook_list" => parse(arguments).and_then(|args| list(server, args)),
        "hook_get" => parse(arguments).and_then(|args| get(server, args)),
        "hook_add" => parse(arguments).and_then(|args| add(server, args)),
        "hook_update" => parse(arguments).and_then(|args| update(server, args)),
        "hook_remove" => parse(arguments).and_then(|args: RemoveArgs| {
            shown(server, &args.id)?;
            remove(server.store.as_ref(), args)
        }),
        _ => return routine_calls::call(server, name, arguments),
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

fn list(server: &Server, args: ListArgs) -> Reply {
    let store = store(server)?;
    let project = server.list_project(&args)?;

    // Another project's hooks are never shown, not even those over its limit.
    let listing = store.list_for(server.capabilities, project.as_deref());
    let views = listing.hooks.iter().map(|view| view as &dyn DocView);

    list_reply::<HookKind>(project, listing.folder_problem, views, args.scope, NOTE)
}

fn get(server: &Server, args: GetArgs) -> Reply {
    let view = shown(server, &args.id)?;

    Ok(json!({ "hook": view_json(&view), "note": NOTE }))
}

/// Hook `id` when `hook_list` would show it to the agent (see [`Server::shown`]).
fn shown(server: &Server, id: &str) -> Result<HookView, String> {
    let view = store(server)?
        .get(id, server.capabilities)
        .map_err(refusal::<HookKind>)?;

    server.shown::<HookKind, _>(view)
}

fn add(server: &Server, args: AddArgs) -> Reply {
    let store = store(server)?;

    let project = server.placement::<HookKind>(args.scope, args.project.as_deref(), None)?;
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
        .map_err(refusal::<HookKind>)?;

    Ok(json!({ "added": view_json(&view), "note": NOTE }))
}

fn update(server: &Server, args: UpdateArgs) -> Reply {
    let store = store(server)?;

    if args.is_empty() {
        return Err("name at least one field to change".to_owned());
    }

    let view = shown(server, &args.id)?;
    let current = editable::<HookKind, _>(&view, &args.sha256, view.hook.clone())?;

    let scope = args.scope.unwrap_or(current.scope);
    let project = server.placement::<HookKind>(scope, args.project.as_deref(), Some(&current))?;

    let event = args.event.unwrap_or(current.event);
    let kind = args.action.unwrap_or_else(|| ActionKind::of(&current.action));
    let action = action(kind, args.text, args.command, args.timeout_secs, Some(&current.action))?;

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
        .map_err(refusal::<HookKind>)?;

    Ok(json!({ "updated": view_json(&view), "note": NOTE }))
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

fn store(server: &Server) -> Result<&HookStore, String> {
    store_of(server.store.as_ref())
}
