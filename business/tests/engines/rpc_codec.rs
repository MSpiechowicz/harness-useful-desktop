//! Framing of the agent RPC streams and delta coalescing.

use std::time::Duration;

use harness_business::agent::DeltaCoalescer;
use harness_business::model::AgentEvent;
use harness_business::pi_family::rpc::{Frame, FrameReader, LineSplitter, encode_frame, parse_frame};
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::time::Instant;

async fn frames(bytes: &[u8], limit: usize) -> Vec<Frame> {
    let mut reader = FrameReader::with_limit(bytes, limit);
    let mut frames = Vec::new();

    while let Some(frame) = reader.next_frame().await {
        frames.push(frame);
    }

    frames
}

fn fault_error(frame: &Frame) -> &str {
    match frame {
        Frame::Fault(fault) => &fault.error,
        Frame::Json(value) => panic!("expected a fault, got {value}"),
    }
}

#[tokio::test]
async fn splits_on_newline_only_and_keeps_unicode_separators_inside_strings() {
    let input = "{\"text\":\"a\u{2028}b\u{2029}c\"}\n{\"n\":2}\r\n\n   \n{\"n\":3}".as_bytes();

    let parsed = frames(input, 1024).await;

    assert_eq!(
        parsed,
        [
            Frame::Json(json!({ "text": "a\u{2028}b\u{2029}c" })),
            Frame::Json(json!({ "n": 2 })),
            Frame::Json(json!({ "n": 3 })),
        ]
    );
}

#[tokio::test]
async fn invalid_and_non_object_frames_are_faults_with_their_type_when_readable() {
    let input = b"{\"type\":\"message_end\",\"oops\n[1]\n\"text\"\n{\"ok\":true}\n";

    let parsed = frames(input, 1024).await;

    assert_eq!(parsed.len(), 4);
    let Frame::Fault(first) = &parsed[0] else {
        panic!("invalid JSON is a fault");
    };
    assert_eq!(first.original_type.as_deref(), Some("message_end"));
    assert!(first.error.starts_with("invalid JSON at column"));
    assert_eq!(fault_error(&parsed[1]), "frame is not a JSON object");
    assert_eq!(fault_error(&parsed[2]), "frame is not a JSON object");
    assert_eq!(parsed[3], Frame::Json(json!({ "ok": true })));
}

#[tokio::test]
async fn an_oversized_frame_is_dropped_and_reading_resumes_at_the_next_newline() {
    let mut input = br#"{"type":"response","id":"req-7","pad":""#.to_vec();
    input.extend(std::iter::repeat_n(b'x', 5000));
    input.extend_from_slice(b"\"}\n{\"after\":1}\n");

    let parsed = frames(&input, 256).await;

    let Frame::Fault(fault) = &parsed[0] else {
        panic!("oversized frame is a fault");
    };
    assert_eq!(fault.original_type.as_deref(), Some("response"));
    assert_eq!(fault.response_id.as_deref(), Some("req-7"));
    assert!(fault.error.contains("4 MiB"));
    assert_eq!(parsed[1], Frame::Json(json!({ "after": 1 })));
}

#[tokio::test]
async fn an_oversized_frame_keeps_its_whole_numeric_id_or_none() {
    let pad = "x".repeat(200);
    let input = format!(
        "{{\"id\":12345678901234567890,\"result\":\"{pad}\"}}\n{{\"id\": 42 , \"method\":\"m\",\"params\":\"{pad}\"}}\n{{\"id\":{},\"result\":\"{pad}\"}}\n",
        "9".repeat(40)
    );

    let ids: Vec<Option<String>> = frames(input.as_bytes(), 100)
        .await
        .iter()
        .map(|frame| match frame {
            Frame::Fault(fault) => fault.rpc_id.clone(),
            Frame::Json(value) => panic!("expected a fault, got {value}"),
        })
        .collect();

    assert_eq!(
        ids,
        [Some("12345678901234567890".to_owned()), Some("42".to_owned()), None],
        "never the first digits of a longer number"
    );
}

#[tokio::test]
async fn an_oversized_final_frame_without_newline_is_still_reported() {
    let input = vec![b'x'; 600];

    let parsed = frames(&input, 256).await;

    assert_eq!(parsed.len(), 1);
    assert!(fault_error(&parsed[0]).contains("limit"));
}

#[tokio::test]
async fn line_splitter_truncates_long_lines() {
    let mut splitter = LineSplitter::new(&b"abcdefgh\nij\r\n"[..], 4);

    let first = splitter.next_line().await.unwrap();
    assert_eq!((first.bytes.as_slice(), first.overflowed), (&b"abcd"[..], true));
    let second = splitter.next_line().await.unwrap();
    assert_eq!((second.bytes.as_slice(), second.overflowed), (&b"ij"[..], false));
    assert!(splitter.next_line().await.is_none());
}

#[test]
fn encoding_is_one_json_line_with_escaped_newlines() {
    let frame = json!({ "type": "prompt", "message": "two\nlines\u{2028}" });

    let encoded = encode_frame(&frame);

    assert_eq!(encoded.iter().filter(|byte| **byte == b'\n').count(), 1);
    assert_eq!(encoded.last(), Some(&b'\n'));
    assert_eq!(parse_frame(&encoded[..encoded.len() - 1]), Frame::Json(frame));
}

fn text(message: &str, index: u32, delta: &str) -> AgentEvent {
    AgentEvent::TextDelta {
        message_id: message.to_owned(),
        content_index: index,
        delta: delta.to_owned(),
    }
}

#[test]
fn consecutive_deltas_of_one_block_merge_within_the_window() {
    let mut coalescer = DeltaCoalescer::new(Duration::from_millis(16));
    let start = Instant::now();

    assert!(coalescer.push(text("m", 0, "a"), start).is_empty());
    assert!(
        coalescer
            .push(text("m", 0, "b"), start + Duration::from_millis(5))
            .is_empty()
    );
    assert_eq!(coalescer.deadline(), Some(start + Duration::from_millis(16)));

    // Another block flushes the merged delta first.
    let flushed = coalescer.push(text("m", 1, "c"), start + Duration::from_millis(6));
    assert_eq!(flushed, [text("m", 0, "ab")]);

    // Past the window a new delta starts a new batch.
    let late = coalescer.push(text("m", 1, "d"), start + Duration::from_millis(30));
    assert_eq!(late, [text("m", 1, "c")]);

    // Any other event flushes and keeps the order.
    let ordered = coalescer.push(AgentEvent::TurnEnd, start + Duration::from_millis(31));
    assert_eq!(ordered, [text("m", 1, "d"), AgentEvent::TurnEnd]);
    assert_eq!(coalescer.deadline(), None);
}

#[test]
fn thinking_and_text_deltas_never_merge_and_a_zero_window_passes_everything_through() {
    let mut coalescer = DeltaCoalescer::new(Duration::from_millis(16));
    let now = Instant::now();
    let thinking = AgentEvent::ThinkingDelta {
        message_id: "m".to_owned(),
        content_index: 0,
        delta: "t".to_owned(),
    };

    coalescer.push(thinking.clone(), now);
    assert_eq!(coalescer.push(text("m", 0, "x"), now), [thinking]);
    assert_eq!(coalescer.flush(), [text("m", 0, "x")]);

    let mut passthrough = DeltaCoalescer::new(Duration::ZERO);
    assert_eq!(passthrough.push(text("m", 0, "x"), now), [text("m", 0, "x")]);
}
