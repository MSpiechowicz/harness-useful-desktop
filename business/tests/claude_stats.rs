//! Session statistics and the context meter of a Claude chat (`claude_stream::stats`), fed with
//! the recorded runs' results, compactions and `get_context_usage` replies as the engine would.

mod claude_mapper_support;
mod claude_recorded;

use claude_mapper_support::replay;
use harness_business::claude_stream::mapper::Hint;
use harness_business::claude_stream::stats::{ClaudeStats, MessageCounts};
use harness_business::claude_stream::wire::control::ContextUsageReply;
use harness_business::model::{ContextUsage, TokenUsage};
use pretty_assertions::assert_eq;

/// The statistics after replaying `file`, the session's model from `system/init`; with
/// `ask_context` the `get_context_usage` replies count too.
fn stats_of(file: &str, ask_context: bool) -> ClaudeStats {
    let mut stats = ClaudeStats::default();
    let mut model: Option<String> = None;

    for hint in replay(file).hints() {
        match hint {
            Hint::Init(init) => model = Some(init.model.clone()),
            Hint::TurnEnded(turn) => stats.on_result(&turn.result, model.as_deref()),
            Hint::Compacted(metadata) => stats.on_compacted(&metadata),
            Hint::Reply(reply) if ask_context => {
                if let Some(context) = reply
                    .parse::<ContextUsageReply>()
                    .filter(|context| context.max_tokens > 0)
                {
                    stats.on_context_usage(&context);
                }
            }
            _ => {}
        }
    }

    stats
}

#[test]
fn totals_are_the_process_totals_of_the_newest_result() {
    let stats = stats_of("basic.jsonl", false);

    assert_eq!(
        stats.tokens(),
        TokenUsage {
            input: 4,
            output: 8,
            cache_read: 27090,
            cache_write: 39885,
            total: 4 + 8 + 27090 + 39885,
        }
    );
    assert_eq!(stats.cost_usd(), Some(0.324674));
    assert_eq!(
        stats.context(),
        Some(&ContextUsage {
            tokens: 2 + 4 + 14790 + 19109,
            context_window: 1_000_000,
            percent: 3.3905,
        })
    );
}

#[test]
fn the_context_reply_sets_the_meter() {
    let mut stats = ClaudeStats::default();
    let reply = ContextUsageReply {
        total_tokens: 33078,
        max_tokens: 1_000_000,
        ..ContextUsageReply::default()
    };
    stats.on_context_usage(&reply);

    let context = stats.context().unwrap();
    assert_eq!((context.tokens, context.context_window), (33078, 1_000_000));
    assert!((context.percent - 3.3078).abs() < 1e-9);

    assert_eq!(
        stats_of("basic.jsonl", true).context().unwrap().context_window,
        1_000_000
    );
}

#[test]
fn an_interrupted_turn_changes_nothing() {
    let stats = stats_of("interrupt.jsonl", false);

    assert_eq!(stats.cost_usd(), Some(0.043269600000000005));
    assert_eq!(stats.tokens().input, 2);
}

#[test]
fn a_compaction_sets_the_meter_to_what_it_left() {
    let stats = stats_of("compact.jsonl", false);

    assert_eq!(stats.tokens().input, 2841, "compaction calls count");
    assert_eq!(stats.context().unwrap().tokens, 2 + 6 + 14122 + 19584);

    let mut compacted = ClaudeStats::default();
    compacted.on_context_usage(&ContextUsageReply {
        total_tokens: 33905,
        max_tokens: 1_000_000,
        ..ContextUsageReply::default()
    });
    compacted.on_compacted(&harness_business::claude_stream::wire::system::CompactMetadata {
        trigger: Some("manual".to_owned()),
        pre_tokens: Some(33905),
        post_tokens: Some(10129),
    });
    assert_eq!(compacted.context().unwrap().tokens, 10129);
}

#[test]
fn session_stats_combine_the_counts_with_the_totals() {
    let stats = stats_of("tools.jsonl", false);
    let counts = MessageCounts {
        user: 1,
        assistant: 4,
        tool_calls: 3,
        total: 8,
    };

    let session = stats.session_stats(counts);
    assert_eq!(
        (
            session.user_messages,
            session.assistant_messages,
            session.tool_calls,
            session.total_messages
        ),
        (1, 4, 3, 8)
    );
    assert_eq!(session.cost_usd, Some(0.1933646));
    assert_eq!(session.tokens.output, 472);
    assert!(session.context_usage.is_some());
}
