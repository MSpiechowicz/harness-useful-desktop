//! Whether the Harness family hook "Save what you learned" (`HookSeed::MemorySave`) may ask the
//! agent to save: not while the chat is paused, nor while notes and scripts are both off for it.
//! While only one kind may be saved, its text starts with a sentence that says so
//! ([`MemoryGuardState::tailor`]). The shell reads the guards (`crate::hooks::read_memory_guards`)
//! and hands the result to [`HookSession::set_memory_guards`](super::HookSession::set_memory_guards).
//! "Check again" sends that hook's text at once ([`memory_save_part`]).

use crate::hooks::compose::ResolvedPart;
use crate::hooks::family::family_hook;
use crate::model::{HookAction, HookSeed};

use super::HookSet;
use super::memory_state::MemoryGuardState;

/// What the memory guards allow a chat to save, as far as the memory-save hook is concerned.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemorySaveGate {
    /// The chat is paused.
    pub paused: bool,
    /// Notes are off for the chat, its repository or everywhere.
    pub notes_off: bool,
    /// Scripts are off for the chat, its repository or everywhere.
    pub scripts_off: bool,
}

impl MemorySaveGate {
    /// Nothing is off.
    pub const OPEN: Self = Self {
        paused: false,
        notes_off: false,
        scripts_off: false,
    };

    /// Every save is refused (the guards could not be read).
    pub const CLOSED: Self = Self {
        paused: true,
        notes_off: true,
        scripts_off: true,
    };

    /// True when the memory-save hook must not fire: the chat is paused, or neither notes nor
    /// scripts may be saved.
    pub fn skips_memory_save(self) -> bool {
        self.paused || (self.notes_off && self.scripts_off)
    }
}

/// `hooks` as the memory-save hook may fire under `guards`: without it when they skip it, with its
/// text tailored while only one kind may be saved, else `None` (use `hooks` as they are).
pub(super) fn gated(hooks: &HookSet, guards: MemoryGuardState) -> Option<HookSet> {
    if guards.gate().skips_memory_save() {
        let kept = hooks
            .hooks
            .iter()
            .filter(|entry| entry.hook.seed != Some(HookSeed::MemorySave))
            .cloned()
            .collect();

        return Some(HookSet { hooks: kept });
    }

    guards.save_prefix()?;

    let mut tailored = hooks.clone();

    for entry in &mut tailored.hooks {
        let hook = &mut entry.hook;

        if let (Some(HookSeed::MemorySave), HookAction::FollowUp { text }) = (hook.seed, &mut hook.action) {
            *text = guards.tailor(text);
        }
    }

    Some(tailored)
}

/// What "Check again" sends for a chat whose hooks are `hooks`, under `guards`: the memory-save
/// hook as the user keeps it (the first active follow-up of that seed, in run order, with its
/// edited name and text), else the catalog's version while there is none (removed, off, or a
/// command now), its text tailored as the hook's follow-up is ([`MemoryGuardState::tailor`]).
pub fn memory_save_part(hooks: &HookSet, guards: MemoryGuardState) -> ResolvedPart {
    let configured = hooks.hooks.iter().find_map(|entry| {
        let hook = &entry.hook;

        match &hook.action {
            HookAction::FollowUp { text } if hook.seed == Some(HookSeed::MemorySave) => Some(ResolvedPart::new(
                hook.id.as_str(),
                hook.name.as_str(),
                guards.tailor(text),
            )),
            _ => None,
        }
    });

    configured.unwrap_or_else(|| {
        let family = family_hook(HookSeed::MemorySave);

        ResolvedPart::new(HookSeed::MemorySave.as_str(), family.name, guards.tailor(family.text))
    })
}
