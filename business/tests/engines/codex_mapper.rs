//! The Codex chat's event mapping (`codex_rpc::mapper::CodexMapper`) over the recorded app-server
//! runs (`fixtures/codex/app_server`), replayed as the engine drives the mapper: turns, replies,
//! tool cards with touched paths, approvals accepted and declined, steering, an interrupt, a
//! `!command`, a compaction, a rename and token counts.

use crate::codex_recorded;

use codex_recorded::{FILES, THREAD, frame, replay, replay_in, shape};
use harness_business::codex_rpc::mapper::{AskKind, COMMAND_DECLINED, Hint, ShellRun, UserMessage};
use harness_business::codex_rpc::stats::{CodexStats, MessageCounts};
use harness_business::codex_rpc::wire::RequestId;
use harness_business::codex_rpc::wire::results::ThreadStatus;
use harness_business::model::{AgentEvent, ChatBlock, ContextUsage, MessageRole, PromptStatus, TokenUsage};
use pretty_assertions::assert_eq;

/// Each `toolEnd`: name, error, result text, touched paths.
fn tool_ends(events: &[AgentEvent]) -> Vec<(String, bool, String, Vec<String>)> {
    let mut ends = Vec::new();

    for event in events {
        if let AgentEvent::ToolEnd {
            name,
            is_error,
            result,
            touched_paths,
            ..
        } = event
        {
            let text = result["content"][0]["text"].as_str().unwrap_or_default();
            ends.push((name.clone(), *is_error, text.to_owned(), touched_paths.clone()));
        }
    }

    ends
}

/// Each assistant message that ended: its text (or thinking) and stop reason.
fn replies(events: &[AgentEvent]) -> Vec<(String, Option<String>)> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::MessageEnd { message, .. } if message.role == MessageRole::Assistant => {
                let text = message
                    .blocks
                    .iter()
                    .map(|block| match block {
                        ChatBlock::Text { text } | ChatBlock::Thinking { text, .. } => text.as_str(),
                        _ => "",
                    })
                    .collect();

                Some((text, message.stop_reason.clone()))
            }
            _ => None,
        })
        .collect()
}

fn prompt_results(events: &[AgentEvent]) -> Vec<(PromptStatus, Option<String>)> {
    let mut results = Vec::new();

    for event in events {
        if let AgentEvent::PromptResult { status, error, .. } = event {
            results.push((*status, error.clone()));
        }
    }

    results
}

fn settles(events: &[AgentEvent]) -> Vec<Option<bool>> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::Settled { aborted } => Some(*aborted),
            _ => None,
        })
        .collect()
}

/// The recorded probe command as Codex runs it (its script is double-quoted, so it stays wrapped).
const PROBE_COMMAND: &str =
    r#"/usr/bin/bash -lc "pwd && rg --files -g 'AGENTS.md' -g '"'!node_modules'"' -g '"'!vendor'"'""#; // guard-allow: installer-shell

#[test]
fn every_recorded_run_maps_cleanly_and_ends_what_it_starts() {
    let pairs = [
        ("agentStart", "agentEnd"),
        ("agentStart", "settled"),
        ("turnStart", "turnEnd"),
        ("messageStart", "messageEnd"),
        ("toolStart", "toolEnd"),
    ];

    for file in FILES {
        let replay = replay(file);

        for (start, end) in pairs {
            assert_eq!(replay.count(start), replay.count(end), "{file}: {start} … {end}");
        }

        assert_eq!(replay.count("frameError") + replay.count("unknown"), 0, "{file}");
        assert!(!replay.mapper.is_working(), "{file}");
    }
}

