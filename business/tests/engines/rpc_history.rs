//! Reading an OMP chat's history while the harness refuses to page: the session is busy, or it
//! changes between two pages. The real engine against scripted `fake_agent` replies.

use std::time::{Duration, Instant};

use crate::agent_support::{Run, start_omp};
use harness_business::BusinessError;
use harness_business::model::{AgentAction, AgentEvent, AgentReply, ChatBlock, NoticeLevel};
use pretty_assertions::assert_eq;

/// Longest a read may take while the session stays busy (the retry pauses add up to 750 ms).
const BUSY_BOUND: Duration = Duration::from_secs(3);

fn history(cursor: Option<&str>) -> AgentAction {
    AgentAction::GetHistory {
        cursor: cursor.map(str::to_owned),
        limit: 100,
    }
}

/// The text of every message of the reply, oldest first.
fn texts(reply: AgentReply) -> Vec<String> {
    let AgentReply::History { messages, .. } = reply else {
        panic!("history reply");
    };

    messages
        .iter()
        .flat_map(|message| &message.blocks)
        .filter_map(|block| match block {
            ChatBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

async fn read(run: &Run) -> Vec<String> {
    texts(run.manager.send(run.key(), history(None)).await.unwrap())
}

fn expected(count: usize) -> Vec<String> {
    (1..=count).map(|number| format!("m{number}")).collect()
}

fn notices(run: &Run) -> Vec<(NoticeLevel, String)> {
    run.events()
        .into_iter()
        .filter_map(|event| match event {
            AgentEvent::Notice { level, message } => Some((level, message)),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_busy_session_is_asked_again_for_the_whole_history() {
    let run = start_omp("omp_history_busy_once.jsonl").await;

    assert_eq!(read(&run).await, expected(4));
    assert!(notices(&run).is_empty());

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn a_cursor_invalidated_between_pages_restarts_on_the_new_snapshot() {
    let run = start_omp("omp_history_stale.jsonl").await;

    assert_eq!(read(&run).await, expected(5));
    assert!(notices(&run).is_empty());

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn pages_that_disagree_on_the_total_are_never_stitched() {
    let run = start_omp("omp_history_total_moves.jsonl").await;

    assert_eq!(read(&run).await, expected(5));

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn a_session_busy_for_good_is_read_whole_within_bounded_time() {
    let run = start_omp("omp_history_busy_whole.jsonl").await;
    let started = Instant::now();

    assert_eq!(read(&run).await, expected(3));

    assert!(started.elapsed() < BUSY_BOUND, "took {:?}", started.elapsed());
    assert!(notices(&run).is_empty());

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn an_unreadable_history_is_empty_with_a_notice_instead_of_a_failed_reload() {
    let run = start_omp("omp_history_unreadable.jsonl").await;
    let started = Instant::now();

    let reply = run.manager.send(run.key(), history(None)).await.unwrap();
    let AgentReply::History {
        messages,
        next_cursor,
        total,
    } = reply
    else {
        panic!("history reply");
    };

    assert_eq!((messages.len(), next_cursor, total), (0, None, None));
    assert!(started.elapsed() < BUSY_BOUND, "took {:?}", started.elapsed());

    run.wait_until("the notice", |events| {
        events.iter().any(|event| matches!(event, AgentEvent::Notice { .. }))
    })
    .await;

    let shown = notices(&run);
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].0, NoticeLevel::Warning);
    assert!(shown[0].1.contains("could not be read"));

    // An older page cannot be skipped over: asking for it fails and the chat keeps its cursor.
    let older = run.manager.send(run.key(), history(Some("3"))).await;
    assert!(matches!(older, Err(BusinessError::Protocol(_))), "{older:?}");

    run.manager.stop(run.key()).await.unwrap();
}
