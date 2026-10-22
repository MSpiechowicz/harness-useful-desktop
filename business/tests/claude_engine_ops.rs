//! Session actions of the Claude Code chat engine against the scripted `claude`: a new chat names
//! its session, new / switch / fork / clone start the CLI again on one event stream, a fork keeps
//! the turn it starts from and is refused before the last compaction, renaming, compaction with
//! instructions, the statistics and context meter, the history, the export and the last reply.

mod agent_support;
mod claude_engine_support;

use std::time::Duration;

use claude_engine_support::{BASIC_SESSION, Chat, Setup, argv, flag, kinds, prompt, stop_cleanly};
use harness_business::model::{
    AgentAction, AgentEvent, AgentReply, ChatMessage, HarnessId, IpcErrorCode, MessageRole, SessionRef, TokenUsage,
};
use pretty_assertions::assert_eq;

/// The session of queue2.jsonl: a turn with a Bash call and a steer, then a follow-up turn.
const QUEUE_SESSION: &str = "e52c9fea-232a-48d5-a135-a564fe56012e";

/// Its first prompt and the last line of that turn (the recorded fork point, fork_at.jsonl).
const FIRST_PROMPT: &str = "1ce282ed-e2e7-435b-a239-06cebea7fa1e";
const FIRST_TURN_END: &str = "d74da2e9-e450-44c3-9301-09138b4d1162";

/// The fork's and the clone's own ids, which their first frames name.
const FORK_SESSION: &str = "44b8b4df-227b-4f61-98d1-c42cf465a870";
const CLONE_SESSION: &str = "acf5e683-2293-45ef-a9ac-d3016775fdbc";

/// In BASIC_SESSION's file: a prompt before its compaction and one after it, with its reply.
const BEFORE_COMPACTION: &str = "c3ffc375-3f53-4214-acb4-665d7431545c";
const AFTER_COMPACTION: &str = "00c001c2-18bd-4e78-a1db-d7d85c85cbb8";
const AFTER_COMPACTION_REPLY: &str = "18a72c51-6ddc-42c1-9748-203c3b92fb3e";

fn claude(id: &str) -> SessionRef {
    SessionRef {
        harness: HarnessId::Claude,
        id: id.to_owned(),
    }
}

