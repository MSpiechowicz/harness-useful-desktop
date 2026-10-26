//! The loopback HTTP client of `opencode serve` against recorded and crafted responses.

use crate::opencode_http_support;

use std::net::SocketAddr;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use harness_business::error::BusinessError;
use harness_business::opencode_server::{ClientOptions, HttpError, OpenCodeClient, Route, ServerPassword, SessionId};
use opencode_http_support::{Answer, chunked_response, connect, fixture, json_response, serve, serve_silence};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const SESSION: &str = "ses_eaa561636ffezINnnn1ghd5370";

const NEXT_CURSOR: &str = "eyJpZCI6Im1zZ18xNTVhYTAzNWUwMDFkc1FhQUhMNVE1dEFyYyIsInRpbWUiOjE3OTI0Mzg1Njc3NzR9";

fn session() -> SessionId {
    SessionId::parse(SESSION).expect("recorded session id")
}

#[tokio::test]
async fn health_sends_basic_auth_and_reads_the_recorded_answer() {
    let server = serve(fixture("health.http"), Answer::default()).await;
    let (client, password) = connect(&server, ClientOptions::default());

    let response = client.send(Route::Health, None).await.expect("health");

    assert_eq!(response.status(), 200);
    assert_eq!(response.header("content-type"), Some("application/json"));
    assert_eq!(
        response.json::<Value>().expect("json"),
        json!({"healthy": true, "version": "1.19.3"})
    );

    let request = server.request().await;
    let credentials = BASE64.encode(format!("opencode:{}", password.expose_secret()));
    let expected = format!(
        "GET /global/health HTTP/1.1\r\nHost: {}\r\nAuthorization: Basic {credentials}\r\n\
         Accept: application/json\r\nConnection: close\r\n\r\n",
        client.addr()
    );

    assert_eq!(request, expected);
}

#[tokio::test]
async fn responses_survive_one_byte_reads() {
    let answer = Answer {
        max_piece: 1,
        hold: false,
    };
    let server = serve(fixture("health.http"), answer).await;
    let (client, _) = connect(&server, ClientOptions::default());

    let response = client.send(Route::Health, None).await.expect("health");

    assert_eq!(response.body(), br#"{"healthy":true,"version":"1.19.3"}"#);
}

#[tokio::test]
async fn unauthorized_is_returned_as_a_status_and_fails_error_for_status() {
    let server = serve(fixture("health-unauthorized.http"), Answer::default()).await;
    let (client, password) = connect(&server, ClientOptions::default());

    let response = client.send(Route::Health, None).await.expect("a 401 is a response");

    assert_eq!(response.status(), 401);
    assert_eq!(response.header("WWW-Authenticate"), Some("Basic realm=\"Secure Area\""));
    assert!(response.body().is_empty());

    let error = response.error_for_status().expect_err("401 fails");

    assert!(matches!(error, HttpError::Status { status: 401 }));
    assert_eq!(error.to_string(), "the OpenCode server answered HTTP 401");
    assert_secret_free(&format!("{error} {error:?}"), &password);
}

#[tokio::test]
async fn message_pages_expose_the_next_cursor_and_encode_before() {
    let server = serve(fixture("messages-raw.http"), Answer::default()).await;
    let (client, _) = connect(&server, ClientOptions::default());
    let session = session();

    let route = Route::Messages {
        session: &session,
        limit: Some(2),
        before: Some("a+b/c="),
    };
    let response = client.send(route, None).await.expect("messages");

    assert_eq!(response.next_cursor(), Some(NEXT_CURSOR));

    let messages = response.json::<Vec<Value>>().expect("message list");

    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["info"]["sessionID"], SESSION);

    let request = server.request().await;

    assert!(request.starts_with(&format!(
        "GET /session/{SESSION}/message?limit=2&before=a%2Bb%2Fc%3D HTTP/1.1\r\n"
    )));
}

#[tokio::test]
async fn config_providers_reads_a_large_recorded_body() {
    let server = serve(fixture("config-providers-raw.http"), Answer::default()).await;
    let (client, _) = connect(&server, ClientOptions::default());

    let route = Route::ConfigProviders {
        directory: "/work/my project",
    };
    let providers = client
        .send(route, None)
        .await
        .expect("providers")
        .json::<Value>()
        .expect("json");

    assert_eq!(providers["default"], json!({"opencode": "big-pickle", "mock": "m1"}));
    assert!(providers["providers"].as_array().is_some_and(|list| !list.is_empty()));

    let request = server.request().await;

    assert!(request.starts_with("GET /config/providers?directory=%2Fwork%2Fmy%20project HTTP/1.1\r\n"));
}

