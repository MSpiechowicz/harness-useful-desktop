//! How each dialect encodes actions and decodes recorded frames.

use std::path::Path;

use harness_business::harness::Harness;
use harness_business::harnesses::{Omp, Pi};
use harness_business::model::{
    AgentAction, AgentEvent, HarnessId, ImageAttachment, IpcErrorCode, PromptStatus, SessionRef, StreamingBehavior,
    UiOutcome, UiRequestKind, UiResponse,
};
use harness_business::pi_family::rpc::{DecodeState, EncodeContext, Handshake, Inbound, RpcDialect};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn encode(dialect: &dyn RpcDialect, action: AgentAction) -> Value {
    let context = EncodeContext {
        sessions_dir: Path::new("/nonexistent"),
    };

    dialect.encode(&action, "req-9", &context).expect("encodes")
}

fn text(value: &str) -> String {
    value.to_owned()
}

#[test]
fn both_dialects_share_the_pi_family_command_names() {
    let cases = [
        (
            AgentAction::Prompt {
                message: text("hi"),
                images: Vec::new(),
                streaming_behavior: Some(StreamingBehavior::FollowUp),
            },
            json!({ "id": "req-9", "type": "prompt", "message": "hi", "streamingBehavior": "followUp" }),
        ),
        (
            AgentAction::Prompt {
                message: text("hi"),
                images: Vec::new(),
                streaming_behavior: None,
            },
            json!({ "id": "req-9", "type": "prompt", "message": "hi" }),
        ),
        (
            AgentAction::Steer {
                message: text("s"),
                images: Vec::new(),
            },
            json!({ "id": "req-9", "type": "steer", "message": "s" }),
        ),
        (
            AgentAction::FollowUp {
                message: text("f"),
                images: Vec::new(),
            },
            json!({ "id": "req-9", "type": "follow_up", "message": "f" }),
        ),
        (AgentAction::Abort, json!({ "id": "req-9", "type": "abort" })),
        (AgentAction::NewSession, json!({ "id": "req-9", "type": "new_session" })),
        (
            AgentAction::Fork { entry_id: text("e1") },
            json!({ "id": "req-9", "type": "fork", "entryId": "e1" }),
        ),
        (
            AgentAction::SetModel {
                provider: text("openai"),
                model_id: text("gpt-5"),
            },
            json!({ "id": "req-9", "type": "set_model", "provider": "openai", "modelId": "gpt-5" }),
        ),
        (
            AgentAction::SetThinkingLevel { level: text("high") },
            json!({ "id": "req-9", "type": "set_thinking_level", "level": "high" }),
        ),
        (
            AgentAction::Compact {
                instructions: Some(text("keep tests")),
            },
            json!({ "id": "req-9", "type": "compact", "customInstructions": "keep tests" }),
        ),
        (
            AgentAction::SetSessionName { name: text("n") },
            json!({ "id": "req-9", "type": "set_session_name", "name": "n" }),
        ),
        (AgentAction::GetState, json!({ "id": "req-9", "type": "get_state" })),
        (
            AgentAction::GetAvailableModels,
            json!({ "id": "req-9", "type": "get_available_models" }),
        ),
        (
            AgentAction::GetAvailableThinkingLevels,
            json!({ "id": "req-9", "type": "get_available_thinking_levels" }),
        ),
        (
            AgentAction::GetSessionStats,
            json!({ "id": "req-9", "type": "get_session_stats" }),
        ),
        (AgentAction::ExportHtml, json!({ "id": "req-9", "type": "export_html" })),
        (
            AgentAction::GetLastAssistantText,
            json!({ "id": "req-9", "type": "get_last_assistant_text" }),
        ),
    ];

    for dialect in [Omp.dialect(), Pi.dialect()] {
        for (action, expected) in &cases {
            assert_eq!(&encode(dialect, action.clone()), expected, "{:?}", dialect.harness());
        }
    }
}

#[test]
fn images_ride_along_with_prompt_steer_and_follow_up() {
    let image = || ImageAttachment {
        data: text("aGk="),
        mime_type: text("image/png"),
    };
    let blocks = json!([{ "type": "image", "data": "aGk=", "mimeType": "image/png" }]);

    for dialect in [Omp.dialect(), Pi.dialect()] {
        let prompt = AgentAction::Prompt {
            message: text("look"),
            images: vec![image()],
            streaming_behavior: None,
        };
        assert_eq!(encode(dialect, prompt)["images"], blocks);
        assert_eq!(
            encode(
                dialect,
                AgentAction::Steer {
                    message: text("s"),
                    images: vec![image()]
                }
            )["images"],
            blocks
        );
        assert_eq!(
            encode(
                dialect,
                AgentAction::FollowUp {
                    message: text("f"),
                    images: vec![image()]
                }
            )["images"],
            blocks
        );
    }
}