#[test]
fn approvals_accepted_run_their_tools_and_name_the_written_file() {
    let project = tempfile::tempdir().unwrap();
    let replay = replay_in("02_turn_approvals_accept.jsonl", project.path());
    let events = replay.events.clone();

    assert_eq!(
        shape(&events),
        [
            "agentStart",
            "turnStart",
            "messageStart",
            "textDelta",
            "messageEnd",
            "toolStart",
            "toolEnd",
            "toolStart",
            "toolEnd",
            "toolStart",
            "toolEnd",
            "messageStart",
            "textDelta",
            "messageEnd",
            "turnEnd",
            "agentEnd",
            "promptResult",
            "settled",
        ]
    );

    let probe = "pwd && rg --files -g 'AGENTS.md' -g '!node_modules' -g '!vendor'";
    let root = project.path().to_str().unwrap();
    assert_eq!(
        tool_ends(&events),
        [
            ("shell".to_owned(), true, format!("{root}\n"), vec![]),
            (
                "apply_patch".to_owned(),
                false,
                format!("add {root}/hello.txt\nhi\n"),
                vec!["hello.txt".to_owned()]
            ),
            ("shell".to_owned(), false, "README.md\nhello.txt\n".to_owned(), vec![]),
        ]
    );

    // The card shows the command as it runs, Codex's reading of it as the hint.
    let first = events.iter().find_map(|event| match event {
        AgentEvent::ToolStart { name, args, intent, .. } => {
            Some((name.clone(), args["command"].clone(), intent.clone()))
        }
        _ => None,
    });
    assert_eq!(
        first,
        Some(("shell".to_owned(), PROBE_COMMAND.into(), Some(probe.to_owned())))
    );

    assert_eq!(
        replies(&events),
        [
            (
                "I’ll create the requested file, then list this directory.".to_owned(),
                Some("commentary".to_owned())
            ),
            (
                "Created `hello.txt`.\n\n`ls -1` output:\n```text\nREADME.md\nhello.txt\n```".to_owned(),
                Some("final_answer".to_owned())
            ),
        ]
    );
    assert_eq!(prompt_results(&events), [(PromptStatus::Completed, None)]);
}

