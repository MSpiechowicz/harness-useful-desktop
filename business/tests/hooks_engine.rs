//! Which hooks fire in a chat: `sessionStart` once per new session, `promptSubmit` on each
//! eligible prompt, nothing in utility agents, and only active hooks of the chat's scope.

mod hook_support;

mod hooks_engine {
    use harness_business::hooks::{EventOutcome, HookSet, PlannedPart, ResolvedPart, SendOrigin};
    use harness_business::model::{
        AgentAction, HarnessId, HookEvent, HookScope, HookStatus, SessionRef, StreamingBehavior,
    };
    use pretty_assertions::assert_eq;

    use super::hook_support::{
        PROJECT, chat, command, context, fired, follow_up, prompt, set, settled, tool_end, with_status,
    };

    /// The text parts of `plan` as resolved context.
    fn resolved_parts(plan: &harness_business::hooks::SendPlan) -> Vec<ResolvedPart> {
        plan.firings
            .iter()
            .flat_map(|firing| firing.parts.iter())
            .filter_map(|part| match part {
                PlannedPart::Text(text) => Some(text.clone()),
                PlannedPart::Command(_) => None,
            })
            .collect()
    }

    #[test]
    fn session_start_fires_once_and_again_after_a_new_session() {
        let hooks = set(vec![
            context(1, HookEvent::SessionStart),
            context(2, HookEvent::PromptSubmit),
        ]);
        let mut session = chat(true, false);

        let first = session.on_send(&hooks, &prompt("hello"), SendOrigin::User);
        assert_eq!(
            fired(&first),
            vec![
                (HookEvent::SessionStart, vec!["hook 1".to_owned()]),
                (HookEvent::PromptSubmit, vec!["hook 2".to_owned()]),
            ]
        );

        let second = session.on_send(&hooks, &prompt("again"), SendOrigin::User);
        assert_eq!(
            fired(&second),
            vec![(HookEvent::PromptSubmit, vec!["hook 2".to_owned()])]
        );

        session.on_send(&hooks, &AgentAction::NewSession, SendOrigin::User);
        let renewed = session.on_send(&hooks, &prompt("new topic"), SendOrigin::User);
        assert_eq!(renewed.firings[0].event, HookEvent::SessionStart);
    }

    #[test]
    fn a_resumed_or_switched_session_does_not_fire_session_start() {
        let hooks = set(vec![context(1, HookEvent::SessionStart)]);

        let mut resumed = chat(false, false);
        assert!(resumed.on_send(&hooks, &prompt("hi"), SendOrigin::User).is_empty());

        let switches = [
            AgentAction::SwitchSession {
                session: SessionRef {
                    harness: HarnessId::Omp,
                    id: "other".to_owned(),
                },
            },
            AgentAction::Fork {
                entry_id: "e1".to_owned(),
            },
            AgentAction::Clone,
        ];

        for switch in switches {
            let mut session = chat(true, false);
            session.on_send(&hooks, &switch, SendOrigin::User);

            assert!(!session.is_armed(), "{switch:?}");
            assert!(session.on_send(&hooks, &prompt("hi"), SendOrigin::User).is_empty());
        }
    }

    #[test]
    fn only_eligible_user_prompts_get_context() {
        let hooks = set(vec![
            context(1, HookEvent::SessionStart),
            context(2, HookEvent::PromptSubmit),
        ]);
        let mut session = chat(true, false);

        let ineligible = [
            prompt("/compact"),
            prompt("!ls"),
            AgentAction::Prompt {
                message: "queued".to_owned(),
                images: Vec::new(),
                streaming_behavior: Some(StreamingBehavior::FollowUp),
            },
            AgentAction::Steer {
                message: "steer".to_owned(),
                images: Vec::new(),
            },
            AgentAction::FollowUp {
                message: "later".to_owned(),
                images: Vec::new(),
            },
        ];

        for action in &ineligible {
            assert!(
                session.on_send(&hooks, action, SendOrigin::User).is_empty(),
                "{action:?}"
            );
        }

        assert!(
            session
                .on_send(&hooks, &prompt("from a hook"), SendOrigin::Hook)
                .is_empty()
        );
        assert!(session.is_armed(), "ineligible prompts keep sessionStart armed");

        for _ in 0..3 {
            let plan = session.on_send(&hooks, &prompt("work"), SendOrigin::User);
            assert!(fired(&plan).contains(&(HookEvent::PromptSubmit, vec!["hook 2".to_owned()])));
        }
    }

