//! The routine tools' `tools/call`: the flat arguments turned into a routine's `when` and `action`,
//! the drafts written on behalf of the agent, and the JSON replies. Every write goes through
//! [`RoutineStore`] as [`HookAuthor::Agent`], which keeps the agent rules of the format doc:
//! pending until approved, the sha256 precondition, `createdBy` kept, removal only of routines an
//! agent created, and no write to the app's config folder. Every tool sees only the global
//! routines and those of the server's own project: another project's routine has no id here. Team routines and run records
//! are never read, and no tool runs a routine.

use std::time::SystemTime;

use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::Server;
use super::common::{
    DocView, GetArgs, ListArgs, RemoveArgs, Reply, editable, list_reply, parse, refusal, remove, store_of, view_json,
};
use crate::model::{
    ApprovalMode, HarnessId, HookAuthor, HookScope, RoutineAction, RoutineDraft, RoutineModel, RoutineView,
    RoutineWhen, RoutineWhenKind, ScriptScope,
};
use crate::routines::format::{check_action, check_when, takes_approval_mode, takes_trust_once};
use crate::routines::{RoutineKind, RoutineStore};

/// Sent with every routine shown: routines an agent writes wait for the user.
const NOTE: &str = "Routines run on their schedule while Harness Desktop runs. A routine an agent added or \
                    changed is pending: it runs only after the user approves that exact version in Harness \
                    Desktop's Routines view.";

/// Runs routine tool `name`; `None` when there is no such tool.
pub(super) fn call(server: &Server, name: &str, arguments: Value) -> Option<Reply> {
    let reply = match name {
        "routine_list" => parse(arguments).and_then(|args| list(server, args)),
        "routine_get" => parse(arguments).and_then(|args| get(server, args)),
        "routine_add" => parse(arguments).and_then(|fields| add(server, fields)),
        "routine_update" => update(server, arguments),
        "routine_remove" => parse(arguments).and_then(|args: RemoveArgs| {
            shown(server, &args.id)?;
            remove(server.routines.as_ref(), args)
        }),
        _ => return None,
    };

    Some(reply)
}

/// The action types an agent names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ActionKind {
    Agent,
    Command,
    Script,
}

impl ActionKind {
    fn of(action: &RoutineAction) -> Self {
        match action {
            RoutineAction::Agent { .. } => Self::Agent,
            RoutineAction::Command { .. } => Self::Command,
            RoutineAction::Script { .. } => Self::Script,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Command => "command",
            Self::Script => "script",
        }
    }
}

/// `approvalMode` as an agent names it: a mode, or `default` for the harness's setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ModeArg {
    AlwaysAsk,
    Write,
    Yolo,
    Auto,
    Default,
}

impl ModeArg {
    fn mode(self) -> Option<ApprovalMode> {
        match self {
            Self::AlwaysAsk => Some(ApprovalMode::AlwaysAsk),
            Self::Write => Some(ApprovalMode::Write),
            Self::Yolo => Some(ApprovalMode::Yolo),
            Self::Auto => Some(ApprovalMode::Auto),
            Self::Default => None,
        }
    }
}

/// The routine fields of `routine_add` and `routine_update`, all optional here: `routine_add`
/// requires its own.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Fields {
    name: Option<String>,
    description: Option<String>,
    scope: Option<HookScope>,
    project: Option<String>,
    when: Option<RoutineWhenKind>,
    at: Option<String>,
    every: Option<u32>,
    cron: Option<String>,
    action: Option<ActionKind>,
    prompt: Option<String>,
    harness: Option<HarnessId>,
    model: Option<String>,
    approval_mode: Option<ModeArg>,
    trust_once: Option<bool>,
    max_minutes: Option<u32>,
    command: Option<String>,
    timeout_secs: Option<u32>,
    script: Option<String>,
    script_scope: Option<ScriptScope>,
    enabled: Option<bool>,
}