/// Waits until `count` events match `predicate`.
async fn wait_count(chat: &Chat, what: &str, count: usize, predicate: impl Fn(&AgentEvent) -> bool) -> Vec<AgentEvent> {
    let deadline = tokio::time::Instant::now() + agent_support::WAIT;

    loop {
        let events = chat.events();
        if events.iter().filter(|event| predicate(event)).count() >= count {
            return events;
        }

        assert!(
            tokio::time::Instant::now() < deadline,
            "never {count} {what}: {events:#?}"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn is_ready(event: &AgentEvent) -> bool {
    matches!(event, AgentEvent::Ready { .. })
}

fn session_of(event: &AgentEvent) -> Option<&str> {
    match event {
        AgentEvent::State { state } => Some(&state.session_id),
        _ => None,
    }
}

async fn history(chat: &Chat) -> Vec<ChatMessage> {
    let reply = chat
        .send(AgentAction::GetHistory {
            cursor: None,
            limit: 200,
        })
        .await;

    match reply {
        AgentReply::History { messages, .. } => messages,
        other => panic!("not a history: {other:?}"),
    }
}

/// Every message id of `messages`.
fn ids(messages: &[ChatMessage]) -> Vec<&str> {
    messages.iter().filter_map(|message| message.id.as_deref()).collect()
}

#[tokio::test]
async fn a_new_chat_starts_the_cli_again_with_a_fresh_session_on_one_event_stream() {
    let chat = Setup::new(&["claude_init.jsonl", "claude_init.jsonl"]).start().await;
    let first = flag(&argv(&chat, 0), "--session-id").expect("named").to_owned();

    assert_eq!(chat.send(AgentAction::NewSession).await, AgentReply::Ack);

    let events = wait_count(&chat, "ready events", 2, is_ready).await;
    let second = flag(&argv(&chat, 1), "--session-id").expect("named").to_owned();
    assert_ne!(first, second, "a fresh session");
    assert!(
        events.iter().any(|event| session_of(event) == Some(second.as_str())),
        "the chat reports the new session"
    );

    let AgentReply::State { state } = chat.send(AgentAction::GetState).await else {
        panic!("a state")
    };
    assert_eq!(state.session_id, second);

    let envelopes = chat.sink.envelopes();
    assert!(
        envelopes.windows(2).all(|pair| pair[1].seq == pair[0].seq + 1),
        "one numbered stream"
    );
    assert!(
        !events.iter().any(|event| matches!(event, AgentEvent::Exited { .. })),
        "the old process's exit never reaches the chat"
    );

    stop_cleanly(&chat).await;
    assert_eq!(chat.spawner.exited(), 2, "both processes ended");
    let exits = chat
        .events()
        .iter()
        .filter(|event| matches!(event, AgentEvent::Exited { .. }))
        .count();
    assert_eq!(exits, 1);
}

#[tokio::test]
async fn a_move_is_refused_while_a_turn_runs() {
    let chat = Setup::new(&["claude_busy.jsonl"]).start().await;
    chat.send(prompt("work")).await;

    for action in [AgentAction::NewSession, AgentAction::Clone] {
        let refused = chat.session().send(action).await.expect_err("busy");
        assert_eq!(refused.code(), IpcErrorCode::Unsupported);
    }

    assert_eq!(chat.spawner.requested().len(), 1);
    chat.stop().await;
}

#[tokio::test]
async fn a_stop_gives_a_move_under_way_up_and_the_chat_exits_once() {
    let chat = std::sync::Arc::new(Setup::new(&["claude_init.jsonl", "claude_hang.jsonl"]).start().await);

    let moving = {
        let chat = chat.clone();
        tokio::spawn(async move { chat.session().send(AgentAction::NewSession).await })
    };

    while chat.spawner.requested().len() < 2 {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let events = chat.stop().await;
    assert!(moving.await.expect("joined").is_err(), "the move gave up");

    let exits = events
        .iter()
        .filter(|event| matches!(event, AgentEvent::Exited { .. }))
        .count();
    assert_eq!(exits, 1);
    assert!(matches!(events.last(), Some(AgentEvent::Exited { .. })));
    assert_eq!(
        wait_count(&chat, "ready events", 1, is_ready)
            .await
            .iter()
            .filter(|event| is_ready(event))
            .count(),
        1
    );
}

#[tokio::test]
async fn a_switch_resumes_another_saved_session_of_the_project() {
    let chat = Setup {
        sessions: &[BASIC_SESSION],
        ..Setup::new(&["claude_init.jsonl", "claude_init.jsonl"])
    }
    .start()
    .await;

    let missing = chat
        .session()
        .send(AgentAction::SwitchSession {
            session: claude(QUEUE_SESSION),
        })
        .await
        .expect_err("no such file");
    assert_eq!(missing.code(), IpcErrorCode::NotFound);

    let reply = chat
        .send(AgentAction::SwitchSession {
            session: claude(BASIC_SESSION),
        })
        .await;
    assert_eq!(reply, AgentReply::Ack);

    let args = argv(&chat, 1);
    assert_eq!(flag(&args, "--resume"), Some(BASIC_SESSION));
    assert_eq!(flag(&args, "--session-id"), None);

    let AgentReply::State { state } = chat.send(AgentAction::GetState).await else {
        panic!("a state")
    };
    assert_eq!(state.session_id, BASIC_SESSION);
    assert!(state.message_count > 0, "the saved messages count");
    assert!(!history(&chat).await.is_empty(), "the resumed history");

    stop_cleanly(&chat).await;
}

#[tokio::test]
async fn a_fork_keeps_the_turn_of_its_message_and_runs_on_the_forked_history() {
    let chat = Setup {
        resume: Some(QUEUE_SESSION),
        sessions: &[QUEUE_SESSION],
        ..Setup::new(&["claude_init.jsonl", "claude_fork_start.jsonl"])
    }
    .start()
    .await;

    let reply = chat
        .send(AgentAction::Fork {
            entry_id: FIRST_PROMPT.to_owned(),
        })
        .await;
    assert_eq!(reply, AgentReply::Ack);

    let args = argv(&chat, 1);
    assert_eq!(flag(&args, "--resume"), Some(QUEUE_SESSION));
    assert!(args.iter().any(|arg| arg == "--fork-session"));
    assert_eq!(flag(&args, "--resume-session-at"), Some(FIRST_TURN_END));
    let named = flag(&args, "--session-id").expect("the fork's id").to_owned();
    assert_ne!(named, QUEUE_SESSION);

    // The fork's file is not written yet: the history is what it copied, up to the turn's end.
    let copied = history(&chat).await;
    assert_eq!(ids(&copied).last(), Some(&FIRST_TURN_END));
    assert_eq!(copied[0].role, MessageRole::User);

    chat.wait("the fork's own id", |event| session_of(event) == Some(FORK_SESSION))
        .await;

    chat.send(prompt("What was my last question? Then reply: forked")).await;
    chat.settled(1).await;

    stop_cleanly(&chat).await;
}

#[tokio::test]
async fn a_fork_before_the_last_compaction_or_off_the_path_is_refused() {
    let chat = Setup {
        resume: Some(BASIC_SESSION),
        sessions: &[BASIC_SESSION],
        ..Setup::new(&["claude_init.jsonl"])
    }
    .start()
    .await;

    let fork = |entry: &str| AgentAction::Fork {
        entry_id: entry.to_owned(),
    };

    let before = chat
        .session()
        .send(fork(BEFORE_COMPACTION))
        .await
        .expect_err("compacted");
    assert_eq!(before.code(), IpcErrorCode::InvalidInput);
    assert!(before.to_string().contains("last compaction"), "{before}");

    let unknown = chat.session().send(fork("not-a-message")).await.expect_err("unknown");
    assert_eq!(unknown.code(), IpcErrorCode::NotFound);

    assert_eq!(chat.spawner.requested().len(), 1, "nothing started");
    chat.stop().await;
}

#[tokio::test]
async fn a_fork_after_the_compaction_resumes_at_its_reply() {
    let chat = Setup {
        resume: Some(BASIC_SESSION),
        sessions: &[BASIC_SESSION],
        ..Setup::new(&["claude_init.jsonl", "claude_init.jsonl"])
    }
    .start()
    .await;

    chat.send(AgentAction::Fork {
        entry_id: AFTER_COMPACTION.to_owned(),
    })
    .await;

    assert_eq!(
        flag(&argv(&chat, 1), "--resume-session-at"),
        Some(AFTER_COMPACTION_REPLY)
    );

    stop_cleanly(&chat).await;
}

#[tokio::test]
async fn a_clone_copies_every_message_without_a_cut() {
    let chat = Setup {
        resume: Some(BASIC_SESSION),
        sessions: &[BASIC_SESSION],
        ..Setup::new(&["claude_init.jsonl", "claude_clone_start.jsonl"])
    }
    .start()
    .await;

    assert_eq!(chat.send(AgentAction::Clone).await, AgentReply::Ack);

    let args = argv(&chat, 1);
    assert_eq!(flag(&args, "--resume"), Some(BASIC_SESSION));
    assert!(args.iter().any(|arg| arg == "--fork-session"));
    assert_eq!(flag(&args, "--resume-session-at"), None);
    assert!(flag(&args, "--session-id").is_some());

    chat.wait("the clone's own id", |event| session_of(event) == Some(CLONE_SESSION))
        .await;
    chat.send(prompt("Reply with exactly: cloned")).await;
    chat.settled(1).await;

    let AgentReply::Text { text } = chat.send(AgentAction::GetLastAssistantText).await else {
        panic!("a text")
    };
    assert_eq!(text.as_deref(), Some("cloned"));

    stop_cleanly(&chat).await;
}

#[tokio::test]
async fn a_new_chat_has_nothing_saved_to_fork_or_clone_yet() {
    let chat = Setup::new(&["claude_init.jsonl"]).start().await;

    let refused = chat.session().send(AgentAction::Clone).await.expect_err("unsaved");
    assert_eq!(refused.code(), IpcErrorCode::Unsupported);
    assert_eq!(history(&chat).await, Vec::new());

    chat.stop().await;
}

#[tokio::test]
async fn renaming_sends_rename_session_and_takes_the_title_claude_reports() {
    let chat = Setup::new(&["claude_rename.jsonl"]).start().await;

    let reply = chat
        .send(AgentAction::SetSessionName {
            name: "  Recorded rename ".to_owned(),
        })
        .await;
    assert_eq!(reply, AgentReply::Ack);

    let AgentReply::State { state } = chat.send(AgentAction::GetState).await else {
        panic!("a state")
    };
    assert_eq!(state.session_name.as_deref(), Some("Recorded rename"));

    stop_cleanly(&chat).await;
}

#[tokio::test]
async fn compaction_sends_compact_with_the_instructions_and_shows_it_quietly() {
    let chat = Setup::new(&["claude_compact.jsonl"]).start().await;

    let reply = chat
        .send(AgentAction::Compact {
            instructions: Some(" keep only the words pong and ping ".to_owned()),
        })
        .await;
    assert_eq!(reply, AgentReply::Ack);

    let events = wait_count(
        &chat,
        "compaction ends",
        1,
        |event| matches!(event, AgentEvent::Compaction { phase, .. } if phase == "end"),
    )
    .await;
    assert!(!kinds(&events).contains(&"agentStart".to_owned()), "no model turn");

    chat.wait("the state after the quiet run", |event| {
        matches!(event, AgentEvent::State { state } if state.session_id == BASIC_SESSION && !state.is_compacting)
    })
    .await;

    stop_cleanly(&chat).await;
}

#[tokio::test]
async fn statistics_add_the_result_totals_the_counts_and_the_context_usage() {
    let chat = Setup::new(&["claude_stats.jsonl"]).start().await;
    chat.send(prompt("Reply with exactly the word: pong")).await;
    chat.settled(1).await;

    let AgentReply::Stats { stats } = chat.send(AgentAction::GetSessionStats).await else {
        panic!("statistics")
    };
    assert_eq!(
        stats.tokens,
        TokenUsage {
            input: 2,
            output: 4,
            cache_read: 12_300,
            cache_write: 20_776,
            total: 33_082,
        }
    );
    assert_eq!(stats.cost_usd, Some(0.168_756));
    assert_eq!((stats.user_messages, stats.assistant_messages), (1, 1));

    let context = stats.context_usage.expect("a context meter");
    assert_eq!((context.tokens, context.context_window), (33_500, 1_000_000));

    let AgentReply::State { state } = chat.send(AgentAction::GetState).await else {
        panic!("a state")
    };
    assert_eq!(state.message_count, 2);
    assert_eq!(state.context_usage.map(|usage| usage.tokens), Some(33_500));

    stop_cleanly(&chat).await;
}

#[tokio::test]
async fn a_resumed_chat_reads_its_history_last_reply_and_export_from_the_session_file() {
    let exports = tempfile::tempdir().expect("export folder");
    let mut options = agent_support::options();
    options.export_dir = Some(exports.path().to_path_buf());

    let chat = Setup {
        resume: Some(BASIC_SESSION),
        sessions: &[BASIC_SESSION],
        options,
        ..Setup::new(&["claude_init.jsonl"])
    }
    .start()
    .await;

    let messages = history(&chat).await;
    assert_eq!(ids(&messages).last(), Some(&AFTER_COMPACTION_REPLY));

    let page = chat.send(AgentAction::GetHistory { cursor: None, limit: 2 }).await;
    let AgentReply::History {
        messages: newest,
        next_cursor,
        total,
    } = page
    else {
        panic!("a page")
    };
    assert_eq!(newest.len(), 2);
    assert!(next_cursor.is_some());
    assert_eq!(total, Some(u32::try_from(messages.len()).unwrap()));

    let AgentReply::Text { text } = chat.send(AgentAction::GetLastAssistantText).await else {
        panic!("a text")
    };
    assert_eq!(text.as_deref(), Some("after-compact"));

    let AgentReply::Exported { path } = chat.send(AgentAction::ExportHtml).await else {
        panic!("an export")
    };
    assert_eq!(
        std::path::Path::new(&path),
        exports.path().join(format!("claude-{BASIC_SESSION}.html"))
    );
    let html = std::fs::read_to_string(&path).expect("the export");
    assert!(html.contains("after-compact") && html.contains("Claude Code"));

    let AgentReply::State { state } = chat.send(AgentAction::GetState).await else {
        panic!("a state")
    };
    assert_eq!(state.message_count, u32::try_from(messages.len()).unwrap());

    stop_cleanly(&chat).await;
}
