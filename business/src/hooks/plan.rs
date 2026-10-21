//! What the engine hands the shell: the hooks that apply to a chat ([`HookSet`]), and the
//! firings to carry out ([`Firing`]: fixed texts and commands to run, in run order).

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Duration;

use super::compose::ResolvedPart;
use super::engine::Announcement;
use super::format::DEFAULT_TIMEOUT_SECS;
use super::view::{RunOrder, order_key, team_order_key};
use crate::model::{Hook, HookAction, HookEvent, HookScope, HookStatus, HookView};

mod env_values;

use env_values::{project_value, tool_value, touched_value};

/// Largest `HARNESS_HOOK_TOUCHED` value, in bytes; later paths are left out.
pub const MAX_TOUCHED_BYTES: usize = 16 * 1024;

/// The chat a [`super::HookSession`] runs hooks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatContext {
    /// The desktop session id (`HARNESS_HOOK_SESSION`).
    pub session_id: String,
    /// The chat's folder: where global command hooks run.
    pub folder: PathBuf,
    /// The chat's project root ([`super::project_identity`] of its folder), when it has one.
    /// Project hooks with this `project` apply; it is `HARNESS_HOOK_PROJECT` too.
    pub project: Option<String>,
    /// A new chat (no saved session resumed): its first eligible prompt fires `sessionStart`.
    pub fresh: bool,
    /// A utility agent (titles, reviews): no hook ever fires.
    pub utility: bool,
}

/// The hooks that run in one chat: active (valid, enabled, approved or trusted, requirements
/// met), global or of the chat's project, in run order: the local hooks (global, then project),
/// then the team hooks (global, then project).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HookSet {
    pub(super) hooks: Vec<SetHook>,
}

/// One hook of a [`HookSet`] and where it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SetHook {
    pub hook: Hook,
    pub order: RunOrder,
}

impl HookSet {
    /// No hooks.
    pub fn empty() -> Self {
        Self::default()
    }

    /// The active hooks of `views` (local files) that apply to a chat in `project` (`None`: global
    /// hooks only).
    pub fn for_chat(views: &[HookView], project: Option<&str>) -> Self {
        let hooks = active(views)
            .filter(|hook| match hook.scope {
                HookScope::Global => true,
                HookScope::Project => project.is_some() && hook.project.as_deref() == project,
            })
            .map(|hook| SetHook {
                order: order_key(hook),
                hook: hook.clone(),
            })
            .collect();

        let mut set = Self { hooks };
        set.sort();
        set
    }

    /// Adds the active team hooks of `team` that apply to a chat in `project`, whose repository
    /// key is `key`: the global ones, and the project ones of that repository, which run in
    /// `project` like its own project hooks. Team hooks run after every local one.
    pub fn with_team(mut self, team: &[HookView], project: Option<&str>, key: Option<&str>) -> Self {
        let team = active(team).filter_map(|hook| match (hook.scope, project, key) {
            (HookScope::Global, _, _) => Some(hook.clone()),
            (HookScope::Project, Some(root), Some(key)) if hook.project.as_deref() == Some(key) => Some(Hook {
                project: Some(root.to_owned()),
                ..hook.clone()
            }),
            (HookScope::Project, _, _) => None,
        });

        self.hooks.extend(team.map(|hook| SetHook {
            order: team_order_key(&hook),
            hook,
        }));
        self.sort();
        self
    }

    fn sort(&mut self) {
        self.hooks.sort_by(|left, right| left.order.cmp(&right.order));
    }

    /// True when no hook applies.
    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty()
    }

    /// The hooks of `event`, in run order.
    pub(crate) fn of_event(&self, event: HookEvent) -> impl Iterator<Item = &SetHook> {
        self.hooks.iter().filter(move |entry| entry.hook.event == event)
    }
}

/// The hooks of the active views.
fn active(views: &[HookView]) -> impl Iterator<Item = &Hook> {
    views
        .iter()
        .filter(|view| view.status == HookStatus::Active)
        .filter_map(|view| view.hook.as_ref())
}