#[test]
fn dialect_specific_commands() {
    let (omp, pi) = (Omp.dialect(), Pi.dialect());

    assert_eq!(
        encode(omp, AgentAction::RefreshCommands)["type"],
        "get_available_commands"
    );
    assert_eq!(encode(pi, AgentAction::RefreshCommands)["type"], "get_commands");
    assert_eq!(encode(pi, AgentAction::Clone)["type"], "clone");

    let context = EncodeContext {
        sessions_dir: Path::new("/nonexistent"),
    };
    let clone = omp.encode(&AgentAction::Clone, "req-1", &context).unwrap_err();
    assert_eq!(clone.code(), IpcErrorCode::Unsupported);

    assert_eq!(
        omp.history_request(Some("c-1"), "req-2"),
        json!({ "id": "req-2", "type": "get_messages_page", "cursor": "c-1", "limit": 256 })
    );
    assert_eq!(
        pi.history_request(None, "req-2"),
        json!({ "id": "req-2", "type": "get_messages" })
    );
    assert!(omp.history_paged() && !pi.history_paged());

    assert_eq!(omp.handshake(), Handshake::ReadyFrame);
    assert_eq!(pi.handshake(), Handshake::Probe);
    assert_eq!(
        omp.after_handshake(),
        [json!({ "type": "set_event_filter", "events": null, "messageUpdates": "delta" })]
    );
    assert!(pi.after_handshake().is_empty());
    assert_eq!(
        pi.encode_bash("ls", "req-3"),
        json!({ "id": "req-3", "type": "bash", "command": "ls" })
    );
}

#[test]
fn switch_session_sends_the_resolved_session_file() {
    let sessions = tempfile::tempdir().unwrap();
    let folder = sessions.path().join("--home-demo-Projects-demo--");
    std::fs::create_dir_all(folder.join("2026-10-09_abc123")).unwrap();
    std::fs::write(folder.join("2026-10-09T10-00-00_abc123.jsonl"), "{}\n").unwrap();
    std::fs::write(folder.join("2026-10-09T10-00-00_abc1234.jsonl"), "{}\n").unwrap();

    let context = EncodeContext { sessions_dir: &folder };
    let action = AgentAction::SwitchSession {
        session: SessionRef {
            harness: HarnessId::Pi,
            id: text("abc123"),
        },
    };

    let frame = Pi.dialect().encode(&action, "req-1", &context).unwrap();

    let expected = folder.join("2026-10-09T10-00-00_abc123.jsonl");
    assert_eq!(frame["type"], "switch_session");
    assert_eq!(frame["sessionPath"], expected.to_string_lossy().as_ref());
}

#[test]
fn ui_responses_carry_value_confirmed_or_cancelled() {
    let response = |outcome| UiResponse {
        id: text("u1"),
        outcome,
    };
    let encode = |outcome| Omp.dialect().encode_ui_response(&response(outcome));

    assert_eq!(
        encode(UiOutcome::Value { value: text("Approve") }),
        json!({ "type": "extension_ui_response", "id": "u1", "value": "Approve" })
    );
    assert_eq!(
        encode(UiOutcome::Confirmed { confirmed: true }),
        json!({ "type": "extension_ui_response", "id": "u1", "confirmed": true })
    );
    assert_eq!(
        encode(UiOutcome::Cancelled),
        json!({ "type": "extension_ui_response", "id": "u1", "cancelled": true })
    );
}

fn decode(dialect: &dyn RpcDialect, frame: Value) -> Vec<Inbound> {
    dialect.decode(frame, &mut DecodeState::new("/nonexistent"))
}

fn event(inbound: Vec<Inbound>) -> AgentEvent {
    match inbound.as_slice() {
        [Inbound::Event(event)] => event.clone(),
        other => panic!("expected one event, got {other:?}"),
    }
}

#[test]
fn omp_ready_and_prompt_results_decode() {
    let omp = Omp.dialect();

    let ready = decode(
        omp,
        json!({ "type": "ready", "protocolVersion": 1, "supportedProtocolVersions": [1, 2] }),
    );
    assert_eq!(ready, [Inbound::Ready(Some(1))]);

    let failed = json!({
        "type": "prompt_result", "id": "req-4", "agentInvoked": true, "status": "error", "sessionSettled": true,
        "error": { "message": "overloaded", "retryable": true, "provider": "anthropic", "httpStatus": 529 }
    });
    assert_eq!(
        event(decode(omp, failed)),
        AgentEvent::PromptResult {
            request_id: Some(text("req-4")),
            status: PromptStatus::Error,
            error: Some(text("overloaded")),
            retryable: true,
        }
    );

    let mut state = DecodeState::new("/nonexistent");
    let local = omp.on_prompt_ack("req-5", Some(&json!({ "agentInvoked": false })), &mut state);
    assert!(matches!(
        local.as_slice(),
        [AgentEvent::PromptResult {
            status: PromptStatus::Handled,
            ..
        }]
    ));
    assert!(omp.on_prompt_ack("req-6", None, &mut state).is_empty());
}

