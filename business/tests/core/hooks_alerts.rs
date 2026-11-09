//! When a chat raises an alert: the end of a run (after any hook follow-up), a dialog, a failed or
//! stopped run, and the clean or unexpected end of the agent process.

use crate::hook_support;

mod hooks_alerts {
    use std::time::{Duration, Instant};

    use harness_business::hooks::{
        EventOutcome, Firing, FollowUpPlan, HookSession, HookSet, NEEDS_YOU_GAP, SendOrigin, exit_alert,
    };
    use harness_business::model::{
        AgentAction, AgentEvent, AgentReply, ChatAlertKind, HarnessId, HookEvent, PromptStatus, SessionRef, UiRequest,
        UiRequestKind,
    };
    use pretty_assertions::assert_eq;

    use super::hook_support::{chat, command, follow_up, follow_up_plan, prompt, set, settled, tool_end, user_send};

    const FINISHED: Option<ChatAlertKind> = Some(ChatAlertKind::Finished);
    const FAILED: Option<ChatAlertKind> = Some(ChatAlertKind::Failed);
    const NEEDS_YOU: Option<ChatAlertKind> = Some(ChatAlertKind::NeedsYou);

    fn aborted() -> AgentEvent {
        AgentEvent::Settled { aborted: Some(true) }
    }

    fn prompt_result(status: PromptStatus) -> AgentEvent {
        AgentEvent::PromptResult {
            request_id: None,
            status,
            error: None,
            retryable: false,
        }
    }

    fn dialog() -> AgentEvent {
        let kind = UiRequestKind::Confirm {
            title: "Run it".to_owned(),
            message: "Run cargo test?".to_owned(),
            timeout_ms: None,
        };

        AgentEvent::UiRequest {
            request: UiRequest::new("ask-1", kind),
        }
    }

    fn ran(agent_invoked: bool) -> AgentReply {
        AgentReply::Prompt {
            agent_invoked: Some(agent_invoked),
            disposition: None,
        }
    }

    /// A user prompt and a successful tool call, not settled yet.
    fn working(session: &mut HookSession, hooks: &HookSet) {
        user_send(session, hooks, &prompt("Fix it"));
        session.on_event(hooks, &tool_end("read", false, &[]));
    }

    /// A user run that earned a follow-up: its settle, and the plan the shell composes.
    fn earned(session: &mut HookSession, hooks: &HookSet) -> FollowUpPlan {
        working(session, hooks);

        follow_up_plan(session.on_event(hooks, &settled()))
    }

    /// The shell sends the follow-up of `plan`, and the agent starts a run for it.
    fn follow_up_goes_out(session: &mut HookSession, plan: &FollowUpPlan) {
        assert!(session.take_follow_up(plan.ticket), "the follow-up may still go out");

        session.on_send(&HookSet::empty(), &prompt("<harness-hook-followup>"), SendOrigin::Hook);
        session.follow_up_replied(&ran(true));
    }

    /// A chat whose finished run earned a turn-end follow-up that has gone out and is running.
    fn follow_up_running() -> (HookSet, HookSession) {
        let hooks = set(vec![follow_up(1, HookEvent::TurnEnd)]);
        let mut session = chat(false, false);
        let plan = earned(&mut session, &hooks);

        follow_up_goes_out(&mut session, &plan);

        (hooks, session)
    }

    /// A run that earned a follow-up settled while its `toolEnd` command still runs: the follow-up
    /// waits for the output, and so does the run's alert.
    fn settled_while_commands_run() -> (HookSet, HookSession, Firing) {
        let hooks = set(vec![command(1, HookEvent::ToolEnd), follow_up(2, HookEvent::TurnEnd)]);
        let mut session = chat(false, false);

        user_send(&mut session, &hooks, &prompt("Fix it"));
        let EventOutcome::ToolCommands(firing) = session.on_event(&hooks, &tool_end("read", false, &[])) else {
            panic!("expected toolEnd commands");
        };

        assert_eq!(session.on_event(&hooks, &settled()), EventOutcome::Nothing);
        assert_eq!(session.take_alert(), None, "the follow-up waits for the output");

        (hooks, session, firing)
    }

