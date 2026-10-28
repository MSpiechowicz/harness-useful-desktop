//! The subagent panel's shared contract: wire format, action limits and the bounded-text helpers.

use harness_business::model::{
    AgentAction, AgentEvent, IpcError, IpcErrorCode, MAX_ACTION_TEXT_BYTES, MAX_SUBAGENT_ID_BYTES, SubagentStep,
    SubagentStepKind, SubagentStepStatus,
};
use harness_business::security::{bounded_masked, bounded_redacted, tail_masked, tail_redacted};
use pretty_assertions::assert_eq;
use serde_json::json;

/// A GitHub-style token, built at run time so no secret-looking literal sits in the source.
fn fake_token() -> String {
    format!("ghp_{}", "a1B2c3D4".repeat(5))
}

fn stop(id: &str) -> AgentAction {
    AgentAction::StopSubagent { id: id.to_owned() }
}

fn message(id: &str, text: &str) -> AgentAction {
    AgentAction::MessageSubagent {
        id: id.to_owned(),
        message: text.to_owned(),
    }
}

#[test]
fn subagent_actions_and_step_events_use_the_camel_case_wire_format() {
    let action: AgentAction =
        serde_json::from_value(json!({ "type": "messageSubagent", "id": "s1", "message": "hi" })).unwrap();
    assert_eq!(action, message("s1", "hi"));
    assert_eq!(
        serde_json::to_value(stop("s1")).unwrap(),
        json!({ "type": "stopSubagent", "id": "s1" })
    );

    let event = AgentEvent::SubagentStep {
        subagent_id: "s1".to_owned(),
        step: Box::new(SubagentStep {
            id: "call-1".to_owned(),
            kind: SubagentStepKind::Tool,
            name: Some("Read".to_owned()),
            detail: "src/main.rs".to_owned(),
            status: SubagentStepStatus::Done,
            at: Some(1_700_000_000_000),
            output: Some("fn main() {}".to_owned()),
            output_lines: Some(1),
        }),
    };
    let expected = json!({
        "type": "subagentStep",
        "subagentId": "s1",
        "step": {
            "id": "call-1", "kind": "tool", "name": "Read", "detail": "src/main.rs", "status": "done",
            "at": 1_700_000_000_000_u64, "output": "fn main() {}", "outputLines": 1
        }
    });
    assert_eq!(serde_json::to_value(&event).unwrap(), expected);
    assert_eq!(serde_json::from_value::<AgentEvent>(expected).unwrap(), event);
}

#[test]
fn subagent_actions_accept_what_fits() {
    assert!(stop("s").validate().is_ok());
    assert!(stop(&"i".repeat(MAX_SUBAGENT_ID_BYTES)).validate().is_ok());
    assert!(message("s", "go on").validate().is_ok());
    assert!(message("s", &"x".repeat(MAX_ACTION_TEXT_BYTES)).validate().is_ok());
}

#[test]
fn subagent_actions_refuse_bad_ids_and_messages() {
    let refused = [
        stop(""),
        stop(&"i".repeat(MAX_SUBAGENT_ID_BYTES + 1)),
        message("", "hi"),
        message(&"i".repeat(MAX_SUBAGENT_ID_BYTES + 1), "hi"),
        message("s", ""),
        message("s", "  \n"),
        message("s", &"x".repeat(MAX_ACTION_TEXT_BYTES + 1)),
    ];

    for action in refused {
        let error = IpcError::from(action.validate().unwrap_err());
        assert_eq!(error.code, IpcErrorCode::InvalidInput, "{action:?}");
    }
}

#[test]
fn tail_keeps_the_last_lines_and_counts_all_of_them() {
    let text = (1..=10).map(|n| format!("line {n}")).collect::<Vec<_>>().join("\n");

    let (tail, total) = tail_redacted(&text, 3, 1024);

    assert_eq!(tail, "line 8\nline 9\nline 10");
    assert_eq!(total, 10);
}

#[test]
fn tail_cuts_at_line_boundaries_when_the_bytes_run_out() {
    let text = "aaaa\nbbbb\ncccc\ndddd";

    // 13 bytes hold "cccc\ndddd" (9) but not "bbbb\ncccc\ndddd" (14).
    let (tail, total) = tail_redacted(text, 400, 13);

    assert_eq!(tail, "cccc\ndddd");
    assert_eq!(total, 4);
}

#[test]
fn tail_keeps_the_end_of_a_single_line_longer_than_the_byte_limit() {
    let (tail, total) = tail_redacted(&format!("{}END", "é".repeat(50)), 400, 6);

    assert_eq!(tail, "éEND");
    assert_eq!(total, 1);
}

#[test]
fn tail_of_nothing_is_empty() {
    assert_eq!(tail_redacted("", 400, 1024), (String::new(), 0));
}

#[test]
fn tail_and_bounded_text_hide_secrets() {
    let token = fake_token();
    let text = format!("first\ntoken={token}\nlast");

    let (tail, _) = tail_redacted(&text, 400, 4096);
    assert!(!tail.contains(&token), "{tail}");
    assert_eq!(tail, "first\ntoken=••••\nlast");

    let bounded = bounded_redacted(&text, 4096);
    assert!(!bounded.contains(&token), "{bounded}");
}

#[test]
fn bounded_text_is_cut_to_the_character_limit_with_an_ellipsis() {
    assert_eq!(bounded_redacted("abcdef", 6), "abcdef");
    assert_eq!(bounded_redacted("abcdefg", 6), "abcde…");
    assert_eq!(bounded_redacted("ééééééé", 4), "ééé…");
}

#[test]
fn a_secret_is_masked_before_the_cut_so_no_piece_of_it_survives() {
    let secret = "0123456789abcdef".repeat(4);

    // The cut falls in the middle of the secret.
    let text = format!("{}{secret}", "x".repeat(20));
    let bounded = bounded_masked(&text, Some(&secret), 20 + secret.len() / 2 + 1);
    assert_eq!(bounded, format!("{}••••", "x".repeat(20)));

    // The start of the kept tail falls in the middle of the secret.
    let line = format!("{secret}{}", "q".repeat(100));
    let (tail, lines) = tail_masked(&line, Some(&secret), 400, 100 + secret.len() / 2);
    assert_eq!((tail.contains("0123"), lines), (false, 1), "{tail}");

    // Without a secret nothing changes.
    assert_eq!(bounded_masked("abc", None, 10), "abc");
}