#[tokio::test]
async fn json_bodies_carry_type_and_length_and_never_ask_for_compression() {
    let server = serve(
        b"HTTP/1.1 204 No Content\r\nVary: Origin\r\n\r\n".to_vec(),
        Answer::default(),
    )
    .await;
    let (client, _) = connect(&server, ClientOptions::default());
    let session = session();
    let body = json!({"parts": [{"type": "text", "text": "héllo"}]});

    let response = client
        .send(Route::PromptAsync { session: &session }, Some(&body))
        .await
        .expect("prompt");

    assert_eq!(response.status(), 204);
    assert!(response.body().is_empty());

    let request = server.request().await;
    let encoded = body.to_string();

    assert!(request.starts_with(&format!("POST /session/{SESSION}/prompt_async HTTP/1.1\r\n")));
    assert!(request.contains("\r\nContent-Type: application/json\r\n"));
    assert!(request.contains(&format!("\r\nContent-Length: {}\r\n", encoded.len())));
    assert!(request.ends_with(&format!("\r\n\r\n{encoded}")));
    assert!(!request.to_ascii_lowercase().contains("accept-encoding"));
}

#[tokio::test]
async fn post_without_a_body_sends_a_zero_length() {
    let server = serve(json_response("true"), Answer::default()).await;
    let (client, _) = connect(&server, ClientOptions::default());
    let session = session();

    client
        .send(Route::Abort { session: &session }, None)
        .await
        .expect("abort");

    let request = server.request().await;

    assert!(request.starts_with(&format!("POST /session/{SESSION}/abort HTTP/1.1\r\n")));
    assert!(request.ends_with("Content-Length: 0\r\n\r\n"));
}

#[tokio::test]
async fn chunked_json_bodies_are_decoded() {
    let body = br#"{"id":"ses_abc","title":"chunked across many pieces"}"#;
    let mut response = chunked_response(body, 5, true);
    response = String::from_utf8(response)
        .expect("utf-8")
        .replace("text/event-stream", "application/json")
        .into_bytes();

    let server = serve(response, Answer::default()).await;
    let (client, _) = connect(&server, ClientOptions::default());
    let session = session();

    let response = client
        .send(Route::SessionGet { session: &session }, None)
        .await
        .expect("session");

    assert_eq!(response.body(), body);
}

#[tokio::test]
async fn malformed_chunk_sizes_fail() {
    let response = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nzz\r\nabc\r\n0\r\n\r\n".to_vec();
    let server = serve(response, Answer::default()).await;
    let (client, _) = connect(&server, ClientOptions::default());

    let error = client.send(Route::Health, None).await.expect_err("bad chunk size");

    assert_eq!(
        error.to_string(),
        "OpenCode server connection failed: invalid chunk size"
    );
}

#[tokio::test]
async fn bodies_over_the_limit_fail() {
    let options = ClientOptions {
        max_body_bytes: 16,
        ..ClientOptions::default()
    };

    let server = serve(fixture("health.http"), Answer::default()).await;
    let (client, _) = connect(&server, options);
    let error = client.send(Route::Health, None).await.expect_err("35-byte body");

    assert!(matches!(error, HttpError::BodyTooLarge { limit: 16 }));

    let server = serve(chunked_response(&[b'x'; 40], 7, true), Answer::default()).await;
    let (client, _) = connect(&server, options);
    let error = client
        .send(Route::Health, None)
        .await
        .expect_err("40-byte chunked body");

    assert!(matches!(error, HttpError::BodyTooLarge { limit: 16 }));
}

#[tokio::test]
async fn heads_over_64_kib_fail() {
    let mut response = b"HTTP/1.1 200 OK\r\n".to_vec();

    for index in 0..2000 {
        response.extend_from_slice(format!("x-filler-{index}: {}\r\n", "y".repeat(32)).as_bytes());
    }

    response.extend_from_slice(b"Content-Length: 0\r\n\r\n");

    let answer = Answer {
        max_piece: 4096,
        hold: false,
    };
    let server = serve(response, answer).await;
    let (client, _) = connect(&server, ClientOptions::default());

    let error = client.send(Route::Health, None).await.expect_err("huge head");

    assert!(matches!(error, HttpError::HeadTooLarge));
}