    #[test]
    fn a_run_that_ends_gives_one_finished() {
        let hooks = set(Vec::new());
        let mut session = chat(false, false);

        working(&mut session, &hooks);
        assert_eq!(session.take_alert(), None, "the run is still going");

        session.on_event(&hooks, &settled());
        assert_eq!(session.take_alert(), FINISHED);
        assert_eq!(session.take_alert(), None, "an alert is taken once");
    }

    #[test]
    fn a_run_without_a_tool_call_finishes_too() {
        let hooks = set(vec![follow_up(1, HookEvent::TurnEnd)]);
        let mut session = chat(false, false);

        user_send(&mut session, &hooks, &prompt("Explain it"));
        session.on_event(&hooks, &settled());

        assert_eq!(session.take_alert(), FINISHED);
    }

    #[test]
    fn a_run_with_a_follow_up_finishes_when_the_follow_up_run_settles() {
        let hooks = set(vec![follow_up(1, HookEvent::TurnEnd)]);
        let mut session = chat(false, false);
        let plan = earned(&mut session, &hooks);

        assert_eq!(session.take_alert(), None, "the follow-up is still to come");

        follow_up_goes_out(&mut session, &plan);
        assert_eq!(session.take_alert(), None, "the follow-up runs");

        session.on_event(&hooks, &tool_end("edit", false, &[]));
        assert_eq!(session.on_event(&hooks, &settled()), EventOutcome::Nothing);
        assert_eq!(session.take_alert(), FINISHED);
        assert_eq!(session.take_alert(), None, "one alert for the whole chain");

        // The next run of the user is a run of its own.
        working(&mut session, &hooks);
        session.on_event(&hooks, &settled());
        assert_eq!(
            session.take_alert(),
            None,
            "it earns a follow-up of its own, which holds it"
        );
    }

    #[test]
    fn a_follow_up_that_never_runs_releases_the_alert_at_once() {
        let hooks = set(vec![follow_up(1, HookEvent::TurnEnd)]);

        // Nothing could be composed (every hook command failed), or sending failed.
        let mut unsent = chat(false, false);
        earned(&mut unsent, &hooks);
        assert_eq!(unsent.take_alert(), None);
        unsent.follow_up_unsent();
        assert_eq!(unsent.take_alert(), FINISHED);
        assert_eq!(unsent.take_alert(), None);

        // The user sent a message first: the ticket is refused.
        let mut refused = chat(false, false);
        let plan = earned(&mut refused, &hooks);
        user_send(&mut refused, &hooks, &prompt("Wait"));
        assert!(!refused.take_follow_up(plan.ticket));
        refused.follow_up_unsent();
        assert_eq!(refused.take_alert(), FINISHED);

        // The harness handled the follow-up without a run.
        let mut handled = chat(false, false);
        let plan = earned(&mut handled, &hooks);
        assert!(handled.take_follow_up(plan.ticket));
        handled.on_send(&HookSet::empty(), &prompt("<harness-hook-followup>"), SendOrigin::Hook);
        handled.follow_up_replied(&ran(false));
        assert_eq!(handled.take_alert(), FINISHED);

        // The release does not come again at a later settle.
        handled.on_event(&hooks, &prompt_result(PromptStatus::Handled));
        handled.on_event(&hooks, &settled());
        assert_eq!(handled.take_alert(), None);
    }

    #[test]
    fn a_user_message_while_the_commands_run_releases_the_alert_of_the_settled_run() {
        let (hooks, mut session, firing) = settled_while_commands_run();

        user_send(&mut session, &hooks, &prompt("Never mind"));
        session.on_tool_output(&firing, &[]);

        assert_eq!(session.take_alert(), FINISHED, "the follow-up was cancelled");
    }