impl Fields {
    /// Each action field the call names, with the action type it belongs to.
    fn action_fields(&self) -> [(&'static str, ActionKind, bool); 10] {
        [
            ("prompt", ActionKind::Agent, self.prompt.is_some()),
            ("harness", ActionKind::Agent, self.harness.is_some()),
            ("model", ActionKind::Agent, self.model.is_some()),
            ("approvalMode", ActionKind::Agent, self.approval_mode.is_some()),
            ("trustOnce", ActionKind::Agent, self.trust_once.is_some()),
            ("maxMinutes", ActionKind::Agent, self.max_minutes.is_some()),
            ("command", ActionKind::Command, self.command.is_some()),
            ("timeoutSecs", ActionKind::Command, self.timeout_secs.is_some()),
            ("script", ActionKind::Script, self.script.is_some()),
            ("scriptScope", ActionKind::Script, self.script_scope.is_some()),
        ]
    }
}

/// Why `routine_list` refuses a project other than the server's own.
const OTHER_PROJECT: &str = "an agent sees only the global routines and those of the project it works in (the \
                             server's working folder): leave project out or name that folder";

fn list(server: &Server, args: ListArgs) -> Reply {
    let store = store(server)?;
    let project = server.list_project(&args)?;

    // Another project's routines are never shown, and team routines are not listed for agents.
    if project.is_some() && project != server.own_project() {
        return Err(OTHER_PROJECT.to_owned());
    }

    let listing = store.list_for(project.as_deref());
    let views = listing.routines.iter().map(|view| view as &dyn DocView);

    list_reply::<RoutineKind>(project, listing.folder_problem, views, args.scope, NOTE)
}

fn get(server: &Server, args: GetArgs) -> Reply {
    let view = shown(server, &args.id)?;

    Ok(json!({ "routine": view_json(&view), "note": NOTE }))
}

/// Routine `id` when `routine_list` would show it to the agent (see [`Server::shown`]).
fn shown(server: &Server, id: &str) -> Result<RoutineView, String> {
    let view = store(server)?.get(id).map_err(refusal::<RoutineKind>)?;

    server.shown::<RoutineKind, _>(view)
}

fn add(server: &Server, fields: Fields) -> Reply {
    let store = store(server)?;

    let name = required(fields.name.clone(), "name")?;
    let scope = required(fields.scope, "scope")?;
    let when_kind = required(fields.when, "when")?;
    let action_kind = required(fields.action, "action")?;

    let project = server.placement::<RoutineKind>(scope, fields.project.as_deref(), None)?;
    let when = schedule(when_kind, &fields, None)?;
    let action = action(action_kind, &fields, None, scope)?;

    let draft = RoutineDraft {
        id: None,
        expected_sha256: None,
        name,
        description: fields.description,
        scope,
        project,
        when,
        action,
        enabled: fields.enabled.unwrap_or(true),
    };

    save(store, &draft, "added")
}

fn update(server: &Server, arguments: Value) -> Reply {
    let store = store(server)?;

    let Value::Object(mut arguments) = arguments else {
        return Err("invalid arguments: expected an object".to_owned());
    };
    let id = take_text(&mut arguments, "id")?;
    let sha256 = take_text(&mut arguments, "sha256")?;

    if arguments.values().all(Value::is_null) {
        return Err("name at least one field to change".to_owned());
    }

    let fields: Fields = parse(Value::Object(arguments))?;

    let view = shown(server, &id)?;
    let current = editable::<RoutineKind, _>(&view, &sha256, view.routine.clone())?;

    let scope = fields.scope.unwrap_or(current.scope);
    let project = server.placement::<RoutineKind>(scope, fields.project.as_deref(), Some(&current))?;

    let when_kind = fields.when.unwrap_or_else(|| current.when.kind());
    let when = schedule(when_kind, &fields, Some(&current.when))?;

    let action_kind = fields.action.unwrap_or_else(|| ActionKind::of(&current.action));
    let action = action(action_kind, &fields, Some(&current.action), scope)?;

    let draft = RoutineDraft {
        id: Some(id),
        expected_sha256: Some(sha256),
        name: fields.name.unwrap_or(current.name),
        description: fields.description.or(current.description),
        scope,
        project,
        when,
        action,
        enabled: fields.enabled.unwrap_or(current.enabled),
    };

    save(store, &draft, "updated")
}

/// Writes `draft` as the agent; the reply holds the routine as written under `key`.
fn save(store: &RoutineStore, draft: &RoutineDraft, key: &str) -> Reply {
    let view = store
        .save(draft, HookAuthor::Agent, SystemTime::now())
        .map_err(refusal::<RoutineKind>)?;

    let mut reply = Map::new();
    reply.insert(key.to_owned(), view_json(&view));
    reply.insert("note".to_owned(), json!(NOTE));

    Ok(Value::Object(reply))
}

/// The schedule an agent describes. `current` is the routine's schedule on an edit: its `at` is
/// kept between daily and weekdays, its `every` and `cron` while the type stays.
fn schedule(kind: RoutineWhenKind, fields: &Fields, current: Option<&RoutineWhen>) -> Result<RoutineWhen, String> {
    use RoutineWhenKind as Kind;

    let owners = [
        (
            "at",
            fields.at.is_some(),
            matches!(kind, Kind::Daily | Kind::Weekdays),
            "daily and weekdays",
        ),
        (
            "every",
            fields.every.is_some(),
            matches!(kind, Kind::EveryMinutes | Kind::EveryHours),
            "everyMinutes and everyHours",
        ),
        ("cron", fields.cron.is_some(), kind == Kind::Cron, "cron"),
    ];

    if let Some((field, _, _, owner)) = owners.iter().find(|(_, given, fits, _)| *given && !fits) {
        return Err(format!("{field} belongs to when {owner}"));
    }

    let kept_at = match current {
        Some(RoutineWhen::Daily { at } | RoutineWhen::Weekdays { at }) => Some(at.clone()),
        _ => None,
    };
    let kept_every = match current {
        Some(RoutineWhen::EveryMinutes { every }) if kind == Kind::EveryMinutes => Some(*every),
        Some(RoutineWhen::EveryHours { every }) if kind == Kind::EveryHours => Some(*every),
        _ => None,
    };
    let kept_cron = match current {
        Some(RoutineWhen::Cron { expression }) => Some(expression.clone()),
        _ => None,
    };

    let what = format!("when {}", kind.as_str());
    let at = || needed(fields.at.clone().or(kept_at.clone()), &what, "at");
    let every = || needed(fields.every.or(kept_every), &what, "every");

    let when = match kind {
        Kind::Daily => RoutineWhen::Daily { at: at()? },
        Kind::Weekdays => RoutineWhen::Weekdays { at: at()? },
        Kind::EveryMinutes => RoutineWhen::EveryMinutes { every: every()? },
        Kind::EveryHours => RoutineWhen::EveryHours { every: every()? },
        Kind::Cron => RoutineWhen::Cron {
            expression: needed(fields.cron.clone().or(kept_cron), &what, "cron")?,
        },
        Kind::AppStart => RoutineWhen::AppStart,
    };

    check_when(&when)?;

    Ok(when)
}

/// The action an agent describes, for a routine of `scope`. `current` is the routine's action on
/// an edit: its fields are kept while the type stays and the call leaves them out.
fn action(
    kind: ActionKind,
    fields: &Fields,
    current: Option<&RoutineAction>,
    scope: HookScope,
) -> Result<RoutineAction, String> {
    let foreign = fields
        .action_fields()
        .into_iter()
        .find(|(_, owner, given)| *given && *owner != kind);

    if let Some((field, owner, _)) = foreign {
        return Err(format!(
            "{field} belongs to the action {}, not {}",
            owner.as_str(),
            kind.as_str()
        ));
    }

    let what = format!("the action {}", kind.as_str());

    let action = match (kind, current) {
        (ActionKind::Agent, _) => agent(fields, current, &what)?,
        (ActionKind::Command, Some(RoutineAction::Command { command, timeout_secs })) => RoutineAction::Command {
            command: fields.command.clone().unwrap_or_else(|| command.clone()),
            timeout_secs: fields.timeout_secs.or(*timeout_secs),
        },
        (ActionKind::Command, _) => RoutineAction::Command {
            command: needed(fields.command.clone(), &what, "command")?,
            timeout_secs: fields.timeout_secs,
        },
        (ActionKind::Script, Some(RoutineAction::Script { name, scope })) => RoutineAction::Script {
            name: fields.script.clone().unwrap_or_else(|| name.clone()),
            scope: fields.script_scope.unwrap_or(*scope),
        },
        (ActionKind::Script, _) => RoutineAction::Script {
            name: needed(fields.script.clone(), &what, "script")?,
            scope: needed(fields.script_scope, &what, "scriptScope")?,
        },
    };

    check_action(&action, scope)?;

    Ok(action)
}

/// The agent action of `fields`, the fields of the `current` agent action kept. A kept approval
/// mode is dropped when the harness becomes pi, a kept trustOnce when it becomes omp or opencode.
fn agent(fields: &Fields, current: Option<&RoutineAction>, what: &str) -> Result<RoutineAction, String> {
    let (kept_harness, kept_model, kept_prompt, kept_mode, kept_trust, kept_minutes) = match current {
        Some(RoutineAction::Agent {
            harness,
            model,
            prompt,
            approval_mode,
            trust_once,
            max_minutes,
        }) => (
            Some(*harness),
            model.clone(),
            Some(prompt.clone()),
            *approval_mode,
            *trust_once,
            *max_minutes,
        ),
        _ => (None, None, None, None, false, None),
    };

    let harness = needed(fields.harness.or(kept_harness), what, "harness")?;
    let prompt = needed(fields.prompt.clone().or(kept_prompt), what, "prompt")?;

    let model = match fields.model.as_deref() {
        Some(model) => parse_model(model)?,
        None => kept_model,
    };

    let approval_mode = match fields.approval_mode {
        Some(mode) => mode.mode(),
        None => kept_mode.filter(|_| takes_approval_mode(harness)),
    };

    Ok(RoutineAction::Agent {
        harness,
        model,
        prompt,
        approval_mode,
        trust_once: fields.trust_once.unwrap_or(kept_trust && takes_trust_once(harness)),
        max_minutes: fields.max_minutes.or(kept_minutes),
    })
}

/// `provider/id` as a model (the id may hold more slashes); empty means the harness's default.
fn parse_model(model: &str) -> Result<Option<RoutineModel>, String> {
    if model.is_empty() {
        return Ok(None);
    }

    match model.split_once('/') {
        Some((provider, id)) => Ok(Some(RoutineModel {
            provider: provider.to_owned(),
            id: id.to_owned(),
            thinking: None,
        })),
        None => Err("model must be provider/id, e.g. anthropic/claude-sonnet-4-5".to_owned()),
    }
}

fn required<T>(value: Option<T>, field: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("invalid arguments: missing field `{field}`"))
}

fn needed<T>(value: Option<T>, what: &str, field: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("{what} needs {field}"))
}

/// The string argument `field`, taken out of `arguments`.
fn take_text(arguments: &mut Map<String, Value>, field: &str) -> Result<String, String> {
    match arguments.remove(field) {
        Some(Value::String(text)) => Ok(text),
        None => Err(format!("invalid arguments: missing field `{field}`")),
        Some(_) => Err(format!("invalid arguments: {field} must be a string")),
    }
}

fn store(server: &Server) -> Result<&RoutineStore, String> {
    store_of(server.routines.as_ref())
}
