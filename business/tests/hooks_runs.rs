//! Which run a user message belongs to: messages sent while settles wait behind slow commands,
//! a message the agent never got, a message the harness handled without a run, and what the
//! chat remembers for `sessionEnd`.

mod hook_support;

mod hooks_runs {
    use harness_business::hooks::{EventOutcome, Firing, HookSet, SendOrigin};
    use harness_business::model::{AgentEvent, AgentReply, HookEvent, PromptStatus};
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
            "/work/\u{02BA}app",
            "/work/\u{3008}app\u{3009}",
            "\u{2212}work",
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

    #[test]
    fn a_path_windows_could_turn_into_a_special_character_is_left_out() {
        let hooks = set(vec![command(1, HookEvent::ToolEnd)]);
        let mut session = chat(false, false);
        user_send(&mut session, &hooks, &prompt("Fix it"));

        // Each becomes `" | < > ^ & % ! ( ) / \` in some Windows code page ("best fit").
        let look_alikes = [
            "src/\u{02BA}q.rs",
            "src/\u{030E}q.rs",
            "src/\u{02DD}q.rs",
            "src/\u{2223}p.rs",
            "src/\u{01C0}p.rs",
            "src/\u{2329}a.rs",
            "src/\u{3008}a.rs",
            "src/\u{2039}a.rs",
            "src/\u{232A}a.rs",
            "src/\u{3009}a.rs",
            "src/\u{203A}a.rs",
            "src/\u{0302}c.rs",
            "src/\u{02C4}c.rs",
            "src/\u{02C6}c.rs",
            "src/\u{2303}c.rs",
            "src/\u{FE60}c.rs",
            "src/\u{066A}c.rs",
            "src/\u{01C3}c.rs",
            "src/\u{207D}c\u{207E}.rs",
            "src\u{2215}s.rs",
            "src\u{2044}s.rs",
            "src\u{2216}s.rs",
            "src\u{29F5}s.rs",
            "\u{2212}rf",
            "\u{2010}rf",
            "\u{2011}rf",
            "\u{2012}rf",
            "\u{2013}rf",
            "\u{00AD}rf",
        ];
        let kept = [
            "src/a.rs",
            "notes/draft\u{2013}v2.md",
            "src/za\u{017C}\u{00F3}\u{0142}\u{0107}.rs",
            "docs/\u{65E5}\u{672C}.md",
        ];

        let touched: Vec<&str> = look_alikes.iter().chain(kept.iter()).copied().collect();
        let firing = tool_commands(session.on_event(&hooks, &tool_end("edit", false, &touched)));

        assert_eq!(env_value(&firing, "HARNESS_HOOK_TOUCHED"), Some(kept.join("\n")));
    }

    #[test]
    fn a_path_longer_than_the_limit_leaves_the_other_paths_in_place() {
        let hooks = set(vec![command(1, HookEvent::ToolEnd), command(2, HookEvent::SessionEnd)]);
        let mut session = chat(false, false);
        user_send(&mut session, &hooks, &prompt("Fix it"));

        let huge = format!("src/{}.rs", "x".repeat(16 * 1024));
        session.on_event(&hooks, &tool_end("edit", false, &["src/a.rs", "src/b.rs"]));

        // A tool call reporting it still passes its other paths on.
        let firing = tool_commands(session.on_event(&hooks, &tool_end("edit", false, &[huge.as_str(), "src/c.rs"])));
        assert_eq!(env_value(&firing, "HARNESS_HOOK_TOUCHED").as_deref(), Some("src/c.rs"));

        session.on_event(&hooks, &tool_end("edit", false, &["src/d.rs"]));

        let firing = session.on_stop(&hooks).expect("sessionEnd commands");
        assert_eq!(
            env_value(&firing, "HARNESS_HOOK_TOUCHED").as_deref(),
            Some("src/a.rs\nsrc/b.rs\nsrc/c.rs\nsrc/d.rs")
        );
    }

    /// A user run earns a follow-up, the follow-up goes out and the agent replies `reply`; then
    /// the user sends `message` before the engine sees a `Handled` result, and the run settles
    /// after a successful tool call. Returns what that settle asks for.
    fn after_follow_up_reply(reply: &AgentReply, message: &str) -> EventOutcome {
        let hooks = set(vec![follow_up(1, HookEvent::TurnEnd)]);
        let mut session = chat(false, false);
        let plan = follow_up_plan(user_run(&mut session, &hooks));
        assert!(session.take_follow_up(plan.ticket));

        session.on_send(&HookSet::empty(), &prompt("<harness-hook-followup>"), SendOrigin::Hook);
        session.follow_up_replied(reply);

        user_send(&mut session, &hooks, &prompt(message));
        session.on_event(&hooks, &handled());
        session.on_event(&hooks, &tool_end("read", false, &[]));
        session.on_event(&hooks, &settled())
    }

    #[test]
    fn a_follow_up_the_harness_handled_takes_no_user_message_from_the_run() {
        // OMP (`agentInvoked: false`) and PI (`disposition: "handled"`) handled the follow-up
        // without a run: the `Handled` result is the follow-up's, the run stays the user's.
        for reply in [
            AgentReply::Prompt {
                agent_invoked: Some(false),
                disposition: None,
            },
            AgentReply::Prompt {
                agent_invoked: None,
                disposition: Some("handled".to_owned()),
            },
        ] {
            let outcome = after_follow_up_reply(&reply, "Fix it");
            assert!(matches!(outcome, EventOutcome::FollowUp(_)), "{reply:?}");
        }

        // A follow-up that started a run leaves the `Handled` result to the user's `/command`.
        let started = AgentReply::Prompt {
            agent_invoked: Some(true),
            disposition: None,
        };
        assert_eq!(after_follow_up_reply(&started, "/model fast"), EventOutcome::Nothing);
    }
}
