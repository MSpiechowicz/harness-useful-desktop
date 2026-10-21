//! A user message tells the agent when the chat's memory guards changed since it was told last:
//! one short "Memory settings" part in the message's hook context, once per change. A chat where
//! everything may be saved hears nothing at first. Hook follow-ups and utility agents carry none.
//! The guards are read as Harness Memory reads them (a file that cannot be read refuses all).

mod hook_support;

mod hooks_memory_notice {
    use std::fs;

    use harness_business::ecosystem::{update_global, write_chat};
    use harness_business::hooks::{
        HookSession, HookSet, MEMORY_NOTICE_NAME, MemoryGuardState, PlannedPart, SaveRule, SendOrigin, SendPlan,
        compose_prompt, parse_hook_context, read_memory_guards,
    };
    use harness_business::model::{
        AgentAction, HookEvent, KindGuard, MemoryChatGuards, MemoryGuardKind, MemoryGuardsPatch,
    };
    use pretty_assertions::assert_eq;

    use super::hook_support::{chat, context, prompt, set};

    const KEY: &str = "3b2f6c1e-8a4d-4e5f-9a0b-1c2d3e4f5a6b";

    fn guards(notes: SaveRule, scripts: SaveRule) -> MemoryGuardState {
        MemoryGuardState { notes, scripts }
    }

    /// A user prompt as the shell sends it; the message as delivered and its plan.
    fn send(session: &mut HookSession, hooks: &HookSet, action: &AgentAction) -> (String, SendPlan) {
        let mut plan = session.on_send(hooks, action, SendOrigin::User);

        let message = match action {
            AgentAction::Prompt { message, .. } => message.clone(),
            _ => String::new(),
        };

        // The firings' texts, as the shell resolves them (no command hooks here).
        let resolved = plan
            .firings
            .iter()
            .flat_map(|firing| &firing.parts)
            .filter_map(|part| match part {
                PlannedPart::Text(text) => Some(text.clone()),
                PlannedPart::Command(_) => None,
            })
            .collect();

        let parts = plan.context_parts(resolved);
        let (message, delivered) = compose_prompt(&message, &parts);
        session.context_composed(&plan, &parts, &delivered);
        session.sending(&mut plan);

        (message, plan)
    }

    /// The memory settings part `message` carries, when it carries one.
    fn notice(message: &str) -> Option<String> {
        let (items, _) = parse_hook_context(message)?;

        items
            .into_iter()
            .find(|item| item.name == MEMORY_NOTICE_NAME)
            .map(|item| item.text)
    }

    fn told(session: &mut HookSession) -> Option<String> {
        notice(&send(session, &HookSet::empty(), &prompt("Go on")).0)
    }

    #[test]
    fn a_first_message_where_everything_may_be_saved_carries_nothing() {
        let mut session = chat(true, false);
        session.set_memory_guards(MemoryGuardState::OPEN);

        let (message, plan) = send(&mut session, &HookSet::empty(), &prompt("Hello"));

        assert_eq!(message, "Hello");
        assert!(plan.is_empty());
    }

    #[test]
    fn a_first_message_tells_what_is_restricted() {
        let mut session = chat(true, false);
        session.set_memory_guards(guards(SaveRule::ChatOff, SaveRule::Saved));

        let text = told(&mut session).expect("the first message tells the guards");

        assert_eq!(
            text,
            "Harness Desktop: saving notes is off for this chat. Scripts are saved."
        );
        assert_eq!(told(&mut session), None, "said once");
    }

    #[test]
    fn a_change_is_told_once_with_the_other_hooks_context() {
        let hooks = set(vec![context(1, HookEvent::PromptSubmit)]);
        let mut session = chat(false, false);
        assert_eq!(told(&mut session), None);

        session.set_memory_guards(guards(SaveRule::ProjectOff, SaveRule::ProjectOff));
        let (message, _) = send(&mut session, &hooks, &prompt("Next"));
        let (items, text) = parse_hook_context(&message).unwrap();

        assert_eq!(text, "Next");
        assert_eq!(
            items.iter().map(|item| item.name.as_str()).collect::<Vec<_>>(),
            vec![MEMORY_NOTICE_NAME, "hook 1"],
            "the notice comes first"
        );
        assert_eq!(
            items[0].text,
            "Harness Desktop: saving notes and scripts is off for this project."
        );

        let (message, _) = send(&mut session, &hooks, &prompt("Again"));
        assert_eq!(notice(&message), None, "not repeated");

        session.set_memory_guards(MemoryGuardState::OPEN);
        assert_eq!(
            told(&mut session).as_deref(),
            Some("Harness Desktop: saving notes and scripts is on again for this chat.")
        );
        assert_eq!(told(&mut session), None);
    }