#[tokio::test]
async fn bodies_cut_short_fail() {
    let response = b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n{\"healthy\":".to_vec();
    let server = serve(response, Answer::default()).await;
    let (client, _) = connect(&server, ClientOptions::default());

    let error = client.send(Route::Health, None).await.expect_err("short body");

    assert!(matches!(error, HttpError::Malformed("the response body ended early")));

    let response = chunked_response(b"{\"healthy\":true}", 4, false);
    let server = serve(response, Answer::default()).await;
    let (client, _) = connect(&server, ClientOptions::default());

    let error = client.send(Route::Health, None).await.expect_err("no final chunk");

    assert!(matches!(error, HttpError::Malformed("the response body ended early")));
}

#[tokio::test]
async fn silent_servers_time_out_and_blocking_routes_get_the_long_deadline() {
    let options = ClientOptions {
        timeout: Duration::from_millis(100),
        blocking_timeout: Duration::from_millis(400),
        ..ClientOptions::default()
    };

    let server = serve_silence().await;
    let (client, password) = connect(&server, options);
    let started = tokio::time::Instant::now();

    let error = client.send(Route::Health, None).await.expect_err("no answer");

    assert!(matches!(error, HttpError::Timeout(limit) if limit == Duration::from_millis(100)));
    assert!(started.elapsed() < Duration::from_millis(350));
    assert_secret_free(&format!("{error} {error:?}"), &password);

    let server = serve_silence().await;
    let (client, _) = connect(&server, options);
    let session = session();
    let started = tokio::time::Instant::now();

    let error = client
        .send(Route::Shell { session: &session }, None)
        .await
        .expect_err("no answer");

    assert!(matches!(error, HttpError::Timeout(limit) if limit == Duration::from_millis(400)));
    assert!(started.elapsed() >= Duration::from_millis(400));
}

#[tokio::test]
async fn the_event_route_is_not_a_plain_request() {
    let client = OpenCodeClient::new(loopback(), ServerPassword::random()).expect("loopback");

    let error = client.send(Route::GlobalEvent, None).await.expect_err("stream route");

    assert!(matches!(error, HttpError::StreamRoute));
}

#[test]
fn only_loopback_addresses_are_accepted() {
    for refused in [
        "10.0.0.1:4096",
        "0.0.0.0:4096",
        "192.168.1.2:80",
        "[::]:4096",
        "[::ffff:127.0.0.1]:4096",
    ] {
        let addr = refused.parse::<SocketAddr>().expect("address");
        let error = OpenCodeClient::new(addr, ServerPassword::random()).expect_err(refused);

        assert!(matches!(error, HttpError::NotLoopback), "{refused}");
        assert!(matches!(BusinessError::from(error), BusinessError::Forbidden(_)));
    }

    for accepted in ["127.0.0.1:4096", "127.0.0.2:1", "[::1]:4096"] {
        let addr = accepted.parse::<SocketAddr>().expect("address");

        assert!(
            OpenCodeClient::new(addr, ServerPassword::random()).is_ok(),
            "{accepted}"
        );
    }
}

#[tokio::test]
async fn the_password_never_shows_in_debug_output_or_errors() {
    let password = ServerPassword::random();
    let client = OpenCodeClient::new(loopback(), password.clone()).expect("loopback");

    assert_eq!(password.expose_secret().len(), 64);
    assert_ne!(password, ServerPassword::random());
    assert_eq!(format!("{password:?}"), "ServerPassword(<redacted>)");
    assert_secret_free(&format!("{client:?}"), &password);

    let refused = client.send(Route::Health, None).await.expect_err("nothing listens");

    assert_secret_free(&format!("{refused} {refused:?}"), &password);
    assert_secret_free(&BusinessError::from(refused).to_string(), &password);
}

/// A loopback port nothing listens on.
fn loopback() -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    listener.local_addr().expect("address")
}

fn assert_secret_free(text: &str, password: &ServerPassword) {
    let secret = password.expose_secret();
    let credentials = BASE64.encode(format!("opencode:{secret}"));

    assert!(!text.contains(secret), "password leaked: {text}");
    assert!(!text.contains(&credentials), "credentials leaked: {text}");
}
