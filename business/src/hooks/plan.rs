//! What the engine hands the shell: the hooks that apply to a chat ([`HookSet`]), and the
//! firings to carry out ([`Firing`]: fixed texts and commands to run, in run order).

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Duration;

use super::compose::ResolvedPart;
use super::format::DEFAULT_TIMEOUT_SECS;
use super::view::order_key;
use crate::model::{Hook, HookAction, HookEvent, HookScope, HookStatus, HookView};

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

/// The hooks that run in one chat: active (valid, enabled, approved, requirements met), global
/// or of the chat's project, in run order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HookSet {
    hooks: Vec<Hook>,
}

impl HookSet {
    /// No hooks.
    pub fn empty() -> Self {
        Self::default()
    }

    /// The active hooks of `views` that apply to a chat in `project` (`None`: global hooks only).
    pub fn for_chat(views: &[HookView], project: Option<&str>) -> Self {
        let mut hooks: Vec<Hook> = views
            .iter()
            .filter(|view| view.status == HookStatus::Active)
            .filter_map(|view| view.hook.clone())
            .filter(|hook| match hook.scope {
                HookScope::Global => true,
                HookScope::Project => project.is_some() && hook.project.as_deref() == project,
            })
            .collect();

        hooks.sort_by_key(order_key);
        Self { hooks }
    }

    /// True when no hook applies.
    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty()
    }

    /// The hooks of `event`, in run order.
    pub(crate) fn of_event(&self, event: HookEvent) -> impl Iterator<Item = &Hook> {
        self.hooks.iter().filter(move |hook| hook.event == event)
    }
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
    pub(crate) order: (u8, [u32; 7], String),
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

    fn order(&self) -> &(u8, [u32; 7], String) {
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
/// (`sessionStart`, then `promptSubmit`), put [`super::compose_prompt`] of all their parts in front
/// of the message and report what it kept ([`super::HookSession::context_composed`]). Right
/// before the message is delivered, pass it to [`super::HookSession::sending`], and to
/// [`super::HookSession::send_failed`] when the message could not be delivered.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SendPlan {
    /// Firings whose parts become context, possibly empty.
    pub firings: Vec<Firing>,
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
        self.firings.iter().all(Firing::is_empty)
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

/// The part `hook` contributes to a firing of `event`: its text, or its command to run in `chat`
/// with `env` (tool name, tool error, touched paths).
pub(crate) fn planned_part(hook: &Hook, chat: &ChatContext, event: HookEvent, tool: ToolEnv<'_>) -> PlannedPart {
    let order = order_key(hook);

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

/// Longest `HARNESS_HOOK_TOOL` value, in characters (the matcher's limit).
const MAX_TOOL_CHARS: usize = 200;

/// Characters `cmd.exe` treats specially, never in `HARNESS_HOOK_TOUCHED` or
/// `HARNESS_HOOK_PROJECT` on any platform.
const SHELL_SPECIALS: &[char] = &['&', '|', '<', '>', '^', '%', '!', '"', '(', ')'];

/// True for a character Windows may map to an ASCII one when it converts the environment to a
/// legacy code page ("best fit"): the fullwidth and halfwidth forms (U+FF00 to U+FFEF, among them
/// fullwidth `"` and `&`), the yen and won signs (both may become `\`). Left out like
/// [`SHELL_SPECIALS`].
fn is_best_fit(character: char) -> bool {
    matches!(character, '\u{FF00}'..='\u{FFEF}' | '\u{00A5}' | '\u{20A9}')
}

/// The tool name an agent reported, when it is 1 to [`MAX_TOOL_CHARS`] of `[A-Za-z0-9_.:-]`;
/// empty otherwise. The agent names its tools, so the name is untrusted input to a shell.
fn tool_value(name: &str) -> String {
    let allowed = |character: char| character.is_ascii_alphanumeric() || "_.:-".contains(character);
    let valid = !name.is_empty() && name.chars().count() <= MAX_TOOL_CHARS && name.chars().all(allowed);

    if valid { name.to_owned() } else { String::new() }
}

/// True for a path `HARNESS_HOOK_TOUCHED` or `HARNESS_HOOK_PROJECT` may carry: no control
/// character (a newline would split it), no `cmd.exe` special character (nor one "best fit" could
/// turn into one, see [`is_best_fit`]) and no leading `-` (it would read as an option).
fn is_plain_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('-')
        && !path
            .chars()
            .any(|character| character.is_control() || SHELL_SPECIALS.contains(&character) || is_best_fit(character))
}

/// The chat's project root when it is a plain path ([`is_plain_path`]), empty otherwise: the root
/// is a folder name the agent may have chosen, so it is untrusted input to a shell too.
fn project_value(project: Option<&str>) -> String {
    match project {
        Some(project) if is_plain_path(project) => project.to_owned(),
        _ => String::new(),
    }
}

/// `paths` joined by newlines, within [`MAX_TOUCHED_BYTES`]. A path that is not plain
/// ([`is_plain_path`]) is left out: the agent reports these paths, so they are untrusted.
fn touched_value(paths: &[String]) -> String {
    let mut value = String::new();

    for path in paths.iter().filter(|path| is_plain_path(path)) {
        let extra = path.len() + usize::from(!value.is_empty());

        if value.len() + extra > MAX_TOUCHED_BYTES {
            break;
        }

        if !value.is_empty() {
            value.push('\n');
        }

        value.push_str(path);
    }

    value
}

/// Sorts `parts` into run order.
pub(crate) fn sort_parts(parts: &mut [PlannedPart]) {
    parts.sort_by(|left, right| left.order().cmp(right.order()));
}
