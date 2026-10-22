//! The `opencode serve` event stream (`GET /global/event`) against the recorded stream and crafted
//! ones.

use crate::opencode_http_support;

use std::time::Duration;

use harness_business::opencode_server::{ClientOptions, EventStream, HttpError, SseFault};
use opencode_http_support::{Answer, chunked_response, connect, fixture, serve};
use pretty_assertions::assert_eq;
use serde_json::Value;

/// Every item until the end of the stream (fails after 10 s).
async fn drain(events: &mut EventStream) -> Vec<Result<String, SseFault>> {
    let mut items = Vec::new();

    loop {
        let next = tokio::time::timeout(Duration::from_secs(10), events.next())
            .await
            .expect("stream ends within 10 s");

        match next {
            Some(item) => items.push(item),
            None => return items,
        }
    }
}

/// Serves `response` and reads the whole event stream with `options`.
async fn stream(response: Vec<u8>, options: ClientOptions) -> Vec<Result<String, SseFault>> {
    let server = serve(response, Answer::default()).await;
    let (client, _) = connect(&server, options);
    let mut events = client.events().await.expect("event stream");

    drain(&mut events).await
}

fn payloads(items: Vec<Result<String, SseFault>>) -> Vec<String> {
    items.into_iter().map(|item| item.expect("event payload")).collect()
}

#[tokio::test]
async fn the_recorded_stream_yields_all_628_events() {
    let server = serve(fixture("global-event.sse"), Answer::default()).await;
    let (client, _) = connect(&server, ClientOptions::default());
    let mut events = client.events().await.expect("event stream");

    let events = payloads(drain(&mut events).await);
    let parsed = events
        .iter()
        .map(|event| serde_json::from_str::<Value>(event).expect("JSON payload"))
        .collect::<Vec<_>>();

    assert_eq!(parsed.len(), 628);
    assert_eq!(parsed[0]["payload"]["type"], "server.connected");
    assert_eq!(parsed[627]["payload"]["type"], "server.heartbeat");
    assert!(
        parsed
            .iter()
            .any(|event| event["payload"]["type"] == "permission.asked")
    );

    let request = server.request().await;

    assert!(request.starts_with("GET /global/event HTTP/1.1\r\n"));
    assert!(request.contains("\r\nAccept: text/event-stream\r\n"));
    assert!(!request.to_ascii_lowercase().contains("accept-encoding"));
}

#[tokio::test]
async fn multi_line_data_is_joined_and_other_lines_are_ignored() {
    let body = b": comment\nevent: message\nid: 7\nretry: 100\ndata: first\ndata:second\ndata\r\n\r\n\
                 event: no data here\n\n\
                 data:  two spaces\n\n\
                 data:\ndata: after an empty line\n\n";

    let events = payloads(stream(chunked_response(body, 3, true), ClientOptions::default()).await);

    assert_eq!(events, ["first\nsecond\n", " two spaces", "\nafter an empty line"]);
}

#[tokio::test]
async fn an_unfinished_last_event_is_dropped() {
    let body = b"data: complete\n\ndata: cut off";

    let events = payloads(stream(chunked_response(body, 4, false), ClientOptions::default()).await);

    assert_eq!(events, ["complete"]);
}

#[tokio::test]
async fn oversize_events_are_skipped_and_reported() {
    let options = ClientOptions {
        max_event_bytes: 16,
        ..ClientOptions::default()
    };

    let long_line = format!("data: {}\n\n", "x".repeat(40));
    let long_sum = "data: 0123456789\ndata: 0123456789\n\n";
    let body = format!("data: before\n\n{long_line}{long_sum}data: after\n\n");

    let items = stream(chunked_response(body.as_bytes(), 5, true), options).await;

    assert_eq!(
        items,
        [
            Ok("before".to_owned()),
            Err(SseFault::TooLarge { limit: 16 }),
            Err(SseFault::TooLarge { limit: 16 }),
            Ok("after".to_owned()),
        ]
    );
}

#[tokio::test]
async fn payloads_that_are_not_utf8_are_skipped_and_reported() {
    let body = b"data: \xff\xfe\n\ndata: fine\n\n";

    let items = stream(chunked_response(body, 64, true), ClientOptions::default()).await;

    assert_eq!(items, [Err(SseFault::NotUtf8), Ok("fine".to_owned())]);
}

#[tokio::test]
async fn a_malformed_chunk_ends_the_stream_with_a_fault() {
    let mut response = chunked_response(b"data: one\n\n", 64, false);
    response.extend_from_slice(b"not-hex\r\ndata: two\n\n\r\n");

    let items = stream(response, ClientOptions::default()).await;

    assert_eq!(
        items,
        [
            Ok("one".to_owned()),
            Err(SseFault::Stream("invalid chunk size".to_owned())),
        ]
    );
}

#[tokio::test]
async fn a_rejected_subscription_is_a_status_error() {
    let server = serve(fixture("health-unauthorized.http"), Answer::default()).await;
    let (client, _) = connect(&server, ClientOptions::default());

    let error = client.events().await.expect_err("401");

    assert!(matches!(error, HttpError::Status { status: 401 }));
}

#[tokio::test]
async fn dropping_the_stream_closes_the_connection() {
    let answer = Answer {
        max_piece: 64,
        hold: true,
    };
    let server = serve(chunked_response(b"data: one\n\n", 64, false), answer).await;
    let (client, _) = connect(&server, ClientOptions::default());
    let mut events = client.events().await.expect("event stream");

    assert_eq!(events.next().await, Some(Ok("one".to_owned())));

    drop(events);

    tokio::time::timeout(Duration::from_secs(5), server.request())
        .await
        .expect("the server sees the connection close");
}
