//! The closed route allowlist of `opencode serve`: methods, targets, id checks and query encoding.

use harness_business::error::BusinessError;
use harness_business::opencode_server::routes::{MAX_ID_LEN, MessageId, percent_encode};
use harness_business::opencode_server::{Method, PermissionId, QuestionId, Route, RouteError, SessionId};
use pretty_assertions::assert_eq;

const SESSION: &str = "ses_eaa561636ffezINnnn1ghd5370";

#[test]
fn every_route_has_its_method_target_and_deadline() {
    let session = SessionId::parse(SESSION).expect("session id");
    let permission = PermissionId::parse("per_155a9eef10017ZkJ249WX4lF43").expect("permission id");
    let question = QuestionId::parse("que_155a9f268001rb9HZw2SFL0I5S").expect("question id");
    let directory = "/work/project";
    let s = SESSION;

    let routes = [
        (Route::Health, Method::Get, "/global/health".to_owned(), false),
        (Route::GlobalEvent, Method::Get, "/global/event".to_owned(), false),
        (
            Route::SessionCreate { directory },
            Method::Post,
            "/session?directory=%2Fwork%2Fproject".to_owned(),
            false,
        ),
        (
            Route::SessionGet { session: &session },
            Method::Get,
            format!("/session/{s}"),
            false,
        ),
        (
            Route::SessionPatch { session: &session },
            Method::Patch,
            format!("/session/{s}"),
            false,
        ),
        (
            Route::SessionDelete { session: &session },
            Method::Delete,
            format!("/session/{s}"),
            false,
        ),
        (
            Route::PromptAsync { session: &session },
            Method::Post,
            format!("/session/{s}/prompt_async"),
            false,
        ),
        (
            Route::Command { session: &session },
            Method::Post,
            format!("/session/{s}/command"),
            true,
        ),
        (
            Route::Shell { session: &session },
            Method::Post,
            format!("/session/{s}/shell"),
            true,
        ),
        (
            Route::Fork { session: &session },
            Method::Post,
            format!("/session/{s}/fork"),
            false,
        ),
        (
            Route::Abort { session: &session },
            Method::Post,
            format!("/session/{s}/abort"),
            false,
        ),
        (
            Route::Summarize { session: &session },
            Method::Post,
            format!("/session/{s}/summarize"),
            true,
        ),
        (
            Route::Messages {
                session: &session,
                limit: None,
                before: None,
            },
            Method::Get,
            format!("/session/{s}/message"),
            false,
        ),
        (
            Route::Messages {
                session: &session,
                limit: Some(50),
                before: None,
            },
            Method::Get,
            format!("/session/{s}/message?limit=50"),
            false,
        ),
        (
            Route::Messages {
                session: &session,
                limit: None,
                before: Some("abc="),
            },
            Method::Get,
            format!("/session/{s}/message?before=abc%3D"),
            false,
        ),
        (
            Route::SessionStatus { directory },
            Method::Get,
            "/session/status?directory=%2Fwork%2Fproject".to_owned(),
            false,
        ),
        (
            Route::ConfigProviders { directory },
            Method::Get,
            "/config/providers?directory=%2Fwork%2Fproject".to_owned(),
            false,
        ),
        (
            Route::Agents { directory },
            Method::Get,
            "/agent?directory=%2Fwork%2Fproject".to_owned(),
            false,
        ),
        (
            Route::Commands { directory },
            Method::Get,
            "/command?directory=%2Fwork%2Fproject".to_owned(),
            false,
        ),
        (
            Route::PermissionReply {
                id: &permission,
                directory,
            },
            Method::Post,
            "/permission/per_155a9eef10017ZkJ249WX4lF43/reply?directory=%2Fwork%2Fproject".to_owned(),
            false,
        ),
        (
            Route::QuestionReply {
                id: &question,
                directory,
            },
            Method::Post,
            "/question/que_155a9f268001rb9HZw2SFL0I5S/reply?directory=%2Fwork%2Fproject".to_owned(),
            false,
        ),
        (
            Route::QuestionReject {
                id: &question,
                directory,
            },
            Method::Post,
            "/question/que_155a9f268001rb9HZw2SFL0I5S/reject?directory=%2Fwork%2Fproject".to_owned(),
            false,
        ),
    ];

    for (route, method, target, blocks) in routes {
        assert_eq!(
            (route.method(), route.target(), route.blocks()),
            (method, target, blocks),
            "{route:?}"
        );
    }
}

#[test]
fn ids_are_checked_before_they_reach_a_path() {
    let unsafe_ids = [
        "",
        "ses_",
        "ses",
        "msg_abc",
        "ses_abc/../../global/dispose",
        "ses_abc?directory=/",
        "ses_abc#x",
        "ses_abc%2F",
        "ses_abc def",
        "ses_abc\r\nHost: evil",
        "ses_abc-def",
        "ses_ąbc",
        "SES_abc",
    ];

    for id in unsafe_ids {
        let error = SessionId::parse(id).expect_err(id);

        assert_eq!(error, RouteError::InvalidId { kind: "session" });
        assert_eq!(error.to_string(), "invalid OpenCode session id");
    }

    let longest = format!("ses_{}", "a".repeat(MAX_ID_LEN - 4));

    assert!(SessionId::parse(&longest).is_ok());
    assert!(SessionId::parse(&format!("{longest}a")).is_err());

    assert!(MessageId::parse("msg_155aa035e001dsQaAHL5Q5tArc").is_ok());
    assert!(MessageId::parse(SESSION).is_err());
    assert!(PermissionId::parse("que_155a9f268001rb9HZw2SFL0I5S").is_err());
    assert!(QuestionId::parse("per_155a9eef10017ZkJ249WX4lF43").is_err());

    let error = BusinessError::from(MessageId::parse("msg_../x").expect_err("unsafe"));

    assert_eq!(error.to_string(), "invalid input: invalid OpenCode message id");
}

#[test]
fn query_values_are_percent_encoded() {
    assert_eq!(
        percent_encode("/home/Jo Ann/my-repo_1.0~x"),
        "%2Fhome%2FJo%20Ann%2Fmy-repo_1.0~x"
    );
    assert_eq!(percent_encode("a&b=c?d#e+f%"), "a%26b%3Dc%3Fd%23e%2Bf%25");
    assert_eq!(percent_encode("zażółć"), "za%C5%BC%C3%B3%C5%82%C4%87");
    assert_eq!(percent_encode("C:\\Users\\x"), "C%3A%5CUsers%5Cx");

    let route = Route::Agents {
        directory: "/tmp/a b&directory=/etc",
    };

    assert_eq!(route.target(), "/agent?directory=%2Ftmp%2Fa%20b%26directory%3D%2Fetc");
}
