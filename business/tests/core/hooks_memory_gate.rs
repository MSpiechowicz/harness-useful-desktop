//! The Harness family hook "Save what you learned" stays quiet while the chat's memory guards say
//! nothing may be saved: the chat paused, or notes and scripts both off. Other hooks still fire.
//! While only one kind may be saved, its text starts with a sentence that says so. "Check again"
//! sends that hook's text as the user keeps it, tailored the same way.

use crate::hook_support;

mod hooks_memory_gate {
    use harness_business::hooks::{
        EventOutcome, HookSet, MemoryGuardState, SEED_MEMORY_SAVE_TEXT, SaveRule, compose_follow_up, memory_save_part,
    };
    use harness_business::model::{HookAction, HookEvent, HookSeed};
    use pretty_assertions::assert_eq;

    use super::hook_support::{chat, command, follow_up, follow_up_plan, set, texts, user_run};

    /// A plain `turnEnd` follow-up (1) and the memory-save family hook (2).
    fn hooks() -> harness_business::hooks::HookSet {
        let mut memory_save = follow_up(2, HookEvent::TurnEnd);
        memory_save.seed = Some(HookSeed::MemorySave);

        set(vec![follow_up(1, HookEvent::TurnEnd), memory_save])
    }

    fn guards(notes: SaveRule, scripts: SaveRule) -> MemoryGuardState {
        MemoryGuardState { notes, scripts }
    }

    /// The memory-save follow-up a user turn fires, split into the tailored prefix and the user's own text after it.
    fn tailored_follow_up(session: &mut harness_business::hooks::HookSession) -> (String, String) {
        let plan = follow_up_plan(user_run(session, &hooks()));
        let text = texts(&plan.firing)[1].clone();
        let (prefix, kept) = text.split_once("\n\n").unwrap();

        (prefix.to_owned(), kept.to_owned())
    }

    #[test]
    fn the_memory_save_hook_fires_while_something_may_be_saved() {
        for open in [
            MemoryGuardState::OPEN,
            guards(SaveRule::Ask, SaveRule::Ask),
            guards(SaveRule::ChatOff, SaveRule::Saved),
            guards(SaveRule::Saved, SaveRule::ProjectOff),
            guards(SaveRule::PausedEverywhere, SaveRule::Ask),
        ] {
            let mut session = chat(false, false);
            session.set_memory_guards(open);

            let plan = follow_up_plan(user_run(&mut session, &hooks()));
            let texts = texts(&plan.firing);

            assert_eq!(texts.len(), 2, "{open:?}");
            assert_eq!(texts[0], "follow-up 1", "other hooks stay as they are");
            assert!(texts[1].ends_with("follow-up 2"), "{open:?}: {}", texts[1]);
        }
    }

    #[test]
    fn the_memory_save_hook_is_unchanged_while_both_kinds_may_be_saved() {
        for open in [MemoryGuardState::OPEN, guards(SaveRule::Ask, SaveRule::Saved)] {
            let mut session = chat(false, false);
            session.set_memory_guards(open);

            let plan = follow_up_plan(user_run(&mut session, &hooks()));
            assert_eq!(texts(&plan.firing), vec!["follow-up 1", "follow-up 2"], "{open:?}");
        }
    }

    #[test]
    fn the_memory_save_hook_asks_only_about_scripts_while_notes_are_off() {
        for rule in [SaveRule::ChatOff, SaveRule::ProjectOff, SaveRule::PausedEverywhere] {
            let mut session = chat(false, false);
            session.set_memory_guards(guards(rule, SaveRule::Saved));

            let (prefix, kept) = tailored_follow_up(&mut session);

            assert!(prefix.contains("script_save"), "{prefix}");
            assert!(!prefix.contains("note_save"), "{prefix}");
            assert_eq!(kept, "follow-up 2", "the user's text follows unchanged");
        }
    }

    #[test]
    fn the_memory_save_hook_asks_only_about_notes_while_scripts_are_off() {
        let mut session = chat(false, false);
        session.set_memory_guards(guards(SaveRule::Ask, SaveRule::ChatOff));

        let (prefix, kept) = tailored_follow_up(&mut session);

        assert!(prefix.contains("note_save"), "{prefix}");
        assert!(prefix.contains("off for this chat"), "{prefix}");
        assert_eq!(kept, "follow-up 2", "the user's text follows unchanged");
    }

