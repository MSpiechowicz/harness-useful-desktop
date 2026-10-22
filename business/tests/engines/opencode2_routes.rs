//! The closed route allowlist of the OpenCode 2 server: methods, targets, deadlines, id checks and
//! the `location[directory]` query, against the recorded requests (`fixtures/opencode2/recorded`).

use crate::opencode2_recorded;

use harness_business::error::BusinessError;
use harness_business::opencode_server::http::{ApiRoute, Method};
use harness_business::opencode_server_v2::routes::{MAX_ID_LEN, MAX_MESSAGE_PAGE};
use harness_business::opencode_server_v2::{FormId, MessageId, MessageOrder, PermissionId, RouteV2, SessionId};
use opencode2_recorded::{DIRECTORY, Exchange, exchanges};
use pretty_assertions::assert_eq;

const SESSION: &str = "ses_ea27cfa99ffe21eKppwXzXASSa";
const LOCATION: &str = "location%5Bdirectory%5D=%2Fwork%2Fproj";

#[test]
fn every_route_has_its_method_target_and_deadline() {
    let session = SessionId::parse(SESSION).expect("session id");
    let permission = PermissionId::parse("per_15d7c3a33001lsSJ9gGeiFzCh8").expect("permission id");
    let form = FormId::parse("frm_15d80582d001AwxHnvC4VOQ9RP").expect("form id");
    let directory = DIRECTORY;
    let s = format!("/api/session/{SESSION}");
    let l = LOCATION;

    let routes = [
        (RouteV2::Info, Method::Get, "/api/info".to_owned()),
        (RouteV2::Event, Method::Get, "/api/event".to_owned()),
        (
            RouteV2::SessionCreate { directory },
            Method::Post,
            format!("/api/session?{l}"),
        ),
        (
            RouteV2::SessionGet {
                session: &session,
                directory,
            },
            Method::Get,
            format!("{s}?{l}"),
        ),
        (
            RouteV2::SessionUpdate {
                session: &session,
                directory,
            },
            Method::Patch,
            format!("{s}?{l}"),
        ),
        (
            RouteV2::SessionDelete {
                session: &session,
                directory,
            },
            Method::Delete,
            format!("{s}?{l}"),
        ),
        (
            RouteV2::SessionFork {
                session: &session,
                directory,
            },
            Method::Post,
            format!("{s}/fork?{l}"),
        ),
        (
            RouteV2::SessionAgent {
                session: &session,
                directory,
            },
            Method::Post,
            format!("{s}/agent?{l}"),
        ),
        (
            RouteV2::SessionModel {
                session: &session,
                directory,
            },
            Method::Post,
            format!("{s}/model?{l}"),
        ),
        (
            RouteV2::Prompt {
                session: &session,
                directory,
            },
            Method::Post,
            format!("{s}/prompt?{l}"),
        ),
        (
            RouteV2::Interrupt {
                session: &session,
                directory,
            },
            Method::Post,
            format!("{s}/interrupt?{l}"),
        ),
        (
            RouteV2::Command {
                session: &session,
                directory,
            },
            Method::Post,
            format!("{s}/command?{l}"),
        ),
        (
            RouteV2::Shell {
                session: &session,
                directory,
            },
            Method::Post,
            format!("{s}/shell?{l}"),
        ),
        (
            RouteV2::Compact {
                session: &session,
                directory,
            },
            Method::Post,
            format!("{s}/compact?{l}"),
        ),
        (
            RouteV2::Messages {
                session: &session,
                directory,
                limit: 50,
                cursor: None,
                order: None,
            },
            Method::Get,
            format!("{s}/message?limit=50&{l}"),
        ),
        (
            RouteV2::PermissionList {
                session: &session,
                directory,
            },
            Method::Get,
            format!("{s}/permission?{l}"),
        ),
        (
            RouteV2::PermissionReply {
                session: &session,
                permission: &permission,
                directory,
            },
            Method::Post,
            format!("{s}/permission/per_15d7c3a33001lsSJ9gGeiFzCh8/reply?{l}"),
        ),
        (RouteV2::FormList { directory }, Method::Get, format!("/api/form?{l}")),
        (
            RouteV2::FormReply {
                session: &session,
                form: &form,
                directory,
            },
            Method::Post,
            format!("{s}/form/frm_15d80582d001AwxHnvC4VOQ9RP/reply?{l}"),
        ),
        (
            RouteV2::FormCancel {
                session: &session,
                form: &form,
                directory,
            },
            Method::Delete,
            format!("{s}/form/frm_15d80582d001AwxHnvC4VOQ9RP?{l}"),
        ),
        (RouteV2::AgentList { directory }, Method::Get, format!("/api/agent?{l}")),
        (
            RouteV2::CommandList { directory },
            Method::Get,
            format!("/api/command?{l}"),
        ),
        (RouteV2::SkillList { directory }, Method::Get, format!("/api/skill?{l}")),
        (RouteV2::ModelList { directory }, Method::Get, format!("/api/model?{l}")),
    ];

    for (route, method, target) in routes {
        assert_eq!((route.method(), route.target()), (method, target), "{route:?}");

        let shell = matches!(route, RouteV2::Shell { .. });
        assert_eq!(route.blocks(), shell, "only shell waits for its work: {route:?}");
        assert_eq!(
            ApiRoute::is_stream(&route),
            matches!(route, RouteV2::Event),
            "{route:?}"
        );
        assert_eq!(ApiRoute::target(&route), route.target());
    }
}

