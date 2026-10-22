//! The Claude chat's event mapping (`claude_stream::mapper::ClaudeMapper`) over the recorded
//! Claude Code runs (`fixtures/claude/stream`), replayed as the engine drives the mapper: turns,
//! replies, tool cards with touched paths, questions, steering and follow-ups, interrupts,
//! compaction, quiet `!command` output, controls, a missing login and a fork.

mod claude_mapper_support;
mod claude_recorded;

use claude_mapper_support::{ended, replay, replay_in, shape, streamed_text};
use claude_recorded::FILES;
use harness_business::claude_stream::mapper::{Ask, Hint, SIGN_IN_ERROR};
use harness_business::model::{AgentEvent, ChatBlock, MessageRole, NoticeLevel, PromptStatus, TokenUsage};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const TURN: [&str; 9] = [
    "agentStart",
    "turnStart",
    "messageStart",
    "textDelta",
    "messageEnd",
    "turnEnd",
    "agentEnd",
    "promptResult",
    "settled",
];

/// A turn whose first model call runs one tool, the second answers.
const TOOL_TURN: [&str; 14] = [
    "agentStart",
    "turnStart",
    "messageStart",
    "toolStart",
    "messageEnd",
    "toolEnd",
    "turnEnd",
    "turnStart",
    "messageStart",
    "textDelta",
    "messageEnd",
    "turnEnd",
    "agentEnd",
    "promptResult",
];

fn prompt_results(events: &[AgentEvent]) -> Vec<(PromptStatus, Option<String>)> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::PromptResult { status, error, .. } => Some((*status, error.clone())),
            _ => None,
        })
        .collect()
}

/// Each `toolEnd`: tool, error, result text, touched paths.
fn tool_ends(events: &[AgentEvent]) -> Vec<(String, bool, String, Vec<String>)> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolEnd {
                name,
                is_error,
                result,
                touched_paths,
                ..
            } => Some((
                name.clone(),
                *is_error,
                result["content"][0]["text"].as_str().unwrap_or_default().to_owned(),
                touched_paths.clone(),
            )),
            _ => None,
        })
        .collect()
}