    #[test]
    fn the_memory_save_hook_is_skipped_in_each_gated_state() {
        for closed in [
            guards(SaveRule::ChatPaused, SaveRule::ChatPaused),
            guards(SaveRule::PausedEverywhere, SaveRule::ChatPaused),
            guards(SaveRule::ChatOff, SaveRule::ChatOff),
            guards(SaveRule::ProjectOff, SaveRule::PausedEverywhere),
            MemoryGuardState::CLOSED,
        ] {
            let mut session = chat(false, false);
            session.set_memory_guards(closed);

            let plan = follow_up_plan(user_run(&mut session, &hooks()));
            assert_eq!(texts(&plan.firing), vec!["follow-up 1"], "{closed:?}");
        }
    }

    #[test]
    fn a_run_with_only_the_memory_save_hook_sends_nothing_while_gated() {
        let mut memory_save = follow_up(2, HookEvent::TurnEnd);
        memory_save.seed = Some(HookSeed::MemorySave);
        let only = set(vec![memory_save]);

        let mut session = chat(false, false);
        session.set_memory_guards(guards(SaveRule::ChatPaused, SaveRule::ChatPaused));
        assert_eq!(user_run(&mut session, &only), EventOutcome::Nothing);

        session.set_memory_guards(MemoryGuardState::OPEN);
        let plan = follow_up_plan(user_run(&mut session, &only));
        assert_eq!(
            texts(&plan.firing),
            vec!["follow-up 2"],
            "the gate applies to the run that settles"
        );
    }

    #[test]
    fn check_again_sends_the_memory_save_hook_as_the_user_edited_it() {
        let mut edited = follow_up(2, HookEvent::TurnEnd);
        edited.seed = Some(HookSeed::MemorySave);
        edited.name = "Keep what matters".to_owned();
        edited.action = HookAction::FollowUp {
            text: "Save only the fixes.".to_owned(),
        };

        let part = memory_save_part(
            &set(vec![follow_up(1, HookEvent::TurnEnd), edited]),
            MemoryGuardState::OPEN,
        );

        assert_eq!(
            compose_follow_up(&[part]).as_deref(),
            Some("<harness-hook-followup name=\"Keep what matters\">Save only the fixes.</harness-hook-followup>")
        );
    }

    #[test]
    fn check_again_falls_back_to_the_seed_text_without_that_hook() {
        let mut turned_command = command(2, HookEvent::TurnEnd);
        turned_command.seed = Some(HookSeed::MemorySave);

        for hooks in [
            HookSet::empty(),
            set(vec![follow_up(1, HookEvent::TurnEnd), turned_command]),
        ] {
            let message = compose_follow_up(&[memory_save_part(&hooks, MemoryGuardState::OPEN)]).unwrap();

            assert!(message.contains("Save what you learned"), "{message}");
            assert!(
                message.contains(SEED_MEMORY_SAVE_TEXT.split('"').next().unwrap()),
                "{message}"
            );
        }
    }

    #[test]
    fn check_again_is_tailored_as_the_follow_up_is() {
        let hooks = hooks();

        for state in [
            guards(SaveRule::ChatOff, SaveRule::Saved),
            guards(SaveRule::Saved, SaveRule::PausedEverywhere),
            guards(SaveRule::Ask, SaveRule::Saved),
        ] {
            let mut session = chat(false, false);
            session.set_memory_guards(state);

            let plan = follow_up_plan(user_run(&mut session, &hooks));
            let follow_up = texts(&plan.firing)[1].clone();

            assert_eq!(memory_save_part(&hooks, state).text, follow_up, "{state:?}");
        }

        let fallback = memory_save_part(&HookSet::empty(), guards(SaveRule::Saved, SaveRule::ChatOff)).text;
        assert!(fallback.ends_with(SEED_MEMORY_SAVE_TEXT), "{fallback}");
        assert!(
            fallback.starts_with("Saving scripts is off for this chat"),
            "{fallback}"
        );
    }
}