#[test]
fn session_events_decode_to_contract_events() {
    let omp = Omp.dialect();
    let cases = [
        (
            json!({ "type": "auto_compaction_start", "reason": "threshold", "action": "context-full" }),
            AgentEvent::Compaction {
                phase: text("start"),
                reason: Some(text("threshold")),
                error: None,
            },
        ),
        (
            json!({ "type": "compaction_end", "reason": "manual", "aborted": false, "willRetry": false, "errorMessage": "boom" }),
            AgentEvent::Compaction {
                phase: text("end"),
                reason: Some(text("manual")),
                error: Some(text("boom")),
            },
        ),
        (
            json!({ "type": "thinking_level_changed", "thinkingLevel": "high" }),
            AgentEvent::ThinkingChanged { level: text("high") },
        ),
        (
            json!({ "type": "thinking_level_changed", "level": "low" }),
            AgentEvent::ThinkingChanged { level: text("low") },
        ),
        (
            json!({ "type": "queue_update", "steering": ["a"], "followUp": [] }),
            AgentEvent::QueueUpdate {
                steering: vec![text("a")],
                follow_up: vec![],
            },
        ),
        (
            json!({ "type": "command_output", "text": "usage: …" }),
            AgentEvent::CommandOutput {
                text: text("usage: …")
            },
        ),
        (
            json!({ "type": "session_info_update", "title": "Fix parser", "sessionId": "s" }),
            AgentEvent::Title {
                title: text("Fix parser"),
            },
        ),
        (
            json!({ "type": "brand_new_event" }),
            AgentEvent::Unknown {
                event_type: text("brand_new_event"),
            },
        ),
    ];

    for (frame, expected) in cases {
        assert_eq!(event(decode(omp, frame)), expected);
    }

    assert!(decode(omp, json!({ "type": "subagent_progress", "payload": {} })).is_empty());
    assert!(decode(omp, json!({ "type": "message_update", "assistantMessageEvent": { "type": "toolcall_delta", "contentIndex": 0, "delta": "{" } })).is_empty());
}

#[test]
fn extension_errors_show_only_the_file_name() {
    let frame = json!({
        "type": "extension_error", "extensionPath": "/home/demo/.omp/agent/extensions/guard.ts",
        "event": "tool_call", "error": "denied"
    });

    let AgentEvent::Notice { message, .. } = event(decode(Omp.dialect(), frame)) else {
        panic!("extension errors are notices");
    };
    assert_eq!(message, "guard.ts failed: denied");
}

#[test]
fn select_dialogs_align_option_details_and_ask_is_cancelled_automatically() {
    let omp = Omp.dialect();
    let select = json!({
        "type": "extension_ui_request", "id": "s1", "method": "select", "title": "Pick",
        "options": ["a", "b", "c"], "optionDetails": [{ "description": "first" }], "timeout": 1000
    });

    let decoded = decode(omp, select);
    let [Inbound::UiRequest(request)] = decoded.as_slice() else {
        panic!("a select is a dialog");
    };
    assert_eq!(
        request.kind,
        UiRequestKind::Select {
            title: text("Pick"),
            options: vec![text("a"), text("b"), text("c")],
            details: vec![Some(text("first")), None, None],
            timeout_ms: Some(1000),
        }
    );

    let ask = json!({ "type": "extension_ui_request", "id": "a1", "method": "ask", "questions": [] });
    assert_eq!(
        decode(omp, ask),
        [Inbound::AutoReply(
            json!({ "type": "extension_ui_response", "id": "a1", "cancelled": true })
        )]
    );
}

#[test]
fn pi_prompts_wait_for_agent_settled_unless_handled() {
    let pi = Pi.dialect();
    let mut state = DecodeState::new("/nonexistent");

    let handled = pi.on_prompt_ack("req-1", Some(&json!({ "disposition": "handled" })), &mut state);
    assert!(matches!(
        handled.as_slice(),
        [AgentEvent::PromptResult {
            status: PromptStatus::Handled,
            ..
        }]
    ));

    assert!(
        pi.on_prompt_ack("req-2", Some(&json!({ "disposition": "started" })), &mut state)
            .is_empty()
    );
    assert!(
        pi.on_prompt_ack("req-3", Some(&json!({ "disposition": "queued" })), &mut state)
            .is_empty()
    );

    let settled = pi.decode(json!({ "type": "agent_settled", "aborted": true }), &mut state);
    let statuses: Vec<(Option<String>, PromptStatus)> = settled
        .iter()
        .filter_map(|inbound| match inbound {
            Inbound::Event(AgentEvent::PromptResult { request_id, status, .. }) => Some((request_id.clone(), *status)),
            _ => None,
        })
        .collect();
    assert_eq!(
        statuses,
        [
            (Some(text("req-2")), PromptStatus::Aborted),
            (Some(text("req-3")), PromptStatus::Aborted)
        ]
    );
    assert!(matches!(
        settled.last(),
        Some(Inbound::Event(AgentEvent::Settled { aborted: Some(true) }))
    ));

    // Nothing is left to report on the next settle.
    let again = pi.decode(json!({ "type": "agent_settled", "aborted": false }), &mut state);
    assert_eq!(again.len(), 1);
}
