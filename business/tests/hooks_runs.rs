//! Which run a user message belongs to: messages sent while settles wait behind slow commands,
//! a message the agent never got, a message the harness handled without a run, and what the
//! chat remembers for `sessionEnd`.

mod hook_support;

mod hooks_runs {
    use harness_business::hooks::{EventOutcome, Firing, SendOrigin};
    use harness_business::model::{AgentEvent, HookEvent, PromptStatus};
    use pretty_assertions::assert_eq;

    use super::hook_support::{
        chat, command, follow_up, follow_up_plan, matching, names, prompt, set, settled, texts, tool_end, user_run,
        user_send,
    };

    fn turn_commands(outcome: EventOutcome) -> Vec<String> {
        match outcome {
            EventOutcome::TurnCommands(firing) => names(&firing),
            other => panic!("expected turnEnd commands, got {other:?}"),
        }
    }

    fn tool_commands(outcome: EventOutcome) -> Firing {
        match outcome {
            EventOutcome::ToolCommands(firing) => firing,
            other => panic!("expected toolEnd commands, got {other:?}"),
        }
    }

    fn handled() -> AgentEvent {
        AgentEvent::PromptResult {
            request_id: None,
            status: PromptStatus::Handled,
            error: None,
            retryable: false,
        }
    }

    fn env_value(firing: &Firing, name: &str) -> Option<String> {
        firing
            .commands()
            .next()?
            .env
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    }

    #[test]
    fn two_messages_sent_behind_two_queued_settles_each_start_their_own_run() {
        let hooks = set(vec![
            matching(command(1, HookEvent::ToolEnd), "read"),
            follow_up(2, HookEvent::ToolEnd),
            follow_up(3, HookEvent::TurnEnd),
            command(4, HookEvent::TurnEnd),
        ]);
        let mut session = chat(false, false);

        user_send(&mut session, &hooks, &prompt("Fix it"));
        let firing = tool_commands(session.on_event(&hooks, &tool_end("read", false, &[])));

        // While the toolEnd command runs, the stream delivers the first settle, the user sends,
        // the second run settles too, and the user sends again.
        session.settles_seen(1);
        user_send(&mut session, &hooks, &prompt("Next"));
        session.settles_seen(2);
        user_send(&mut session, &hooks, &prompt("Third"));

        session.on_tool_output(&firing, &[]);

        // Both earlier runs were the user's: their turnEnd commands run, without a follow-up.
        assert_eq!(turn_commands(session.on_event(&hooks, &settled())), vec!["hook 4"]);
        session.on_event(&hooks, &tool_end("edit", false, &[]));
        assert_eq!(turn_commands(session.on_event(&hooks, &settled())), vec!["hook 4"]);

        // The third run is the user's too, and nobody moved on after it.
        session.on_event(&hooks, &tool_end("edit", false, &[]));
        let follow_up = follow_up_plan(session.on_event(&hooks, &settled()));
        assert_eq!(names(&follow_up.firing), vec!["hook 2", "hook 3", "hook 4"]);
        assert!(session.take_follow_up(follow_up.ticket));
    }

    #[test]
    fn a_settle_seen_while_the_context_commands_ran_ends_the_run_before_the_message() {
        let hooks = set(vec![
            command(1, HookEvent::PromptSubmit),
            follow_up(2, HookEvent::TurnEnd),
        ]);
        let mut session = chat(false, false);

        user_send(&mut session, &hooks, &prompt("Fix it"));
        session.on_event(&hooks, &tool_end("read", false, &[]));

        // The next message is planned, then its promptSubmit command runs while the first run
        // settles; the count is read again right before the message goes out.
        let mut plan = session.on_send(&hooks, &prompt("Next"), SendOrigin::User);
        session.settles_seen(1);
        session.sending(&mut plan);

        assert_eq!(
            session.on_event(&hooks, &settled()),
            EventOutcome::Nothing,
            "the first run gets no follow-up: the user moved on after it"
        );

        session.on_event(&hooks, &tool_end("edit", false, &[]));
        assert!(matches!(
            session.on_event(&hooks, &settled()),
            EventOutcome::FollowUp(_)
        ));
    }

