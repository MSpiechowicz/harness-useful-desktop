//! What hooks do after a run: one combined follow-up per user run under the loop rule,
//! `toolEnd` on matching tools, `sessionEnd` once, and the run order of firings.

mod hook_support;

mod hooks_follow_up {
    use std::path::PathBuf;

    use harness_business::hooks::{EventOutcome, Firing, SendOrigin};
    use harness_business::model::{AgentAction, AgentEvent, HookEvent, HookScope, PromptStatus};
    use pretty_assertions::assert_eq;

    use super::hook_support::{
        PROJECT, chat, command, context, fired, follow_up, follow_up_plan, names, prompt, set, settled, texts,
        tool_end, user_run, user_send,
    };

    #[test]
    fn a_user_run_with_a_successful_tool_gets_one_combined_follow_up() {
        let mut matched = follow_up(1, HookEvent::ToolEnd);
        matched.matcher = Some("Edit|WRITE".to_owned());
        let mut unmatched = follow_up(2, HookEvent::ToolEnd);
        unmatched.matcher = Some("bash".to_owned());

        let hooks = set(vec![matched, unmatched, follow_up(3, HookEvent::TurnEnd)]);
        let mut session = chat(false, false);

        user_send(&mut session, &hooks, &prompt("Fix it"));
        session.on_event(&hooks, &tool_end("edit", false, &["src/a.rs"]));
        session.on_event(&hooks, &tool_end("write", false, &["src/b.rs"]));
        session.on_event(&hooks, &tool_end("read", true, &[]));

        let plan = follow_up_plan(session.on_event(&hooks, &settled()));

        assert_eq!(plan.firing.event, HookEvent::TurnEnd);
        assert_eq!(texts(&plan.firing), vec!["follow-up 1", "follow-up 3"]);
        assert!(session.take_follow_up(plan.ticket));
        assert!(!session.take_follow_up(plan.ticket), "a follow-up is taken once");
    }

    #[test]
    fn no_follow_up_without_a_user_message_or_a_successful_tool() {
        let hooks = set(vec![follow_up(1, HookEvent::ToolEnd), follow_up(2, HookEvent::TurnEnd)]);

        let mut no_message = chat(false, false);
        no_message.on_event(&hooks, &tool_end("read", false, &[]));
        assert_eq!(no_message.on_event(&hooks, &settled()), EventOutcome::Nothing);

        let mut only_errors = chat(false, false);
        user_send(&mut only_errors, &hooks, &prompt("Fix it"));
        only_errors.on_event(&hooks, &tool_end("edit", true, &[]));
        assert_eq!(only_errors.on_event(&hooks, &settled()), EventOutcome::Nothing);

        let mut no_tool = chat(false, false);
        user_send(&mut no_tool, &hooks, &prompt("Explain it"));
        assert_eq!(no_tool.on_event(&hooks, &settled()), EventOutcome::Nothing);
    }

    #[test]
    fn no_follow_up_after_an_abort_or_a_failed_prompt() {
        let hooks = set(vec![follow_up(1, HookEvent::TurnEnd)]);

        let mut aborted = chat(false, false);
        user_send(&mut aborted, &hooks, &prompt("Fix it"));
        aborted.on_event(&hooks, &tool_end("read", false, &[]));
        assert_eq!(
            aborted.on_event(&hooks, &AgentEvent::Settled { aborted: Some(true) }),
            EventOutcome::Nothing
        );

        for status in [PromptStatus::Error, PromptStatus::Aborted] {
            let mut failed = chat(false, false);
            user_send(&mut failed, &hooks, &prompt("Fix it"));
            failed.on_event(&hooks, &tool_end("read", false, &[]));
            failed.on_event(
                &hooks,
                &AgentEvent::PromptResult {
                    request_id: None,
                    status,
                    error: None,
                    retryable: false,
                },
            );

            assert_eq!(failed.on_event(&hooks, &settled()), EventOutcome::Nothing, "{status:?}");
        }
    }