#[test]
fn approvals_are_asks_and_a_file_change_names_its_files() {
    let replay = replay("02_turn_approvals_accept.jsonl");
    let hints = replay.hints.clone();

    let asks: Vec<_> = hints
        .iter()
        .filter_map(|hint| match hint {
            Hint::Ask(ask) => Some(ask.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(asks.len(), 3);

    let AskKind::Command(probe) = &asks[0].kind else {
        panic!("a command approval: {:?}", asks[0]);
    };
    assert_eq!(asks[0].request_id, RequestId::Number(0));
    assert_eq!(asks[0].item_id(), Some("exec-f61152e4-887b-4723-add5-193b11a1ad79"));
    assert_eq!(probe.shown_command(), Some(PROBE_COMMAND));
    assert_eq!(
        probe.reading(),
        Some("pwd && rg --files -g 'AGENTS.md' -g '!node_modules' -g '!vendor'")
    );
    assert_eq!(probe.cwd.as_deref(), Some("/work/project"));
    assert!(probe.offers("accept") && probe.offers("cancel") && probe.offers("acceptWithExecpolicyAmendment"));
    assert!(
        !probe.offers("acceptForSession"),
        "Codex 0.154 offers no session-wide accept here"
    );
    assert_eq!(probe.proposed_execpolicy_amendment, Some(vec!["pwd".to_owned()]));

    let AskKind::FileChange { approval, changes } = &asks[1].kind else {
        panic!("a file change approval: {:?}", asks[1]);
    };
    assert_eq!(approval.item_id, "exec-961f8aa0-fdd8-4add-8e4f-d524a5c9bcc4");
    let files: Vec<(&str, &str)> = changes
        .iter()
        .map(|change| (change.path.as_str(), change.kind.kind.as_str()))
        .collect();
    assert_eq!(files, [("/work/project/hello.txt", "add")]);

    let resolved: Vec<RequestId> = hints
        .iter()
        .filter_map(|hint| match hint {
            Hint::Resolved(id) => Some(id.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(resolved, [0, 1, 2].map(RequestId::Number));

    let waiting = hints
        .iter()
        .filter(|hint| matches!(hint, Hint::ThreadStatus(ThreadStatus::Active { active_flags }) if active_flags == &["waitingOnApproval"]))
        .count();
    assert_eq!(waiting, 3);
}

#[test]
fn a_declined_command_fails_its_card_and_the_turn_goes_on() {
    let events = replay("03_turn_decline.jsonl").events;

    assert_eq!(
        tool_ends(&events),
        [("shell".to_owned(), true, COMMAND_DECLINED.to_owned(), vec![])]
    );
    assert_eq!(replies(&events).last().unwrap().0, "Not allowed.");
    assert_eq!(prompt_results(&events), [(PromptStatus::Completed, None)]);
    assert_eq!(settles(&events), [Some(false)]);
}

#[test]
fn a_steer_is_a_second_user_message_of_the_running_turn() {
    let replay = replay("04_steer.jsonl");
    let events = replay.events.clone();

    let taken: Vec<UserMessage> = replay
        .hints
        .iter()
        .filter_map(|hint| match hint {
            Hint::UserMessage(message) => Some(message.clone()),
            _ => None,
        })
        .collect();
    let turn = "01a164a0-96a7-7243-8b57-70d95d87c471";

    assert_eq!(taken.len(), 2);
    assert!(taken.iter().all(|message| message.turn_id == turn));
    assert_eq!(taken[1].text, "Also mention the word STEERED in your answer.");
    assert_eq!(replay.count("agentStart"), 1);

    let output = events.iter().find_map(|event| match event {
        AgentEvent::ToolUpdate { partial, .. } => partial["content"][0]["text"].as_str().map(str::to_owned),
        _ => None,
    });
    assert_eq!(output.as_deref(), Some("done\n"));
    assert_eq!(replies(&events).last().unwrap().0, "STEERED — result: `done`");

    let ids: Vec<Option<String>> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::MessageEnd { message, .. } => Some(message.id.clone()),
            _ => None,
        })
        .collect();
    assert!(
        ids.iter().all(|id| id.as_deref() == Some(turn)),
        "a reply forks at its turn"
    );
}

#[test]
fn an_interrupted_turn_is_aborted_and_settles_as_such() {
    let replay = replay("05_interrupt.jsonl");
    let events = replay.events.clone();

    assert_eq!(prompt_results(&events), [(PromptStatus::Aborted, None)]);
    assert_eq!(settles(&events), [Some(true)]);
    assert_eq!(replies(&events).len(), 1, "the commentary ended before the interrupt");
}

#[test]
fn a_shell_command_and_a_compaction_run_as_quiet_turns() {
    let replay = replay("06_ops.jsonl");
    let events = replay.events.clone();

    // The command is no tool card: the engine answers the `!command` with its output.
    assert_eq!(shape(&events), ["compaction", "compaction", "promptResult"]);
    assert!(replay.hints.contains(&Hint::Shell(ShellRun {
        output: "from-shell\n".to_owned(),
        exit_code: Some(0),
    })));

    let compactions: Vec<(String, Option<String>)> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::Compaction { phase, reason, .. } => Some((phase.clone(), reason.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        compactions,
        [
            ("start".to_owned(), Some("manual".to_owned())),
            ("end".to_owned(), Some("manual".to_owned()))
        ]
    );
    assert_eq!(prompt_results(&events), [(PromptStatus::Handled, None)]);
    assert!(replay.hints.contains(&Hint::Title("S0 recording".to_owned())));
}

#[test]
fn token_counts_fill_the_statistics_and_the_context_meter() {
    let replay = replay("02_turn_approvals_accept.jsonl");
    let mut stats = CodexStats::default();

    for hint in &replay.hints {
        if let Hint::TokenUsage(usage) = hint {
            stats.on_token_usage(usage);
        }
    }

    assert_eq!(
        stats.tokens(),
        TokenUsage {
            input: 77847 - 56576,
            output: 383,
            cache_read: 56576,
            cache_write: 0,
            total: 78230,
        }
    );
    assert_eq!(
        stats.context(),
        Some(&ContextUsage {
            tokens: 19709,
            context_window: 258_400,
            percent: 19709.0 * 100.0 / 258_400.0,
        })
    );

    let session = stats.session_stats(MessageCounts::default());
    assert_eq!(session.cost_usd, None);
    assert_eq!(session.context_usage.map(|usage| usage.context_window), Some(258_400));
}

#[test]
fn a_followed_thread_drops_what_other_threads_report() {
    let replay = replay("07_fork_list.jsonl");
    let started = |hints: Vec<Hint>| {
        hints
            .iter()
            .filter(|hint| matches!(hint, Hint::ThreadStarted(_)))
            .count()
    };
    assert_eq!(started(replay.hints.clone()), 2);

    let mut followed = harness_business::codex_rpc::CodexMapper::new(std::path::Path::new(codex_recorded::PROJECT));
    followed.follow_thread(THREAD);
    let rows = codex_recorded::rows("07_fork_list.jsonl");
    let out = codex_recorded::replay_rows(&mut followed, rows, std::path::Path::new(codex_recorded::PROJECT));
    let hints: Vec<Hint> = out
        .into_iter()
        .filter_map(|mapped| match mapped {
            harness_business::codex_rpc::Mapped::Hint(hint) => Some(hint),
            harness_business::codex_rpc::Mapped::Event(_) => None,
        })
        .collect();

    assert_eq!(
        started(hints.clone()),
        0,
        "the forks' thread/started are about other threads"
    );
    assert!(
        hints.contains(&Hint::Deprecation(
            frame("07_fork_list.jsonl", codex_recorded::Dir::In, "deprecationNotice", 0)["params"]["summary"]
                .as_str()
                .unwrap()
                .to_owned()
        ))
    );
}
