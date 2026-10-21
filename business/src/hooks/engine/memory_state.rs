//! Where a chat's memory guards stand, per kind ([`MemoryGuardState`]), worded for the agent: the
//! short notice a user message carries when the state changed since the agent was last told
//! ([`HookSession::set_memory_guards`]), and the sentence put in front of "Save what you learned"
//! while only one kind may be saved.

use std::path::Path;

use crate::hooks::compose::ResolvedPart;
use crate::model::{GlobalKindGuard, KindGuard, MemoryChatGuards, MemoryGuards};

use super::{HookSession, MemorySaveGate, SendPlan};

/// The id of the memory settings part a user message carries ([`SendPlan::memory_notice`]). Not a
/// hook id, so it never matches a hook.
pub const MEMORY_NOTICE_ID: &str = "harness-desktop:memory-settings";

/// The name the memory settings part is shown under in the transcript.
pub const MEMORY_NOTICE_NAME: &str = "Memory settings";

/// Where saving one kind (notes or scripts) stands for a chat. The first reason that applies wins,
/// in the order Harness Memory checks them: everywhere, the project, the chat.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SaveRule {
    /// Agents save it.
    #[default]
    Saved,
    /// Saves wait for the user's review ("Ask before saving").
    Ask,
    /// Paused for every chat (Settings).
    PausedEverywhere,
    /// Off for the chat's project.
    ProjectOff,
    /// The chat is paused.
    ChatPaused,
    /// Off for this chat.
    ChatOff,
    /// The guards could not be read: Harness Memory refuses every save.
    Unreadable,
}

impl SaveRule {
    /// True when agents may save it (at once or after the user's review).
    pub fn allows(self) -> bool {
        matches!(self, Self::Saved | Self::Ask)
    }

    /// The rule of one kind from the guards of every chat (`global`), the chat's own (`paused`,
    /// `kind`) and the chat's project root (`repo`).
    fn of(global: &GlobalKindGuard, paused: bool, kind: KindGuard, repo: Option<&Path>) -> Self {
        let project_off = repo.is_some_and(|repo| global.off_repos.iter().any(|off| Path::new(off) == repo));

        if global.paused {
            return Self::PausedEverywhere;
        }

        if project_off {
            return Self::ProjectOff;
        }

        if paused {
            return Self::ChatPaused;
        }

        if !kind.on {
            return Self::ChatOff;
        }

        if kind.ask { Self::Ask } else { Self::Saved }
    }

    /// Why saving is off, after "saving notes" (`None` while it is allowed).
    fn off_reason(self) -> Option<&'static str> {
        match self {
            Self::Saved | Self::Ask => None,
            Self::PausedEverywhere => Some("is paused everywhere"),
            Self::ProjectOff => Some("is off for this project"),
            Self::ChatPaused => Some("is paused for this chat"),
            Self::ChatOff => Some("is off for this chat"),
            Self::Unreadable => Some("is off, the memory settings cannot be read"),
        }
    }

    /// One sentence on saving `subject` ("notes", "scripts" or both), in lower case.
    fn sentence(self, subject: &str) -> String {
        match (self, self.off_reason()) {
            (_, Some(reason)) => format!("saving {subject} {reason}."),
            (Self::Ask, None) => format!("{subject} you save wait for the user's review."),
            _ => format!("{subject} are saved."),
        }
    }
}

/// Where saving notes and scripts stands for one chat.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemoryGuardState {
    /// Notes (`note_*`, `entity_*`).
    pub notes: SaveRule,
    /// Scripts (`script_save`, `script_delete`).
    pub scripts: SaveRule,
}

impl MemoryGuardState {
    /// Both saved.
    pub const OPEN: Self = Self {
        notes: SaveRule::Saved,
        scripts: SaveRule::Saved,
    };

    /// The guards could not be read: nothing is saved.
    pub const CLOSED: Self = Self {
        notes: SaveRule::Unreadable,
        scripts: SaveRule::Unreadable,
    };