    #[test]
    fn the_run_of_a_hook_follow_up_fires_nothing() {
        let hooks = set(vec![
            follow_up(1, HookEvent::ToolEnd),
            command(2, HookEvent::ToolEnd),
            follow_up(3, HookEvent::TurnEnd),
            context(4, HookEvent::PromptSubmit),
        ]);
        let mut session = chat(false, false);

        user_send(&mut session, &hooks, &prompt("Fix it"));
        let tools = session.on_event(&hooks, &tool_end("edit", false, &[]));
        let EventOutcome::ToolCommands(firing) = tools else {
            panic!("expected toolEnd commands, got {tools:?}");
        };
        session.on_tool_output(&firing, &[]);

        let plan = follow_up_plan(session.on_event(&hooks, &settled()));
        assert!(session.take_follow_up(plan.ticket));

        let sent = session.on_send(&hooks, &prompt("<harness-hook-followup>"), SendOrigin::Hook);
        assert!(sent.is_empty());
        assert_eq!(
            session.on_event(&hooks, &tool_end("edit", false, &[])),
            EventOutcome::Nothing
        );
        assert_eq!(session.on_event(&hooks, &settled()), EventOutcome::Nothing);

        // The next user run earns a follow-up again.
        let plain = set(vec![follow_up(3, HookEvent::TurnEnd)]);
        assert!(matches!(user_run(&mut session, &plain), EventOutcome::FollowUp(_)));
    }

    #[test]
    fn a_user_message_before_the_follow_up_goes_out_cancels_it() {
        let hooks = set(vec![follow_up(1, HookEvent::TurnEnd)]);
        let mut session = chat(false, false);

        let plan = follow_up_plan(user_run(&mut session, &hooks));
        user_send(
            &mut session,
            &hooks,
            &AgentAction::Steer {
                message: "wait".to_owned(),
                images: Vec::new(),
            },
        );

        assert!(!session.take_follow_up(plan.ticket));
    }

    #[test]
    fn tool_commands_run_on_each_match_and_the_latest_output_counts() {
        let mut lint = command(1, HookEvent::ToolEnd);
        lint.matcher = Some("edit".to_owned());
        let hooks = set(vec![lint, command(2, HookEvent::TurnEnd)]);
        let mut session = chat(false, false);

        user_send(&mut session, &hooks, &prompt("Fix it"));

        let EventOutcome::ToolCommands(first) = session.on_event(&hooks, &tool_end("Edit", false, &["a.rs"])) else {
            panic!("expected toolEnd commands");
        };
        let EventOutcome::ToolCommands(second) = session.on_event(&hooks, &tool_end("EDIT", true, &["b.rs", "c.rs"]))
        else {
            panic!("expected toolEnd commands");
        };

        let request = second.commands().next().unwrap();
        let env = |name: &str| {
            request
                .env
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.as_str())
        };
        assert_eq!(env("HARNESS_HOOK_EVENT"), Some("toolEnd"));
        assert_eq!(env("HARNESS_HOOK_TOOL"), Some("EDIT"));
        assert_eq!(env("HARNESS_HOOK_TOOL_ERROR"), Some("1"));
        assert_eq!(env("HARNESS_HOOK_TOUCHED"), Some("b.rs\nc.rs"));
        assert_eq!(env("HARNESS_HOOK_PROJECT"), Some(PROJECT));
        assert_eq!(env("HARNESS_HOOK_SESSION"), Some("session-1"));
        assert_eq!(request.cwd, PathBuf::from("/work/app/sub"));

        // The run settles while both firings still run: the follow-up waits for them.
        assert_eq!(session.on_event(&hooks, &settled()), EventOutcome::Nothing);

        let output = |firing: &Firing, text: &str| {
            let request = firing.commands().next().unwrap();
            harness_business::hooks::ResolvedPart::new(request.hook_id.clone(), request.hook_name.clone(), text)
        };

        assert_eq!(session.on_tool_output(&second, &[output(&second, "latest")]), None);
        let plan = session
            .on_tool_output(&first, &[output(&first, "earlier")])
            .expect("the last report completes the follow-up");

        assert_eq!(texts(&plan.firing), vec!["latest"]);