#[test]
fn every_recorded_request_is_an_allowlisted_route() {
    let mut matched = 0;

    for exchange in exchanges() {
        if exchange.method == "GET" && exchange.path() == "/api/session" {
            // The session list is read through the CLI (`session list --standalone`), not here.
            continue;
        }

        if exchange.method == "GET" && exchange.path() == "/api/config" {
            // The configuration is read through the CLI (`api --standalone GET /api/config`).
            continue;
        }

        let (method, target) = route_of(&exchange);
        assert_eq!(
            (method.as_str(), target.as_str()),
            (exchange.method.as_str(), exchange.target.as_str()),
            "{}",
            exchange.file
        );
        matched += 1;
    }

    assert!(matched >= 75, "the recordings hold {matched} requests");
}

/// The allowlisted route of a recorded request, built from the ids in its path, as method and
/// target.
fn route_of(exchange: &Exchange) -> (Method, String) {
    let segments: Vec<&str> = exchange.path().trim_start_matches("/api/").split('/').collect();
    let session = (segments[0] == "session" && segments.len() > 1)
        .then(|| SessionId::parse(segments[1]).expect("recorded session id"));
    let permission = id_after(&segments, "permission").map(|id| PermissionId::parse(id).expect("permission id"));
    let form = id_after(&segments, "form").map(|id| FormId::parse(id).expect("form id"));
    let session = session.as_ref();
    let directory = DIRECTORY;

    let route = match (exchange.method.as_str(), segments.as_slice()) {
        ("GET", ["info"]) => RouteV2::Info,
        ("POST", ["session"]) => RouteV2::SessionCreate { directory },
        ("GET", ["session", _]) => RouteV2::SessionGet {
            session: session.expect("session"),
            directory,
        },
        ("PATCH", ["session", _]) => RouteV2::SessionUpdate {
            session: session.expect("session"),
            directory,
        },
        ("DELETE", ["session", _]) => RouteV2::SessionDelete {
            session: session.expect("session"),
            directory,
        },
        ("POST", ["session", _, "fork"]) => RouteV2::SessionFork {
            session: session.expect("session"),
            directory,
        },
        ("POST", ["session", _, "prompt"]) => RouteV2::Prompt {
            session: session.expect("session"),
            directory,
        },
        ("POST", ["session", _, "interrupt"]) => RouteV2::Interrupt {
            session: session.expect("session"),
            directory,
        },
        ("POST", ["session", _, "command"]) => RouteV2::Command {
            session: session.expect("session"),
            directory,
        },
        ("POST", ["session", _, "shell"]) => RouteV2::Shell {
            session: session.expect("session"),
            directory,
        },
        ("POST", ["session", _, "compact"]) => RouteV2::Compact {
            session: session.expect("session"),
            directory,
        },
        ("GET", ["session", _, "message"]) => RouteV2::Messages {
            session: session.expect("session"),
            directory,
            limit: 50,
            cursor: None,
            order: None,
        },
        ("GET", ["session", _, "permission"]) => RouteV2::PermissionList {
            session: session.expect("session"),
            directory,
        },
        ("POST", ["session", _, "permission", _, "reply"]) => RouteV2::PermissionReply {
            session: session.expect("session"),
            permission: permission.as_ref().expect("permission"),
            directory,
        },
        ("POST", ["session", _, "form", _, "reply"]) => RouteV2::FormReply {
            session: session.expect("session"),
            form: form.as_ref().expect("form"),
            directory,
        },
        ("GET", ["form"]) => RouteV2::FormList { directory },
        ("GET", ["agent"]) => RouteV2::AgentList { directory },
        ("GET", ["command"]) => RouteV2::CommandList { directory },
        ("GET", ["model"]) => RouteV2::ModelList { directory },
        other => panic!("no allowlisted route for the recorded {other:?} ({})", exchange.file),
    };

    (route.method(), route.target())
}