    #[test]
    fn each_state_is_worded_for_the_agent() {
        let cases = [
            (
                guards(SaveRule::ChatPaused, SaveRule::ChatPaused),
                "Harness Desktop: saving notes and scripts is paused for this chat.",
            ),
            (
                guards(SaveRule::Saved, SaveRule::PausedEverywhere),
                "Harness Desktop: notes are saved. Saving scripts is paused everywhere.",
            ),
            (
                guards(SaveRule::Ask, SaveRule::Saved),
                "Harness Desktop: notes you save wait for the user's review. Scripts are saved.",
            ),
            (
                MemoryGuardState::CLOSED,
                "Harness Desktop: saving notes and scripts is off, the memory settings cannot be read.",
            ),
        ];

        for (state, expected) in cases {
            let mut session = chat(false, false);
            session.set_memory_guards(state);

            assert_eq!(told(&mut session).as_deref(), Some(expected), "{state:?}");
        }
    }

    #[test]
    fn a_change_back_before_the_next_message_tells_nothing() {
        let mut session = chat(false, false);

        session.set_memory_guards(guards(SaveRule::ChatOff, SaveRule::Saved));
        session.set_memory_guards(MemoryGuardState::OPEN);

        assert_eq!(told(&mut session), None);
    }

    #[test]
    fn a_message_not_delivered_or_without_room_tells_it_next_time() {
        let mut session = chat(false, false);
        session.set_memory_guards(guards(SaveRule::ChatOff, SaveRule::ChatOff));

        let (message, plan) = send(&mut session, &HookSet::empty(), &prompt("Lost"));
        assert!(notice(&message).is_some());
        session.send_failed(&plan);
        assert!(told(&mut session).is_some(), "told again after the failed send");

        session.set_memory_guards(MemoryGuardState::OPEN);
        let plan = session.on_send(&HookSet::empty(), &prompt("Too long"), SendOrigin::User);
        session.context_composed(&plan, &plan.context_parts(Vec::new()), &[]);
        assert!(told(&mut session).is_some(), "told on the next prompt");
    }

    #[test]
    fn hook_follow_ups_commands_and_utility_agents_carry_none() {
        let restricted = guards(SaveRule::Saved, SaveRule::ChatOff);

        let mut session = chat(false, false);
        session.set_memory_guards(restricted);

        let follow_up = session.on_send(&HookSet::empty(), &prompt("<harness-hook-followup>"), SendOrigin::Hook);
        assert!(follow_up.is_empty());

        let (message, _) = send(&mut session, &HookSet::empty(), &prompt("/compact"));
        assert_eq!(message, "/compact");

        assert!(told(&mut session).is_some(), "the next eligible prompt tells it");

        let mut utility = chat(true, true);
        utility.set_memory_guards(restricted);
        assert_eq!(told(&mut utility), None);
    }

    #[test]
    fn another_session_is_told_again() {
        let mut session = chat(false, false);
        session.set_memory_guards(guards(SaveRule::ChatOff, SaveRule::Saved));
        assert!(told(&mut session).is_some());

        session.on_send(&HookSet::empty(), &AgentAction::NewSession, SendOrigin::User);
        assert!(told(&mut session).is_some(), "the new session's agent was told nothing");
    }

    #[test]
    fn the_guards_are_read_from_the_global_and_the_chat_files() {
        let data = tempfile::tempdir().unwrap();
        let dir = data.path();
        let repo = tempfile::tempdir().unwrap();

        assert_eq!(read_memory_guards(dir, Some(KEY), None), MemoryGuardState::OPEN);

        let chat_guards = MemoryChatGuards {
            paused: false,
            notes: KindGuard { on: true, ask: true },
            scripts: KindGuard { on: false, ask: false },
        };
        write_chat(dir, KEY, &chat_guards, None).unwrap();
        assert_eq!(
            read_memory_guards(dir, Some(KEY), None),
            guards(SaveRule::Ask, SaveRule::ChatOff)
        );
        assert_eq!(
            read_memory_guards(dir, None, None),
            MemoryGuardState::OPEN,
            "without a key: no chat rules"
        );

        let root = turn_notes_off_for(dir, repo.path());
        assert_eq!(
            read_memory_guards(dir, Some(KEY), Some(&root)),
            guards(SaveRule::ProjectOff, SaveRule::ChatOff)
        );

        update_global(
            dir,
            &MemoryGuardsPatch {
                kind: MemoryGuardKind::Scripts,
                paused: Some(true),
                add_repo: None,
                remove_repo: None,
            },
        )
        .unwrap();
        assert_eq!(
            read_memory_guards(dir, Some(KEY), Some(&root)),
            guards(SaveRule::ProjectOff, SaveRule::PausedEverywhere)
        );

        fs::write(dir.join("guards").join("global.json"), "not json").unwrap();
        assert_eq!(read_memory_guards(dir, Some(KEY), None), MemoryGuardState::CLOSED);
    }