    #[test]
    fn a_failed_send_keeps_session_start_armed() {
        let hooks = set(vec![context(1, HookEvent::SessionStart)]);
        let mut session = chat(true, false);

        let plan = session.on_send(&hooks, &prompt("hello"), SendOrigin::User);
        session.send_failed(&plan);

        assert!(session.is_armed());
        assert_eq!(
            fired(&session.on_send(&hooks, &prompt("hello"), SendOrigin::User)),
            vec![(HookEvent::SessionStart, vec!["hook 1".to_owned()])]
        );
    }

    #[test]
    fn session_start_context_that_did_not_fit_fires_again() {
        let hooks = set(vec![
            context(1, HookEvent::SessionStart),
            context(2, HookEvent::PromptSubmit),
        ]);
        let mut session = chat(true, false);

        let plan = session.on_send(&hooks, &prompt("hello"), SendOrigin::User);
        let resolved = resolved_parts(&plan);

        // Only the promptSubmit context fit in the message.
        session.context_composed(&plan, &resolved, &[resolved[1].hook_id.clone()]);
        assert!(session.is_armed());

        let plan = session.on_send(&hooks, &prompt("again"), SendOrigin::User);
        assert_eq!(
            fired(&plan),
            vec![
                (HookEvent::SessionStart, vec!["hook 1".to_owned()]),
                (HookEvent::PromptSubmit, vec!["hook 2".to_owned()]),
            ]
        );

        // Delivered this time: consumed.
        let resolved = resolved_parts(&plan);
        let delivered: Vec<String> = resolved.iter().map(|part| part.hook_id.clone()).collect();
        session.context_composed(&plan, &resolved, &delivered);
        assert!(!session.is_armed());
    }

    #[test]
    fn session_start_that_produced_nothing_stays_consumed() {
        let hooks = set(vec![command(1, HookEvent::SessionStart)]);
        let mut session = chat(true, false);

        let plan = session.on_send(&hooks, &prompt("hello"), SendOrigin::User);
        session.context_composed(&plan, &[], &[]);

        assert!(!session.is_armed());
    }

    #[test]
    fn a_utility_agent_fires_nothing() {
        let hooks = set(vec![
            context(1, HookEvent::SessionStart),
            context(2, HookEvent::PromptSubmit),
            follow_up(3, HookEvent::TurnEnd),
            command(4, HookEvent::SessionEnd),
        ]);
        let mut session = chat(true, true);

        assert!(
            session
                .on_send(&hooks, &prompt("title this"), SendOrigin::User)
                .is_empty()
        );
        assert_eq!(
            session.on_event(&hooks, &tool_end("read", false, &[])),
            EventOutcome::Nothing
        );
        assert_eq!(session.on_event(&hooks, &settled()), EventOutcome::Nothing);
        assert_eq!(session.on_stop(&hooks), None);
    }

    #[test]
    fn only_active_hooks_of_the_chat_scope_are_selected() {
        let mut other_project = context(6, HookEvent::PromptSubmit);
        other_project.scope = HookScope::Project;
        other_project.project = Some("/work/other".to_owned());

        let mut this_project = context(7, HookEvent::PromptSubmit);
        this_project.scope = HookScope::Project;
        this_project.project = Some(PROJECT.to_owned());

        let views = vec![
            with_status(context(1, HookEvent::PromptSubmit), HookStatus::Pending),
            with_status(context(2, HookEvent::PromptSubmit), HookStatus::Off),
            with_status(context(3, HookEvent::PromptSubmit), HookStatus::Paused),
            with_status(context(4, HookEvent::PromptSubmit), HookStatus::Invalid),
            with_status(context(5, HookEvent::PromptSubmit), HookStatus::Active),
            with_status(other_project, HookStatus::Active),
            with_status(this_project, HookStatus::Active),
        ];

        let in_project = HookSet::for_chat(&views, Some(PROJECT));
        let plan = chat(false, false).on_send(&in_project, &prompt("hi"), SendOrigin::User);
        assert_eq!(
            fired(&plan),
            vec![(HookEvent::PromptSubmit, vec!["hook 5".to_owned(), "hook 7".to_owned()])]
        );

        let without_project = HookSet::for_chat(&views, None);
        let plan = chat(false, false).on_send(&without_project, &prompt("hi"), SendOrigin::User);
        assert_eq!(fired(&plan), vec![(HookEvent::PromptSubmit, vec!["hook 5".to_owned()])]);
    }
}
