//! What the engine hands the shell: the hooks that apply to a chat ([`HookSet`]), and the
//! firings to carry out ([`Firing`]: fixed texts and commands to run, in run order).

use std::collections::VecDeque;
use std::ops::RangeInclusive;
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

/// Characters Windows may turn into one of [`SHELL_SPECIALS`], `/` or `\` when it converts the
/// environment to a legacy code page ("best fit"), so a path that looked plain would not be once
/// converted. Taken from Microsoft's best-fit tables (`WindowsBestFit/bestfit1252.txt`,
/// `bestfit932.txt`, `bestfit936.txt` and the other `bestfit*.txt` on unicode.org, as far as they
/// could be checked) and widened with look-alikes of the same characters whose mapping could not
/// be confirmed, since refusing a rare character only costs one path. Not exhaustive: another code
/// page may map a character missing here, so the variables stay untrusted input (see the format
/// doc). Windows does not run a round trip through `WideCharToMultiByte` instead: the workspace
/// forbids `unsafe` code, and every binding of it is `unsafe`.
const BEST_FIT_SPECIALS: &[RangeInclusive<char>] = &[
    // `"`
    '\u{02BA}'..='\u{02BA}',
    '\u{02DD}'..='\u{02DD}',
    '\u{02EE}'..='\u{02EE}',
    '\u{030B}'..='\u{030B}',
    '\u{030E}'..='\u{030E}',
    '\u{05F4}'..='\u{05F4}',
    '\u{201F}'..='\u{201F}',
    '\u{2033}'..='\u{2033}',
    '\u{2036}'..='\u{2036}',
    '\u{3003}'..='\u{3003}',
    '\u{301D}'..='\u{301F}',
    // `|`
    '\u{00A6}'..='\u{00A6}',
    '\u{01C0}'..='\u{01C0}',
    '\u{05C0}'..='\u{05C0}',
    '\u{2016}'..='\u{2016}',
    '\u{2223}'..='\u{2223}',
    '\u{2225}'..='\u{2225}',
    '\u{23D0}'..='\u{23D0}',
    '\u{2502}'..='\u{2502}',
    '\u{2758}'..='\u{2758}',
    // `<` and `>`
    '\u{00AB}'..='\u{00AB}',
    '\u{00BB}'..='\u{00BB}',
    '\u{02C2}'..='\u{02C3}',
    '\u{2039}'..='\u{203A}',
    '\u{2190}'..='\u{2190}',
    '\u{2192}'..='\u{2192}',
    '\u{2329}'..='\u{232A}',
    '\u{276E}'..='\u{276F}',
    '\u{27E8}'..='\u{27E9}',
    '\u{3008}'..='\u{3009}',
    // `^`
    '\u{02C4}'..='\u{02C4}',
    '\u{02C6}'..='\u{02C6}',
    '\u{0302}'..='\u{0302}',
    '\u{2038}'..='\u{2038}',
    '\u{2191}'..='\u{2191}',
    '\u{2303}'..='\u{2303}',
    // `%` and `!`
    '\u{066A}'..='\u{066A}',
    '\u{2052}'..='\u{2052}',
    '\u{00A1}'..='\u{00A1}',
    '\u{01C3}'..='\u{01C3}',
    '\u{203C}'..='\u{203C}',
    // `(` and `)`
    '\u{207D}'..='\u{207E}',
    '\u{208D}'..='\u{208E}',
    '\u{2768}'..='\u{276B}',
    '\u{2E28}'..='\u{2E29}',
    // `/` and `\` (the yen and won signs are `\` in the Japanese and Korean code pages)
    '\u{00A5}'..='\u{00A5}',
    '\u{20A9}'..='\u{20A9}',
    '\u{2044}'..='\u{2044}',
    '\u{2215}'..='\u{2216}',
    '\u{2571}'..='\u{2572}',
    '\u{27CB}'..='\u{27CB}',
    '\u{27CD}'..='\u{27CD}',
    '\u{29F5}'..='\u{29F5}',
    '\u{29F8}'..='\u{29F9}',
    // Whole blocks of ASCII look-alikes: the vertical forms, the CJK compatibility forms with the
    // small form variants (small `& % ! < > ( ) \`), the fullwidth and halfwidth forms.
    '\u{FE10}'..='\u{FE1F}',
    '\u{FE30}'..='\u{FE6F}',
    '\u{FF00}'..='\u{FFEF}',
];

/// Characters best fit may turn into `-` (see [`BEST_FIT_SPECIALS`]): hyphens, dashes, minus
/// signs and the soft hyphen. Refused at the start of a path only, where `-` reads as an option.
const BEST_FIT_DASHES: &[RangeInclusive<char>] = &[
    '\u{00AD}'..='\u{00AD}',
    '\u{02D7}'..='\u{02D7}',
    '\u{2010}'..='\u{2015}',
    '\u{2043}'..='\u{2043}',
    '\u{207B}'..='\u{207B}',
    '\u{208B}'..='\u{208B}',
    '\u{2212}'..='\u{2212}',
    '\u{2500}'..='\u{2500}',
    '\u{FE58}'..='\u{FE58}',
    '\u{FE63}'..='\u{FE63}',
    '\u{FF0D}'..='\u{FF0D}',
];

fn is_in(table: &[RangeInclusive<char>], character: char) -> bool {
    table.iter().any(|range| range.contains(&character))
}

/// True when `path` begins with `-` or a character best fit may turn into one.
fn starts_like_option(path: &str) -> bool {
    path.chars()
        .next()
        .is_some_and(|first| first == '-' || is_in(BEST_FIT_DASHES, first))
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
/// turn into one, see [`BEST_FIT_SPECIALS`]) and no leading `-` (it would read as an option), nor
/// one best fit could turn into `-` ([`BEST_FIT_DASHES`]).
fn is_plain_path(path: &str) -> bool {
    let special = |character: char| {
        character.is_control() || SHELL_SPECIALS.contains(&character) || is_in(BEST_FIT_SPECIALS, character)
    };

    !path.is_empty() && !starts_like_option(path) && !path.chars().any(special)
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
/// ([`is_plain_path`]) is left out: the agent reports these paths, so they are untrusted. So is a
/// path longer than the limit on its own, and the paths after it still count.
fn touched_value(paths: &[String]) -> String {
    let mut value = String::new();
    let fits_alone = |path: &&String| path.len() <= MAX_TOUCHED_BYTES;

    for path in paths.iter().filter(fits_alone).filter(|path| is_plain_path(path)) {
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