    #[test]
    fn the_chat_its_project_and_every_place_each_restrict_it() {
        let data = tempfile::tempdir().unwrap();
        let dir = data.path();
        let repo = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();

        let chat_guards = |paused: bool, notes: bool, scripts: bool| MemoryChatGuards {
            paused,
            notes: KindGuard { on: notes, ask: false },
            scripts: KindGuard {
                on: scripts,
                ask: false,
            },
        };

        write_chat(dir, KEY, &chat_guards(true, true, true), None).unwrap();
        assert_eq!(
            read_memory_guards(dir, Some(KEY), None),
            guards(SaveRule::ChatPaused, SaveRule::ChatPaused),
            "a paused chat"
        );
        assert_eq!(
            read_memory_guards(dir, None, None),
            MemoryGuardState::OPEN,
            "another chat"
        );

        write_chat(dir, KEY, &chat_guards(false, false, false), None).unwrap();
        assert_eq!(
            read_memory_guards(dir, Some(KEY), None),
            guards(SaveRule::ChatOff, SaveRule::ChatOff),
            "both kinds off in the chat"
        );

        write_chat(dir, KEY, &chat_guards(false, true, true), None).unwrap();
        let root = turn_notes_off_for(dir, repo.path());
        assert_eq!(
            read_memory_guards(dir, Some(KEY), Some(&root)),
            guards(SaveRule::ProjectOff, SaveRule::Saved),
            "notes off in its project"
        );
        assert_eq!(
            read_memory_guards(dir, Some(KEY), Some(elsewhere.path())),
            MemoryGuardState::OPEN,
            "outside that project"
        );

        write_chat(dir, KEY, &chat_guards(true, true, true), None).unwrap();
        for kind in [MemoryGuardKind::Notes, MemoryGuardKind::Scripts] {
            let pause = MemoryGuardsPatch {
                kind,
                paused: Some(true),
                add_repo: None,
                remove_repo: None,
            };
            update_global(dir, &pause).unwrap();
        }
        assert_eq!(
            read_memory_guards(dir, None, None),
            guards(SaveRule::PausedEverywhere, SaveRule::PausedEverywhere),
            "both kinds paused everywhere"
        );
        assert_eq!(
            read_memory_guards(dir, Some(KEY), Some(&root)),
            guards(SaveRule::PausedEverywhere, SaveRule::PausedEverywhere),
            "everywhere comes first"
        );
    }

    #[test]
    fn a_chat_file_that_cannot_be_read_refuses_every_save() {
        let data = tempfile::tempdir().unwrap();
        let dir = data.path();
        write_chat(dir, KEY, &MemoryChatGuards::default(), None).unwrap();

        fs::write(
            dir.join("guards").join("chats").join(format!("{KEY}.json")),
            r#"{"version":2,"paused":false,"notes":{"on":true,"ask":false},"scripts":{"on":true,"ask":false}}"#,
        )
        .unwrap();

        assert_eq!(read_memory_guards(dir, Some(KEY), None), MemoryGuardState::CLOSED);
        assert_eq!(
            read_memory_guards(dir, None, None),
            MemoryGuardState::OPEN,
            "another chat is not affected"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_global_file_behind_a_symbolic_link_refuses_every_save() {
        let data = tempfile::tempdir().unwrap();
        let dir = data.path();
        write_chat(dir, KEY, &MemoryChatGuards::default(), None).unwrap();

        let elsewhere = dir.join("elsewhere.json");
        fs::write(
            &elsewhere,
            r#"{"version":1,"notes":{"paused":false,"offRepos":[]},"scripts":{"paused":false,"offRepos":[]}}"#,
        )
        .unwrap();
        std::os::unix::fs::symlink(&elsewhere, dir.join("guards").join("global.json")).unwrap();

        assert_eq!(read_memory_guards(dir, None, None), MemoryGuardState::CLOSED);
    }

    /// Turns notes off for the project at `folder` and returns the root it was stored as.
    fn turn_notes_off_for(dir: &std::path::Path, folder: &std::path::Path) -> std::path::PathBuf {
        let off = update_global(
            dir,
            &MemoryGuardsPatch {
                kind: MemoryGuardKind::Notes,
                paused: None,
                add_repo: Some(folder.to_string_lossy().into_owned()),
                remove_repo: None,
            },
        )
        .unwrap();

        std::path::PathBuf::from(&off.notes.off_repos[0])
    }
}