fn id_after<'a>(segments: &[&'a str], name: &str) -> Option<&'a str> {
    let index = segments.iter().position(|segment| *segment == name)?;
    segments.get(index + 1).copied()
}

#[test]
fn ids_take_their_prefix_and_only_letters_and_digits() {
    assert_eq!(SessionId::parse(SESSION).expect("session").as_str(), SESSION);
    assert!(MessageId::parse("msg_15d824387001C5U5EIkeFSEWKC").is_ok());
    assert!(PermissionId::parse("per_15d7c8eeb001fukvNGTNsK07QQ").is_ok());
    assert!(FormId::parse("frm_15d80582d001AwxHnvC4VOQ9RP").is_ok());

    let too_long = format!("ses_{}", "a".repeat(MAX_ID_LEN));
    let refused = [
        "",
        "ses_",
        "ses",
        "msg_15d824387001C5U5EIkeFSEWKC",
        "ses_abc_def",
        "ses_abc/../x",
        "ses_abc?x=1",
        "ses_abc%2F",
        "ses_ab c",
        "ses_ü",
        too_long.as_str(),
    ];

    for value in refused {
        let error = SessionId::parse(value).expect_err(value);
        assert_eq!(error.to_string(), "invalid OpenCode session id");
    }

    let error = FormId::parse("que_155a9f268001rb9HZw2SFL0I5S").expect_err("a v1 question id");
    assert!(matches!(BusinessError::from(error), BusinessError::InvalidInput(message) if message.contains("form")));
}

#[test]
fn ids_serialize_as_their_text() {
    let message = MessageId::parse("msg_15d824387001C5U5EIkeFSEWKC").expect("message id");

    assert_eq!(
        serde_json::to_value(&message).expect("json"),
        serde_json::json!("msg_15d824387001C5U5EIkeFSEWKC")
    );
    assert_eq!(message.to_string(), "msg_15d824387001C5U5EIkeFSEWKC");
}

#[test]
fn the_directory_is_percent_encoded_into_the_location_query() {
    let route = RouteV2::AgentList {
        directory: "/home/me/my project/ü&x=1#top",
    };

    assert_eq!(
        route.target(),
        "/api/agent?location%5Bdirectory%5D=%2Fhome%2Fme%2Fmy%20project%2F%C3%BC%26x%3D1%23top"
    );
}

#[test]
fn message_pages_clamp_the_limit_and_encode_the_cursor() {
    let session = SessionId::parse(SESSION).expect("session id");
    let page = |limit, cursor, order| {
        RouteV2::Messages {
            session: &session,
            directory: DIRECTORY,
            limit,
            cursor,
            order,
        }
        .target()
    };
    let base = format!("/api/session/{SESSION}/message");

    assert_eq!(page(0, None, None), format!("{base}?limit=1&{LOCATION}"));
    assert_eq!(
        page(5000, None, None),
        format!("{base}?limit={MAX_MESSAGE_PAGE}&{LOCATION}")
    );
    assert_eq!(
        page(20, None, Some(MessageOrder::Ascending)),
        format!("{base}?limit=20&order=asc&{LOCATION}")
    );
    assert_eq!(
        page(20, None, Some(MessageOrder::Descending)),
        format!("{base}?limit=20&order=desc&{LOCATION}")
    );
    assert_eq!(
        page(20, Some("eyJpZCI6Im1zZ18x+/=&x"), None),
        format!("{base}?limit=20&cursor=eyJpZCI6Im1zZ18x%2B%2F%3D%26x&{LOCATION}")
    );
}