        let turn = plan.firing.commands().next().unwrap();
        assert_eq!(turn.event, HookEvent::TurnEnd);
        let touched = turn
            .env
            .iter()
            .find(|(key, _)| key == "HARNESS_HOOK_TOUCHED")
            .map(|(_, value)| value.clone());
        assert_eq!(touched.as_deref(), Some("a.rs\nb.rs\nc.rs"));
        assert!(session.take_follow_up(plan.ticket));
    }

    #[test]
    fn session_end_fires_once_with_commands_only() {
        let hooks = set(vec![
            command(1, HookEvent::SessionEnd),
            command(2, HookEvent::PromptSubmit),
        ]);
        let mut session = chat(false, false);

        user_send(&mut session, &hooks, &prompt("Fix it"));
        session.on_event(&hooks, &tool_end("edit", false, &["a.rs"]));
        session.on_event(&hooks, &settled());

        let firing = session.on_stop(&hooks).unwrap();
        assert_eq!(firing.event, HookEvent::SessionEnd);
        assert_eq!(names(&firing), vec!["hook 1"]);

        let touched = firing
            .commands()
            .next()
            .unwrap()
            .env
            .iter()
            .find(|(key, _)| key == "HARNESS_HOOK_TOUCHED");
        assert_eq!(touched.map(|(_, value)| value.as_str()), Some("a.rs"));

        assert_eq!(
            session.on_event(
                &hooks,
                &AgentEvent::Exited {
                    code: Some(0),
                    signal: None
                }
            ),
            EventOutcome::Nothing
        );

        let mut exited = chat(false, false);
        assert!(matches!(
            exited.on_event(
                &hooks,
                &AgentEvent::Exited {
                    code: Some(0),
                    signal: None
                }
            ),
            EventOutcome::SessionEnd(_)
        ));
    }

    #[test]
    fn a_message_sent_after_a_settle_still_queued_starts_the_next_run() {
        let mut slow = command(1, HookEvent::ToolEnd);
        slow.matcher = Some("read".to_owned());
        let hooks = set(vec![
            slow,
            follow_up(2, HookEvent::ToolEnd),
            follow_up(3, HookEvent::TurnEnd),
        ]);
        let mut session = chat(false, false);

        user_send(&mut session, &hooks, &prompt("Fix it"));
        let tools = session.on_event(&hooks, &tool_end("read", false, &[]));
        let EventOutcome::ToolCommands(firing) = tools else {
            panic!("expected toolEnd commands, got {tools:?}");
        };

        // The stream delivered the settle while the command still ran, then the user sent again.
        session.settles_seen(1);
        user_send(&mut session, &hooks, &prompt("Next"));

        session.on_tool_output(&firing, &[]);
        assert_eq!(
            session.on_event(&hooks, &settled()),
            EventOutcome::Nothing,
            "the first run gets no follow-up after the user moved on"
        );

        // The second run is the user's: its tool and turn hooks fire.
        let tools = session.on_event(&hooks, &tool_end("edit", false, &[]));
        assert_eq!(tools, EventOutcome::Nothing, "no command matches edit");

        let plan = follow_up_plan(session.on_event(&hooks, &settled()));
        assert_eq!(texts(&plan.firing), vec!["follow-up 2", "follow-up 3"]);
        assert!(session.take_follow_up(plan.ticket));
    }

    #[test]
    fn turn_end_commands_run_after_every_user_run() {
        let hooks = set(vec![command(1, HookEvent::TurnEnd), follow_up(2, HookEvent::TurnEnd)]);
        let turn_commands = |outcome: EventOutcome| match outcome {
            EventOutcome::TurnCommands(firing) => names(&firing),
            other => panic!("expected turnEnd commands, got {other:?}"),
        };

        // A chat-only run: no tool call, so no follow-up, but the command runs.
        let mut chat_only = chat(false, false);
        user_send(&mut chat_only, &hooks, &prompt("Explain it"));
        assert_eq!(turn_commands(chat_only.on_event(&hooks, &settled())), vec!["hook 1"]);

        let mut aborted = chat(false, false);
        user_send(&mut aborted, &hooks, &prompt("Fix it"));
        aborted.on_event(&hooks, &tool_end("read", false, &[]));
        assert_eq!(
            turn_commands(aborted.on_event(&hooks, &AgentEvent::Settled { aborted: Some(true) })),
            vec!["hook 1"]
        );

        let mut failed = chat(false, false);
        user_send(&mut failed, &hooks, &prompt("Fix it"));
        failed.on_event(
            &hooks,
            &AgentEvent::PromptResult {
                request_id: None,
                status: PromptStatus::Error,
                error: None,
                retryable: false,
            },
        );
        assert_eq!(turn_commands(failed.on_event(&hooks, &settled())), vec!["hook 1"]);

        // A run that earned a follow-up runs the command inside it.
        let mut earned = chat(false, false);
        let plan = follow_up_plan(user_run(&mut earned, &hooks));
        assert_eq!(names(&plan.firing), vec!["hook 1", "hook 2"]);

        // The run of the hook's own follow-up fires nothing.
        assert!(earned.take_follow_up(plan.ticket));
        earned.on_send(&hooks, &prompt("<harness-hook-followup>"), SendOrigin::Hook);
        assert_eq!(earned.on_event(&hooks, &settled()), EventOutcome::Nothing);
    }

    #[test]
    fn a_moved_chat_runs_its_commands_in_the_new_project() {
        let hooks = set(vec![command(1, HookEvent::PromptSubmit)]);
        let mut session = chat(false, false);

        session.move_to(PathBuf::from("/work/other/sub"), Some("/work/other".to_owned()));
        let plan = user_send(&mut session, &hooks, &prompt("Fix it"));

        let request = plan.firings[0].commands().next().unwrap();
        assert_eq!(request.cwd, PathBuf::from("/work/other/sub"));
        assert!(
            request
                .env
                .contains(&("HARNESS_HOOK_PROJECT".to_owned(), "/work/other".to_owned()))
        );
    }

    #[test]
    fn agent_reported_values_reach_commands_only_when_plain() {
        let hooks = set(vec![command(1, HookEvent::ToolEnd)]);
        let mut session = chat(false, false);
        user_send(&mut session, &hooks, &prompt("Fix it"));

        let event = tool_end(
            "edit",
            false,
            &[
                "src/a.rs",
                "a & calc.exe",
                "x\nrm -rf ~",
                "-rf",
                "100%done",
                "dir (copy)/b.rs",
                "src/b c.rs",
            ],
        );
        let EventOutcome::ToolCommands(firing) = session.on_event(&hooks, &event) else {
            panic!("expected toolEnd commands");
        };
        let env = &firing.commands().next().unwrap().env;
        let value = |name: &str| env.iter().find(|(key, _)| key == name).map(|(_, value)| value.as_str());

        assert_eq!(value("HARNESS_HOOK_TOUCHED"), Some("src/a.rs\nsrc/b c.rs"));
        assert_eq!(value("HARNESS_HOOK_TOOL"), Some("edit"));

        for name in ["edit; rm -rf ~", "", "edit\n", "&calc"] {
            let EventOutcome::ToolCommands(firing) = session.on_event(&hooks, &tool_end(name, false, &[])) else {
                panic!("expected toolEnd commands");
            };
            let env = &firing.commands().next().unwrap().env;
            let tool = env
                .iter()
                .find(|(key, _)| key == "HARNESS_HOOK_TOOL")
                .map(|(_, value)| value.as_str());

            assert_eq!(tool, Some(""), "{name:?}");
        }
    }

    #[test]
    fn hooks_fire_in_run_order() {
        let mut project = context(1, HookEvent::PromptSubmit);
        project.scope = HookScope::Project;
        project.project = Some(PROJECT.to_owned());

        let hooks = set(vec![
            project,
            context(9, HookEvent::PromptSubmit),
            context(3, HookEvent::PromptSubmit),
        ]);
        let plan = chat(false, false).on_send(&hooks, &prompt("hi"), SendOrigin::User);

        assert_eq!(
            fired(&plan),
            vec![(
                HookEvent::PromptSubmit,
                vec!["hook 3".to_owned(), "hook 9".to_owned(), "hook 1".to_owned()]
            )]
        );
    }
}