    /// The actions that take the chat to another session.
    fn session_changes() -> Vec<AgentAction> {
        let other = SessionRef {
            harness: HarnessId::Omp,
            id: "other".to_owned(),
        };

        vec![
            AgentAction::NewSession,
            AgentAction::SwitchSession { session: other },
            AgentAction::Fork {
                entry_id: "e1".to_owned(),
            },
            AgentAction::Clone,
        ]
    }

    #[test]
    fn a_new_other_forked_or_cloned_session_releases_the_alert_a_follow_up_held() {
        let hooks = set(vec![follow_up(1, HookEvent::TurnEnd)]);

        for change in session_changes() {
            // The follow-up runs.
            let (_, mut running) = follow_up_running();
            user_send(&mut running, &hooks, &change);
            assert_eq!(running.take_alert(), FINISHED, "{change:?} while the follow-up runs");
            assert_eq!(running.take_alert(), None);

            // The follow-up is composed and not sent yet: its ticket is refused afterwards.
            let mut planned = chat(false, false);
            let plan = earned(&mut planned, &hooks);
            user_send(&mut planned, &hooks, &change);
            assert_eq!(
                planned.take_alert(),
                FINISHED,
                "{change:?} before the follow-up goes out"
            );
            assert!(!planned.take_follow_up(plan.ticket));
            planned.follow_up_unsent();
            assert_eq!(planned.take_alert(), None, "released once, not again");
        }
    }

    #[test]
    fn a_new_session_releases_the_alert_of_a_run_that_waits_for_its_commands() {
        let (hooks, mut session, firing) = settled_while_commands_run();

        user_send(&mut session, &hooks, &AgentAction::NewSession);
        assert_eq!(session.take_alert(), FINISHED);

        session.on_tool_output(&firing, &[]);
        assert_eq!(session.take_alert(), None, "released once, not again");
    }

    #[test]
    fn a_stopped_run_gives_nothing() {
        let hooks = set(vec![follow_up(1, HookEvent::TurnEnd)]);

        let mut by_settle = chat(false, false);
        working(&mut by_settle, &hooks);
        by_settle.on_event(&hooks, &aborted());
        assert_eq!(by_settle.take_alert(), None);

        let mut by_result = chat(false, false);
        working(&mut by_result, &hooks);
        by_result.on_event(&hooks, &prompt_result(PromptStatus::Aborted));
        by_result.on_event(&hooks, &settled());
        assert_eq!(by_result.take_alert(), None);
    }

    #[test]
    fn a_stopped_follow_up_gives_nothing_at_all() {
        let (hooks, mut session) = follow_up_running();

        session.on_event(&hooks, &aborted());

        assert_eq!(session.take_alert(), None);
    }

    #[test]
    fn a_failed_run_gives_failed() {
        let hooks = set(vec![follow_up(1, HookEvent::TurnEnd)]);
        let mut session = chat(false, false);

        working(&mut session, &hooks);
        session.on_event(&hooks, &prompt_result(PromptStatus::Error));
        assert_eq!(session.on_event(&hooks, &settled()), EventOutcome::Nothing);

        assert_eq!(
            session.take_alert(),
            FAILED,
            "a failed run earns no follow-up to wait for"
        );
        assert_eq!(session.take_alert(), None);
    }

    #[test]
    fn a_failed_follow_up_run_gives_failed() {
        let (hooks, mut session) = follow_up_running();

        session.on_event(&hooks, &prompt_result(PromptStatus::Error));
        session.on_event(&hooks, &settled());

        assert_eq!(session.take_alert(), FAILED);
    }

    #[test]
    fn a_settle_a_message_went_out_after_gives_nothing_and_the_later_settle_alerts() {
        let hooks = set(Vec::new());
        let mut session = chat(false, false);

        working(&mut session, &hooks);

        // The stream delivered the settle, the user sent again before the engine handled it.
        session.settles_seen(1);
        user_send(&mut session, &hooks, &prompt("Next"));

        session.on_event(&hooks, &settled());
        assert_eq!(session.take_alert(), None, "the chat is busy again");

        session.on_event(&hooks, &tool_end("edit", false, &[]));
        session.on_event(&hooks, &settled());
        assert_eq!(session.take_alert(), FINISHED);
    }