fn text_of(blocks: &[ChatBlock]) -> String {
    blocks
        .iter()
        .filter_map(|block| match block {
            ChatBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn every_recorded_run_ends_what_it_starts_and_settles() {
    for file in FILES {
        let replay = replay(file);
        let starts = replay.count("agentStart");
        // queue2's first turn ends while its follow-up waits: one settle for both turns.
        let settles = starts - usize::from(file == "queue2.jsonl");

        assert!(starts > 0, "{file}");
        assert_eq!(replay.count("agentEnd"), starts, "{file}");
        assert_eq!(replay.count("settled"), settles, "{file}");
        assert_eq!(replay.count("messageStart"), replay.count("messageEnd"), "{file}");
        assert_eq!(replay.count("toolStart"), replay.count("toolEnd"), "{file}");
        assert_eq!(replay.count("turnStart"), replay.count("turnEnd"), "{file}");
        assert_eq!(replay.count("frameError") + replay.count("unknown"), 0, "{file}");
        assert!(!replay.mapper.is_working(), "{file}");
    }
}

#[test]
fn a_reply_streams_once_and_ends_with_its_entry_model_and_usage() {
    let replay = replay("basic.jsonl");
    let events = replay.events();

    assert_eq!(shape(&events), [TURN, TURN].concat());

    let replies = ended(&events, MessageRole::Assistant);
    assert_eq!(text_of(&replies[0].blocks), "pong");
    assert_eq!(streamed_text(&events, "msg_011CgGtZkHyqd28emDX9vnir"), "pong");
    assert_eq!(replies[0].id.as_deref(), Some("ec4a9c62-e37f-486b-b41c-db8af7eab42e"));
    assert_eq!(replies[0].model.as_deref(), Some("claude-opus-5-5"));
    assert_eq!(replies[0].stop_reason.as_deref(), Some("end_turn"));
    assert_eq!(
        replies[0].usage,
        Some(TokenUsage {
            input: 2,
            output: 4,
            cache_read: 12300,
            cache_write: 20776,
            total: 33082,
        })
    );
    assert!(replies[0].timestamp.is_some());

    let result = events
        .iter()
        .find(|event| matches!(event, AgentEvent::PromptResult { .. }));
    assert_eq!(
        result,
        Some(&AgentEvent::PromptResult {
            request_id: Some("c3ffc375-3f53-4214-acb4-665d7431545c".to_owned()),
            status: PromptStatus::Completed,
            error: None,
            retryable: false,
        })
    );
}

#[test]
fn tool_calls_show_their_input_result_and_touched_files() {
    let project = tempfile::tempdir().unwrap();
    let replay = replay_in("tools.jsonl", project.path());
    let events = replay.events();
    let root = project.path().to_str().unwrap();

    let starts: Vec<(String, Value, Option<String>)> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolStart { name, args, intent, .. } => Some((name.clone(), args.clone(), intent.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        starts,
        [
            (
                "Read".to_owned(),
                json!({ "file_path": format!("{root}/notes.txt") }),
                None
            ),
            (
                "Write".to_owned(),
                json!({ "file_path": format!("{root}/hello.txt"), "content": "hi" }),
                None
            ),
            (
                "Bash".to_owned(),
                json!({ "command": "ls", "description": "List files in current directory" }),
                Some("List files in current directory".to_owned())
            ),
        ]
    );

    let written = format!(
        "File created successfully at: {root}/hello.txt (file state is current in your context — no need to Read it back)"
    );
    assert_eq!(
        tool_ends(&events),
        [
            ("Read".to_owned(), false, "1\talpha\n2\tbeta\n3\t".to_owned(), vec![]),
            ("Write".to_owned(), false, written, vec!["hello.txt".to_owned()]),
            (
                "Bash".to_owned(),
                true,
                "The user declined this command.".to_owned(),
                vec![]
            ),
        ]
    );

    let replies = ended(&events, MessageRole::Assistant);
    assert_eq!(replies.len(), 4, "one message per model call");
    assert!(matches!(&replies[0].blocks[..], [ChatBlock::ToolCall { name, .. }] if name == "Read"));
    assert_eq!(replies[0].stop_reason.as_deref(), Some("tool_use"));
    assert!(matches!(
        &replies[3].blocks[0],
        ChatBlock::Thinking { text, redacted: true } if text.is_empty()
    ));
    assert_eq!(
        text_of(&replies[3].blocks),
        streamed_text(&events, "msg_011CgGtbnn6nGoqune2HVFau")
    );

    let hints = replay.hints();
    let callbacks = hints
        .iter()
        .filter(|hint| matches!(hint, Hint::HookCallback { callback, .. } if callback.callback_id == "askall"))
        .count();
    assert_eq!(callbacks, 3);

    let asked: Vec<(String, String)> = hints
        .iter()
        .filter_map(|hint| match hint {
            Hint::Ask(Ask::Permission(ask)) => Some((ask.tool.clone(), ask.tool_call_id.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        asked,
        [
            ("Read".to_owned(), "toolu_01ATYkU9KWcM1vnvdzgqt7b2".to_owned()),
            ("Write".to_owned(), "toolu_01Dd4KrNvLwodtS2N5msqyHf".to_owned()),
            ("Bash".to_owned(), "toolu_01GvHzCn8jKCqPG59ThCgW6N".to_owned()),
        ]
    );
}

#[test]
fn a_question_is_asked_as_data_and_its_answer_ends_the_card() {
    let replay = replay("question.jsonl");

    let question = replay
        .hints()
        .into_iter()
        .find_map(|hint| match hint {
            Hint::Ask(Ask::Question(question)) => Some(question),
            _ => None,
        })
        .unwrap();
    assert_eq!(question.request_id, "bbb7f33f-6e53-4681-ad89-6e1977429fc2");
    assert_eq!(question.tool_call_id, "toolu_01CG8yg5AeVPXGELiK1QnNzc");
    assert_eq!(question.questions.len(), 1);
    assert_eq!(question.questions[0].question, "Which colours do you like?");
    assert_eq!(question.questions[0].header, "Colours");
    assert!(question.questions[0].multi_select);

    let labels: Vec<&str> = question.questions[0]
        .options
        .iter()
        .map(|option| option.label.as_str())
        .collect();
    assert_eq!(labels, ["Red", "Green", "Blue"]);

    let ends = tool_ends(&replay.events());
    assert_eq!(ends.len(), 1);
    assert_eq!(ends[0].0, "AskUserQuestion");
    assert!(ends[0].2.contains("\"Red, Green\""));
}

#[test]
fn steering_joins_the_turn_and_a_follow_up_runs_as_its_own_before_the_chat_settles() {
    let replay = replay("queue2.jsonl");
    let events = replay.events();

    assert_eq!(shape(&events), [&TOOL_TURN[..], &TURN[..]].concat());

    let steer = "e8159cf5-373a-4231-bf75-ea8bda736346";
    let follow_up = "c1926d4d-68ba-4b89-bc92-3688f30d49a3";
    let removed = "482f1e5c-db56-4e62-8c67-aabc0f67df7e";
    let hints = replay.hints();
    let position = |wanted: &Hint| hints.iter().position(|hint| hint == wanted).unwrap();

    let taken = Hint::PromptsTaken(vec![
        "1ce282ed-e2e7-435b-a239-06cebea7fa1e".to_owned(),
        steer.to_owned(),
    ]);
    assert!(position(&Hint::PromptStarted(steer.to_owned())) < position(&taken));
    assert!(hints.contains(&Hint::PromptCancelled(removed.to_owned())));
    assert!(!hints.contains(&Hint::PromptStarted(removed.to_owned())));

    let turns: Vec<(usize, Vec<String>)> = hints
        .iter()
        .enumerate()
        .filter_map(|(position, hint)| match hint {
            Hint::TurnEnded(turn) => Some((position, turn.prompts.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(turns[0].1, ["1ce282ed-e2e7-435b-a239-06cebea7fa1e", steer]);
    assert_eq!(turns[1].1, [follow_up]);
    assert!(position(&Hint::PromptStarted(follow_up.to_owned())) > turns[0].0);

    let replies = ended(&events, MessageRole::Assistant);
    assert!(text_of(&replies[1].blocks).ends_with("BANANA"));
    assert_eq!(replay.count("subagent"), 0, "a shell task is not a subagent");
}

#[test]
fn an_interrupt_aborts_the_turn_and_the_next_prompt_runs_normally() {
    let replay = replay("interrupt.jsonl");
    let events = replay.events();

    assert_eq!(
        prompt_results(&events),
        [(PromptStatus::Aborted, None), (PromptStatus::Completed, None)]
    );

    let settles: Vec<Option<bool>> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::Settled { aborted } => Some(*aborted),
            _ => None,
        })
        .collect();
    assert_eq!(settles, [Some(true), Some(false)]);

    let replies = ended(&events, MessageRole::Assistant);
    assert_eq!(replies[0].stop_reason.as_deref(), Some("aborted"));
    assert_eq!(
        text_of(&replies[0].blocks),
        streamed_text(&events, "msg_011CgGtkfS6EzGG4ZbYdy4qZ")
    );
    assert!(text_of(&replies[0].blocks).starts_with("**The Last Keeper of Gannet Rock**"));
    assert_eq!(replay.count("notice"), 0, "the interruption marker is not shown");
    assert!(ended(&events, MessageRole::User).is_empty());
}

#[test]
fn a_compaction_is_a_quiet_run_with_its_own_banner() {
    let replay = replay("compact.jsonl");
    let events = replay.events();

    let compaction = |phase: &str| AgentEvent::Compaction {
        phase: phase.to_owned(),
        reason: Some("manual".to_owned()),
        error: None,
    };
    assert_eq!(
        events[..3],
        [
            compaction("start"),
            compaction("end"),
            AgentEvent::PromptResult {
                request_id: Some("f9fa9cdb-22a8-48f8-908e-5fb1d26500e4".to_owned()),
                status: PromptStatus::Handled,
                error: None,
                retryable: false,
            },
        ]
    );
    assert_eq!(shape(&events[3..]), TURN);
    assert_eq!(replay.count("notice"), 0, "\"Compacted\" repeats the banner");

    let compacted = replay.hints().into_iter().find_map(|hint| match hint {
        Hint::Compacted(metadata) => Some((metadata.pre_tokens, metadata.post_tokens)),
        _ => None,
    });
    assert_eq!(compacted, Some((Some(33905), Some(10129))));
}

#[test]
fn a_quiet_command_output_shows_nothing_and_the_typed_bang_is_a_bash_call() {
    let replay = replay("bash.jsonl");
    let events = replay.events();

    assert_eq!(shape(&events), [&TOOL_TURN[..], &["settled"], &TURN[..]].concat());

    let quiet = replay.hints().into_iter().find_map(|hint| match hint {
        Hint::TurnEnded(turn) if turn.quiet => Some(turn.prompts),
        _ => None,
    });
    assert_eq!(quiet, Some(vec!["27aa876f-0042-44e4-95b6-10b6c7acc078".to_owned()]));

    let replies = ended(&events, MessageRole::Assistant);
    assert!(matches!(
        &replies[0].blocks[..],
        [ChatBlock::Thinking { redacted: true, .. }, ChatBlock::ToolCall { name, intent: Some(intent), .. }]
            if name == "Bash" && intent == "Print a test string"
    ));
    assert_eq!(tool_ends(&events)[0].2, "typed-bang");
    assert_eq!(text_of(&replies[2].blocks), "injected-output-42");
}

#[test]
fn controls_report_the_model_notice_mode_title_and_commands() {
    let replay = replay("controls.jsonl");

    let notices: Vec<AgentEvent> = replay
        .events()
        .into_iter()
        .filter(|event| matches!(event, AgentEvent::Notice { .. }))
        .collect();
    assert_eq!(
        notices,
        [AgentEvent::Notice {
            level: NoticeLevel::Info,
            message: "Set model to `sonnet (claude-sonnet-5-5)`".to_owned(),
        }]
    );

    let hints = replay.hints();
    assert!(hints.contains(&Hint::PermissionMode("acceptEdits".to_owned())));
    assert!(hints.contains(&Hint::Title("Recorded rename".to_owned())));
    assert!(
        hints
            .iter()
            .any(|hint| matches!(hint, Hint::Commands(commands) if commands.len() == 3))
    );

    let models: Vec<String> = hints
        .iter()
        .filter_map(|hint| match hint {
            Hint::Init(init) => Some(init.model.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(models, ["claude-opus-5-5", "claude-sonnet-5-5"]);
}

#[test]
fn no_login_fails_the_prompt_with_the_sign_in_hint() {
    let replay = replay("nologin.jsonl");
    let events = replay.events();

    assert_eq!(shape(&events), ["agentStart", "agentEnd", "promptResult", "settled"]);
    assert_eq!(
        prompt_results(&events),
        [(PromptStatus::Error, Some(SIGN_IN_ERROR.to_owned()))]
    );
    assert!(matches!(events[2], AgentEvent::PromptResult { retryable: false, .. }));
}

#[test]
fn a_fork_reports_its_new_session_id_first() {
    let replay = replay("fork_at.jsonl");
    let hints = replay.hints();
    let id = "44b8b4df-227b-4f61-98d1-c42cf465a870";

    assert_eq!(hints[0], Hint::SessionId(id.to_owned()));
    assert_eq!(replay.mapper.session_id(), Some(id));
    assert_eq!(
        hints.iter().filter(|hint| matches!(hint, Hint::SessionId(_))).count(),
        1
    );

    let replies = ended(&replay.events(), MessageRole::Assistant);
    assert!(matches!(
        &replies[0].blocks[0],
        ChatBlock::Thinking { redacted: true, .. }
    ));
    assert!(text_of(&replies[0].blocks).contains("forked"));
}