    #[test]
    fn a_message_the_agent_never_got_leaves_the_follow_up_in_place() {
        let hooks = set(vec![follow_up(1, HookEvent::TurnEnd)]);

        // A follow-up ready to go out.
        let mut ready = chat(false, false);
        let plan = follow_up_plan(user_run(&mut ready, &hooks));
        let refused = user_send(&mut ready, &hooks, &prompt("Next"));
        ready.send_failed(&refused);
        assert!(ready.take_follow_up(plan.ticket));

        // A follow-up still waiting for a toolEnd command.
        let hooks = set(vec![command(1, HookEvent::ToolEnd), follow_up(2, HookEvent::TurnEnd)]);
        let mut waiting = chat(false, false);
        user_send(&mut waiting, &hooks, &prompt("Fix it"));
        let firing = tool_commands(waiting.on_event(&hooks, &tool_end("read", false, &[])));
        assert_eq!(waiting.on_event(&hooks, &settled()), EventOutcome::Nothing);

        let refused = user_send(&mut waiting, &hooks, &prompt("Next"));
        waiting.send_failed(&refused);

        let plan = waiting.on_tool_output(&firing, &[]).expect("the follow-up");
        assert_eq!(texts(&plan.firing), vec!["follow-up 2"]);
        assert!(waiting.take_follow_up(plan.ticket));

        // A message refused after a settle the engine had not handled: that run keeps its
        // follow-up, and the next run is not the user's.
        let hooks = set(vec![follow_up(1, HookEvent::TurnEnd)]);
        let mut queued = chat(false, false);
        user_send(&mut queued, &hooks, &prompt("Fix it"));
        queued.on_event(&hooks, &tool_end("read", false, &[]));
        queued.settles_seen(1);
        let refused = user_send(&mut queued, &hooks, &prompt("Next"));
        queued.send_failed(&refused);

        let plan = follow_up_plan(queued.on_event(&hooks, &settled()));
        assert!(queued.take_follow_up(plan.ticket));
        queued.on_event(&hooks, &tool_end("read", false, &[]));
        assert_eq!(queued.on_event(&hooks, &settled()), EventOutcome::Nothing);
    }

    #[test]
    fn a_message_handled_without_a_run_does_not_make_the_next_run_the_users() {
        let hooks = set(vec![follow_up(1, HookEvent::TurnEnd)]);

        // `/model` while idle: the harness handles it, then a run nobody sent happens.
        let mut idle = chat(false, false);
        user_send(&mut idle, &hooks, &prompt("/model fast"));
        idle.on_event(&hooks, &handled());
        idle.on_event(&hooks, &tool_end("read", false, &[]));
        assert_eq!(idle.on_event(&hooks, &settled()), EventOutcome::Nothing);

        // During a user run, a handled `/command` leaves the run the user's.
        let mut busy = chat(false, false);
        user_send(&mut busy, &hooks, &prompt("Fix it"));
        user_send(&mut busy, &hooks, &prompt("/model fast"));
        busy.on_event(&hooks, &handled());
        busy.on_event(&hooks, &tool_end("read", false, &[]));
        assert!(matches!(busy.on_event(&hooks, &settled()), EventOutcome::FollowUp(_)));
    }

    #[test]
    fn session_end_sees_the_latest_paths_the_chat_changed() {
        let hooks = set(vec![command(1, HookEvent::SessionEnd)]);
        let mut session = chat(false, false);
        user_send(&mut session, &hooks, &prompt("Fix it"));

        for number in 0..300 {
            let path = format!("src/file-{number:03}.rs");
            session.on_event(&hooks, &tool_end("edit", false, &[path.as_str()]));
        }

        let firing = session.on_stop(&hooks).expect("sessionEnd commands");
        let touched = env_value(&firing, "HARNESS_HOOK_TOUCHED").unwrap();
        let paths: Vec<&str> = touched.lines().collect();

        assert_eq!(paths.len(), 256);
        assert_eq!(paths.first(), Some(&"src/file-044.rs"));
        assert_eq!(paths.last(), Some(&"src/file-299.rs"));

        // Long paths: the latest ones that fit in 16 KiB.
        let mut long = chat(false, false);
        let padding = "d".repeat(200);

        for number in 0..100 {
            let path = format!("{padding}/file-{number:03}.rs");
            long.on_event(&hooks, &tool_end("edit", false, &[path.as_str()]));
        }

        let firing = long.on_stop(&hooks).expect("sessionEnd commands");
        let touched = env_value(&firing, "HARNESS_HOOK_TOUCHED").unwrap();

        assert!(touched.len() <= 16 * 1024);
        assert!(touched.ends_with("/file-099.rs"), "the newest path is kept");
        assert!(!touched.contains("/file-000.rs"), "the oldest path is dropped");
    }

    #[test]
    fn a_project_root_that_is_not_a_plain_path_reaches_commands_empty() {
        let hooks = set(vec![command(1, HookEvent::ToolEnd)]);
        let mut session = chat(false, false);
        user_send(&mut session, &hooks, &prompt("Fix it"));

        for project in [
            "/work/a&calc",
            "/work/100%",
            "/work/x\ny",
            "/work/\u{FF02}quoted",
            "/work/\u{00A5}yen",
            "/work/\u{20A9}won",
        ] {
            session.move_to(project.into(), Some(project.to_owned()));
            let firing = tool_commands(session.on_event(&hooks, &tool_end("edit", false, &[])));

            assert_eq!(
                env_value(&firing, "HARNESS_HOOK_PROJECT").as_deref(),
                Some(""),
                "{project:?}"
            );
        }

        session.move_to("/work/my app".into(), Some("/work/my app".to_owned()));
        let event = tool_end("edit", false, &["src/a.rs", "src/\u{FF06}calc.rs", "src/\u{00A5}.rs"]);
        let firing = tool_commands(session.on_event(&hooks, &event));

        assert_eq!(
            env_value(&firing, "HARNESS_HOOK_PROJECT").as_deref(),
            Some("/work/my app")
        );
        assert_eq!(env_value(&firing, "HARNESS_HOOK_TOUCHED").as_deref(), Some("src/a.rs"));
    }
}