    #[test]
    fn a_run_the_engine_started_alone_gives_nothing() {
        let hooks = set(vec![follow_up(1, HookEvent::TurnEnd)]);
        let mut session = chat(false, false);

        session.on_event(&hooks, &tool_end("read", false, &[]));
        session.on_event(&hooks, &settled());

        assert_eq!(session.take_alert(), None);
    }

    #[test]
    fn a_message_the_harness_handled_without_a_run_gives_nothing() {
        let hooks = set(Vec::new());
        let mut session = chat(false, false);

        user_send(&mut session, &hooks, &prompt("/compact"));
        session.on_event(&hooks, &prompt_result(PromptStatus::Handled));
        session.on_event(&hooks, &settled());

        assert_eq!(session.take_alert(), None);
    }

    #[test]
    fn a_dialog_gives_needs_you_and_changes_nothing_for_the_hooks() {
        let hooks = set(vec![follow_up(1, HookEvent::TurnEnd)]);
        let mut session = chat(false, false);
        let now = Instant::now();

        working(&mut session, &hooks);

        assert_eq!(session.on_event_at(&hooks, &dialog(), now), EventOutcome::Nothing);
        assert_eq!(session.take_alert(), NEEDS_YOU);

        let plan = follow_up_plan(session.on_event(&hooks, &settled()));
        assert!(session.take_follow_up(plan.ticket), "the run still earns its follow-up");
    }

    #[test]
    fn a_burst_of_dialogs_gives_one_alert_and_a_later_dialog_another() {
        let hooks = set(Vec::new());
        let mut session = chat(false, false);
        let start = Instant::now();
        let mut alerts = Vec::new();

        let almost = NEEDS_YOU_GAP - Duration::from_millis(1);

        for after in [Duration::ZERO, Duration::from_secs(1), Duration::from_secs(3), almost] {
            session.on_event_at(&hooks, &dialog(), start + after);
            alerts.push(session.take_alert());
        }

        assert_eq!(alerts, vec![NEEDS_YOU, None, None, None], "one alert for the burst");

        session.on_event_at(&hooks, &dialog(), start + NEEDS_YOU_GAP);
        assert_eq!(session.take_alert(), NEEDS_YOU, "five seconds after the first one");

        session.on_event_at(&hooks, &dialog(), start + NEEDS_YOU_GAP + Duration::from_secs(1));
        assert_eq!(session.take_alert(), None);
    }

    #[test]
    fn a_dialog_does_not_hold_back_the_alert_of_the_run() {
        let hooks = set(Vec::new());
        let mut session = chat(false, false);

        working(&mut session, &hooks);
        session.on_event(&hooks, &dialog());
        assert_eq!(session.take_alert(), NEEDS_YOU);

        session.on_event(&hooks, &settled());
        assert_eq!(session.take_alert(), FINISHED);
    }

    #[test]
    fn a_utility_agent_never_alerts() {
        let hooks = set(Vec::new());
        let mut session = chat(false, true);

        user_send(&mut session, &hooks, &prompt("Title this"));
        session.on_event(&hooks, &dialog());
        session.on_event(&hooks, &prompt_result(PromptStatus::Error));
        session.on_event(&hooks, &settled());

        assert_eq!(session.take_alert(), None);
    }

    #[test]
    fn the_exit_ends_what_a_follow_up_waited_for() {
        let (hooks, mut session) = follow_up_running();

        session.on_event(
            &hooks,
            &AgentEvent::Exited {
                code: Some(1),
                signal: None,
            },
        );

        assert_eq!(session.take_alert(), None, "the shell decides about the exit itself");
    }

    #[test]
    fn only_an_unclean_exit_is_a_failure() {
        assert_eq!(exit_alert(Some(0), None), None);

        assert_eq!(exit_alert(Some(1), None), FAILED);
        assert_eq!(exit_alert(Some(0), Some(15)), FAILED);
        assert_eq!(exit_alert(None, Some(9)), FAILED);
        assert_eq!(exit_alert(None, None), FAILED);
    }
}