    /// The state of a chat whose guards are `chat` (default: no chat rules), with the guards of
    /// every chat `global`, in the project root `repo` (none outside one).
    pub fn of(global: &MemoryGuards, chat: &MemoryChatGuards, repo: Option<&Path>) -> Self {
        Self {
            notes: SaveRule::of(&global.notes, chat.paused, chat.notes, repo),
            scripts: SaveRule::of(&global.scripts, chat.paused, chat.scripts, repo),
        }
    }

    /// What it means for the memory-save hook.
    pub fn gate(self) -> MemorySaveGate {
        let paused = [self.notes, self.scripts]
            .iter()
            .any(|rule| matches!(rule, SaveRule::ChatPaused | SaveRule::Unreadable));

        MemorySaveGate {
            paused,
            notes_off: !self.notes.allows(),
            scripts_off: !self.scripts.allows(),
        }
    }

    /// What the agent is told about it, starting `Harness Desktop:`.
    pub fn notice(self) -> String {
        let said = if self == Self::OPEN {
            "saving notes and scripts is on again for this chat.".to_owned()
        } else if self.notes == self.scripts {
            self.notes.sentence("notes and scripts")
        } else {
            let scripts = capitalised(&self.scripts.sentence("scripts"));
            format!("{} {scripts}", self.notes.sentence("notes"))
        };

        format!("Harness Desktop: {said}")
    }

    /// The sentence put in front of the memory-save hook's text while only one kind may be saved,
    /// `None` otherwise (both, or neither: the hook is skipped then).
    pub fn save_prefix(self) -> Option<String> {
        match (self.notes.off_reason(), self.scripts.off_reason()) {
            (Some(reason), None) => Some(format!(
                "Saving notes {reason}: only consider scripts (script_save), save no notes."
            )),
            (None, Some(reason)) => Some(format!(
                "Saving scripts {reason}: only consider notes (note_save, note_update), do not call script_save."
            )),
            _ => None,
        }
    }

    /// `text` (the memory-save hook's) with [`MemoryGuardState::save_prefix`] in front of it.
    pub fn tailor(self, text: &str) -> String {
        match self.save_prefix() {
            Some(prefix) => format!("{prefix}\n\n{text}"),
            None => text.to_owned(),
        }
    }
}

fn capitalised(text: &str) -> String {
    let mut characters = text.chars();

    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => String::new(),
    }
}

/// A memory settings notice a [`SendPlan`] carries: the state it tells, and the one told before.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Announcement {
    pub before: MemoryGuardState,
    pub now: MemoryGuardState,
}

impl HookSession {
    /// Where the chat's memory guards stand now: a run that settles from now on tailors or skips
    /// the memory-save hook by it, and the next eligible user prompt tells the agent when it
    /// differs from what the agent was told last (at first: both saved, so a chat where everything
    /// may be saved hears nothing).
    pub fn set_memory_guards(&mut self, state: MemoryGuardState) {
        self.memory = state;
    }

    /// Adds the memory settings notice to `plan` when the state changed since the agent was told.
    pub(super) fn announce_memory(&self, plan: &mut SendPlan) {
        if self.memory == self.announced {
            return;
        }

        plan.memory_notice = Some(ResolvedPart::new(
            MEMORY_NOTICE_ID,
            MEMORY_NOTICE_NAME,
            self.memory.notice(),
        ));
        plan.announcement = Some(Announcement {
            before: self.announced,
            now: self.memory,
        });
    }

    /// The message of `plan` carries its notice (`delivered`): the agent knows the state now.
    pub(super) fn memory_announced(&mut self, plan: &SendPlan, delivered: &[String]) {
        let carried = delivered.iter().any(|id| id == MEMORY_NOTICE_ID);

        if let (Some(announcement), true) = (plan.announcement, carried) {
            self.announced = announcement.now;
        }
    }

    /// The message of `plan` was not delivered: its notice is told again next time.
    pub(super) fn memory_not_announced(&mut self, plan: &SendPlan) {
        if let Some(announcement) = plan.announcement
            && self.announced == announcement.now
        {
            self.announced = announcement.before;
        }
    }

    /// Another session: its agent was told nothing yet.
    pub(super) fn forget_announced(&mut self) {
        self.announced = MemoryGuardState::OPEN;
    }
}