/// True when `matcher` (absent: every tool) names `tool`, ASCII case-insensitively.
pub(crate) fn matches_tool(matcher: Option<&str>, tool: &str) -> bool {
    match matcher {
        None => true,
        Some(matcher) => matcher.split('|').any(|name| name.eq_ignore_ascii_case(tool)),
    }
}

/// A command hook to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRequest {
    /// The hook's id.
    pub hook_id: String,
    /// The hook's name.
    pub hook_name: String,
    /// The event it fires for.
    pub event: HookEvent,
    /// The command text (run by `sh -c` or `cmd.exe /D /S /C`).
    pub command: String,
    /// Its own limit (`timeoutSecs`, default 10 s), cut short to the firing's budget left.
    pub timeout: Duration,
    /// The project root for a project hook, the chat's folder for a global one.
    pub cwd: PathBuf,
    /// The `HARNESS_HOOK_*` variables.
    pub env: Vec<(String, String)>,
    pub(crate) order: RunOrder,
}

/// One step of a firing, in run order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannedPart {
    /// A fixed text (an `addContext` or `followUp` hook, or a command output already collected).
    Text(ResolvedPart),
    /// A command to run; its output becomes this hook's part.
    Command(CommandRequest),
}

impl PlannedPart {
    /// The id of the hook this part belongs to.
    pub fn hook_id(&self) -> &str {
        match self {
            Self::Text(part) => &part.hook_id,
            Self::Command(request) => &request.hook_id,
        }
    }

    fn order(&self) -> &RunOrder {
        match self {
            Self::Text(part) => &part.order,
            Self::Command(request) => &request.order,
        }
    }
}

/// The hooks of one event firing: run them one after another in this order (commands share a
/// 30 s budget, see [`super::run_firing`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Firing {
    /// The event.
    pub event: HookEvent,
    /// The parts in run order.
    pub parts: Vec<PlannedPart>,
    /// The run it belongs to (for [`super::HookSession::on_tool_output`]).
    pub(crate) run: u64,
    /// Strictly increasing per session.
    pub(crate) seq: u64,
}

impl Firing {
    /// The commands to run.
    pub fn commands(&self) -> impl Iterator<Item = &CommandRequest> {
        self.parts.iter().filter_map(|part| match part {
            PlannedPart::Command(request) => Some(request),
            PlannedPart::Text(_) => None,
        })
    }

    /// True when the firing holds no part.
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }
}

/// What to do before a message goes out ([`super::HookSession::on_send`]): resolve the firings
/// (`sessionStart`, then `promptSubmit`), put [`super::compose_prompt`] of the memory settings
/// notice (when there is one) and all their parts in front of the message
/// ([`SendPlan::context_parts`]) and report what it kept
/// ([`super::HookSession::context_composed`]). Right
/// before the message is delivered, pass it to [`super::HookSession::sending`], and to
/// [`super::HookSession::send_failed`] when the message could not be delivered.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SendPlan {
    /// Firings whose parts become context, possibly empty.
    pub firings: Vec<Firing>,
    /// The chat's memory guards changed since the agent was told: this part, named
    /// [`super::engine::MEMORY_NOTICE_NAME`], goes in front of the firings' parts.
    pub memory_notice: Option<ResolvedPart>,
    /// The state the notice tells, and the one told before.
    pub(crate) announcement: Option<Announcement>,
    pub(crate) consumed_start: bool,
    /// A user message: [`super::HookSession::sending`] counts it.
    pub(crate) user_message: bool,
    /// How the session stood before [`super::HookSession::sending`] counted this message (for
    /// [`super::HookSession::send_failed`]).
    pub(crate) previous: Option<UserMark>,
}

/// What a user message changes in a session, as it was before the message.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct UserMark {
    /// The current run.
    pub run: u64,
    /// The current run's user messages.
    pub user_messages: u32,
    /// Messages waiting for a settle the engine has not handled yet.
    pub deferred: VecDeque<u64>,
    /// The follow-up that could still be sent.
    pub pending_ticket: Option<u64>,
    /// Whether the follow-up waiting for `toolEnd` commands was cancelled, when one waits.
    pub waiting_cancelled: Option<bool>,
}

impl SendPlan {
    /// True when nothing is added to the message.
    pub fn is_empty(&self) -> bool {
        self.memory_notice.is_none() && self.firings.iter().all(Firing::is_empty)
    }

    /// The parts to compose in front of the message: the memory settings notice, then `resolved`
    /// (what the firings produced).
    pub fn context_parts(&self, resolved: Vec<ResolvedPart>) -> Vec<ResolvedPart> {
        self.memory_notice.iter().cloned().chain(resolved).collect()
    }
}

/// A follow-up prompt to compose: resolve `firing` (it may hold `turnEnd` commands), compose
/// [`super::compose_follow_up`] of its parts, then send it only while
/// [`super::HookSession::take_follow_up`] with `ticket` is true, as a prompt from
/// [`super::SendOrigin::Hook`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FollowUpPlan {
    /// Identifies this follow-up; a user message sent before it goes out cancels it.
    pub ticket: u64,
    /// The `turnEnd` firing, with the `toolEnd` contributions of the run merged in run order.
    pub firing: Firing,
}

/// What an agent event asks the shell to do ([`super::HookSession::on_event`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventOutcome {
    /// Nothing.
    Nothing,
    /// Run these `toolEnd` commands, then report the result with
    /// [`super::HookSession::on_tool_output`] (always, even when every command failed).
    ToolCommands(Firing),
    /// The run settled and earned a follow-up.
    FollowUp(FollowUpPlan),
    /// A user run settled without earning a follow-up (no successful tool call, aborted, failed,
    /// or the user already sent the next message): run its `turnEnd` commands and discard their
    /// output.
    TurnCommands(Firing),
    /// The chat ended: run these `sessionEnd` commands (their output is discarded).
    SessionEnd(Firing),
}

/// The part `entry` contributes to a firing of `event`: its text, or its command to run in `chat`
/// with `env` (tool name, tool error, touched paths).
pub(crate) fn planned_part(entry: &SetHook, chat: &ChatContext, event: HookEvent, tool: ToolEnv<'_>) -> PlannedPart {
    let hook = &entry.hook;
    let order = entry.order.clone();

    match &hook.action {
        HookAction::AddContext { text } | HookAction::FollowUp { text } => PlannedPart::Text(ResolvedPart {
            hook_id: hook.id.clone(),
            hook_name: hook.name.clone(),
            text: text.clone(),
            order,
        }),
        HookAction::Command { command, timeout_secs } => {
            let cwd = match (&hook.scope, &hook.project) {
                (HookScope::Project, Some(project)) => PathBuf::from(project),
                _ => chat.folder.clone(),
            };

            PlannedPart::Command(CommandRequest {
                hook_id: hook.id.clone(),
                hook_name: hook.name.clone(),
                event,
                command: command.clone(),
                timeout: Duration::from_secs(u64::from(timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS))),
                cwd,
                env: command_env(chat, event, &tool),
                order,
            })
        }
    }
}

/// The tool facts a command sees.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ToolEnv<'a> {
    /// `toolEnd`: the tool name and whether it failed.
    pub tool: Option<(&'a str, bool)>,
    /// The paths the tool call or the run changed.
    pub touched: &'a [String],
}

fn command_env(chat: &ChatContext, event: HookEvent, tool: &ToolEnv<'_>) -> Vec<(String, String)> {
    let (name, error) = match tool.tool {
        Some((name, is_error)) => (tool_value(name), if is_error { "1" } else { "0" }.to_owned()),
        None => (String::new(), String::new()),
    };

    vec![
        ("HARNESS_HOOK_EVENT".to_owned(), event.as_str().to_owned()),
        (
            "HARNESS_HOOK_PROJECT".to_owned(),
            project_value(chat.project.as_deref()),
        ),
        ("HARNESS_HOOK_SESSION".to_owned(), chat.session_id.clone()),
        ("HARNESS_HOOK_TOOL".to_owned(), name),
        ("HARNESS_HOOK_TOOL_ERROR".to_owned(), error),
        ("HARNESS_HOOK_TOUCHED".to_owned(), touched_value(tool.touched)),
    ]
}

/// Sorts `parts` into run order.
pub(crate) fn sort_parts(parts: &mut [PlannedPart]) {
    parts.sort_by(|left, right| left.order().cmp(right.order()));
}
