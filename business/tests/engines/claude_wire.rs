//! Claude Code's stream-json frames (`claude_stream::wire`): every recorded frame decodes, the
//! fields the chat reads come out as recorded, and frames this app does not know are ignored. The
//! encoders are in `claude_wire_encode.rs`.

use crate::claude_recorded;

use std::collections::HashSet;

use claude_recorded::{Dir, FILES, frames};
use harness_business::claude_stream::wire::control::{
    CancelReply, ContextUsageReply, InitializeReply, InterruptReply, ModelsReply, SessionCostReply,
};
use harness_business::claude_stream::wire::messages::{ContentBlock, Delta, StreamEvent};
use harness_business::claude_stream::wire::{CliRequest, Inbound, LifecycleState, System, decode, decode_value};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn decoded(file: &str) -> Vec<Inbound> {
    frames(file, Dir::Out)
        .into_iter()
        .map(|frame| decode_value(frame).unwrap_or_else(|error| panic!("{file}: {error:?}")))
        .collect()
}

#[test]
fn every_recorded_frame_decodes_to_a_known_frame() {
    for file in FILES {
        for frame in decoded(file) {
            match &frame {
                Inbound::Unknown { frame_type } => panic!("{file}: unknown frame {frame_type}"),
                Inbound::System(system) => {
                    assert!(!matches!(system.kind, System::Unknown { .. }), "{file}: {system:?}");
                }
                Inbound::ControlRequest(request) => {
                    assert!(
                        !matches!(request.request, CliRequest::Unknown { .. }),
                        "{file}: {request:?}"
                    );
                }
                _ => {}
            }
        }
    }
}

#[test]
fn a_result_carries_the_turn_outcome_and_the_process_totals() {
    let results: Vec<_> = decoded("basic.jsonl")
        .into_iter()
        .filter_map(|frame| match frame {
            Inbound::Result(result) => Some(result),
            _ => None,
        })
        .collect();

    assert_eq!(results.len(), 2);

    let second = &results[1];
    assert!(second.succeeded());
    assert_eq!(second.num_turns, 1);
    assert_eq!(second.result.as_deref(), Some("ping"));
    assert_eq!(second.total_cost_usd, Some(0.324674));
    assert_eq!(second.prompts(), ["903006b7-f150-4d0c-81c8-0bdd54153a00"]);

    let opus = &second.model_usage["claude-opus-5-5"];
    assert_eq!((opus.input_tokens, opus.output_tokens), (4, 8));
    assert_eq!(
        (opus.cache_read_input_tokens, opus.cache_creation_input_tokens),
        (27090, 39885)
    );
    assert_eq!(opus.context_window, Some(1_000_000));

    let usage = second.usage.as_ref().unwrap();
    assert_eq!(usage.iterations.len(), 1);
    assert_eq!(usage.iterations[0].total(), 2 + 4 + 14790 + 19109);
}

#[test]
fn errors_interruptions_and_quiet_turns_read_as_recorded() {
    let result = |file: &str, nth: usize| {
        decoded(file)
            .into_iter()
            .filter_map(|frame| match frame {
                Inbound::Result(result) => Some(result),
                _ => None,
            })
            .nth(nth)
            .unwrap()
    };

    let nologin = result("nologin.jsonl", 0);
    assert_eq!((nologin.subtype.as_str(), nologin.is_error), ("success", true));
    assert_eq!(nologin.result.as_deref(), Some("Not logged in · Please run /login"));
    assert!(nologin.model_usage.is_empty());

    let interrupted = result("interrupt.jsonl", 0);
    assert_eq!(interrupted.subtype, "error_during_execution");
    assert_eq!(interrupted.terminal_reason.as_deref(), Some("aborted_streaming"));
    assert_eq!(interrupted.errors.len(), 1);

    let compacted = result("compact.jsonl", 0);
    assert_eq!(
        (compacted.num_turns, compacted.local_command.as_deref()),
        (0, Some("compact"))
    );

    let injected = result("bash.jsonl", 1);
    assert_eq!(injected.num_turns, 0);
    assert!(injected.usage.unwrap().iterations.is_empty());
}

#[test]
fn the_nologin_reply_is_a_synthetic_api_error_message() {
    let assistant = decoded("nologin.jsonl")
        .into_iter()
        .find_map(|frame| match frame {
            Inbound::Assistant(assistant) => Some(assistant),
            _ => None,
        })
        .unwrap();

    assert!(assistant.is_api_error_message);
    assert_eq!(assistant.error.as_deref(), Some("authentication_failed"));
    assert_eq!(assistant.message.model.as_deref(), Some("<synthetic>"));
}

#[test]
fn streamed_blocks_and_their_deltas_read_as_recorded() {
    let events: Vec<StreamEvent> = decoded("tools.jsonl")
        .into_iter()
        .filter_map(|frame| match frame {
            Inbound::StreamEvent(stream) => Some(stream.event),
            _ => None,
        })
        .collect();

    let first_tool = events.iter().find_map(|event| match event {
        StreamEvent::ContentBlockStart {
            content_block: ContentBlock::ToolUse { id, name, .. },
            ..
        } => Some((id.clone(), name.clone())),
        _ => None,
    });
    assert_eq!(
        first_tool,
        Some(("toolu_01ATYkU9KWcM1vnvdzgqt7b2".to_owned(), "Read".to_owned()))
    );

    let input: String = events
        .iter()
        .take_while(|event| !matches!(event, StreamEvent::ContentBlockStop { .. }))
        .filter_map(|event| match event {
            StreamEvent::ContentBlockDelta {
                delta: Delta::InputJsonDelta { partial_json },
                ..
            } => Some(partial_json.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        serde_json::from_str::<Value>(&input).unwrap(),
        json!({ "file_path": "/home/user/project/notes.txt" })
    );

    assert!(events.iter().any(|event| matches!(
        event,
        StreamEvent::ContentBlockDelta {
            delta: Delta::Other,
            ..
        }
    )));
    assert!(events.iter().any(|event| matches!(event, StreamEvent::MessageStop)));
}

#[test]
fn permission_prompts_and_hook_callbacks_carry_what_the_dialogs_need() {
    let requests: Vec<_> = decoded("tools.jsonl")
        .into_iter()
        .filter_map(|frame| match frame {
            Inbound::ControlRequest(request) => Some(request),
            _ => None,
        })
        .collect();

    let CliRequest::HookCallback(hook) = &requests[0].request else {
        panic!("{:?}", requests[0]);
    };
    assert_eq!(hook.callback_id, "askall");
    assert_eq!(hook.input["tool_name"], "Read");

    let CliRequest::CanUseTool(ask) = &requests[1].request else {
        panic!("{:?}", requests[1]);
    };
    assert_eq!(requests[1].request_id, "f9c23b59-dafb-4bcb-aa8f-fd0d3dd816c9");
    assert_eq!(ask.tool_name, "Read");
    assert_eq!(ask.description.as_deref(), Some("notes.txt"));
    assert_eq!(ask.decision_reason_type.as_deref(), Some("hook"));
    assert_eq!(ask.tool_use_id, "toolu_01ATYkU9KWcM1vnvdzgqt7b2");

    let question = decoded("question.jsonl")
        .into_iter()
        .find_map(|frame| match frame {
            Inbound::ControlRequest(request) => Some(request),
            _ => None,
        })
        .unwrap();
    let CliRequest::CanUseTool(ask) = question.request else {
        panic!("{question:?}");
    };
    assert_eq!(ask.tool_name, "AskUserQuestion");
    assert!(ask.requires_user_interaction);
    assert_eq!(ask.input["questions"][0]["multiSelect"], true);
}

#[test]
fn system_frames_read_as_recorded() {
    let systems: Vec<System> = FILES
        .iter()
        .flat_map(|file| decoded(file))
        .filter_map(|frame| match frame {
            Inbound::System(system) => Some(system.kind),
            _ => None,
        })
        .collect();

    let init = systems
        .iter()
        .find_map(|system| match system {
            System::Init(init) => Some(init),
            _ => None,
        })
        .unwrap();
    assert_eq!(init.model, "claude-opus-5-5");
    assert_eq!(init.permission_mode.as_deref(), Some("default"));
    assert_eq!(init.api_key_source.as_deref(), Some("none"));
    assert!(init.capabilities.iter().any(|flag| flag == "interrupt_receipt_v1"));

    let boundary = systems
        .iter()
        .find_map(|system| match system {
            System::CompactBoundary(boundary) => Some(&boundary.compact_metadata),
            _ => None,
        })
        .unwrap();
    assert_eq!(boundary.trigger.as_deref(), Some("manual"));
    assert_eq!((boundary.pre_tokens, boundary.post_tokens), (Some(33905), Some(10129)));

    let steered = systems.iter().any(|system| match system {
        System::Status(status) => status.user_message_uuids.len() == 2,
        _ => false,
    });
    assert!(steered, "queue2 serves the prompt and the steering in one request");

    assert!(
        systems.iter().any(
            |system| matches!(system, System::TaskStarted(task) if task.task_type.as_deref() == Some("local_bash"))
        )
    );
    assert!(
        systems
            .iter()
            .any(|system| matches!(system, System::SessionTitleChanged { title } if title == "Recorded rename"))
    );
    assert!(
        systems
            .iter()
            .any(|system| matches!(system, System::CommandsChanged { commands } if commands.len() == 3))
    );
    assert!(
        systems
            .iter()
            .any(|system| matches!(system, System::ThinkingTokens { estimated_tokens: 50 }))
    );
}

#[test]
fn the_command_queue_reports_every_state() {
    let states: HashSet<String> = decoded("queue2.jsonl")
        .into_iter()
        .filter_map(|frame| match frame {
            Inbound::CommandLifecycle(lifecycle) => Some(format!("{:?}", lifecycle.state)),
            _ => None,
        })
        .collect();

    let expected: HashSet<String> = [
        LifecycleState::Queued,
        LifecycleState::Started,
        LifecycleState::Completed,
        LifecycleState::Cancelled,
    ]
    .iter()
    .map(|state| format!("{state:?}"))
    .collect();
    assert_eq!(states, expected);
}

#[test]
fn control_replies_parse_into_their_payloads() {
    let replies = |file: &str| -> Vec<_> {
        decoded(file)
            .into_iter()
            .filter_map(|frame| match frame {
                Inbound::ControlResponse(response) => Some(response),
                _ => None,
            })
            .collect()
    };

    let basic = replies("basic.jsonl");
    let initialize: InitializeReply = basic[0].parse().unwrap();
    assert_eq!(initialize.commands.len(), 3);
    assert_eq!(initialize.models[0].resolved_model.as_deref(), Some("claude-opus-5-5"));
    assert_eq!(initialize.current_permission_mode.as_deref(), Some("default"));

    let context: ContextUsageReply = basic[1].parse().unwrap();
    assert_eq!((context.total_tokens, context.max_tokens), (33078, 1_000_000));
    assert_eq!(context.percentage, Some(3.0));

    let cost: SessionCostReply = basic[2].parse().unwrap();
    assert!(cost.text.starts_with("Total cost:"));

    let controls = replies("controls.jsonl");
    let models: ModelsReply = controls[1].parse().unwrap();
    assert!(models.models.iter().any(|model| model.value == "opus"));
    assert_eq!(
        controls[2].outcome,
        Ok(json!({})),
        "set_model answers with a bare success"
    );

    let interrupt: InterruptReply = replies("interrupt.jsonl")[1].parse().unwrap();
    assert!(interrupt.still_queued.is_empty());

    let cancel: CancelReply = replies("queue2.jsonl")[1].parse().unwrap();
    assert!(cancel.cancelled);
}

#[test]
fn unknown_frames_subtypes_and_fields_are_ignored() {
    let unknown = decode(r#"{"type":"brand_new","x":1}"#).unwrap();
    assert_eq!(
        unknown,
        Inbound::Unknown {
            frame_type: "brand_new".to_owned()
        }
    );

    let Inbound::System(system) = decode(r#"{"type":"system","subtype":"weather","session_id":"s"}"#).unwrap() else {
        panic!();
    };
    assert_eq!(
        system.kind,
        System::Unknown {
            subtype: "weather".to_owned()
        }
    );
    assert_eq!(system.session_id.as_deref(), Some("s"));

    let Inbound::ControlRequest(request) =
        decode(r#"{"type":"control_request","request_id":"r","request":{"subtype":"mcp_message","x":{}}}"#).unwrap()
    else {
        panic!();
    };
    assert_eq!(
        request.request,
        CliRequest::Unknown {
            subtype: "mcp_message".to_owned()
        }
    );

    let status =
        decode(r#"{"type":"system","subtype":"status","status":"requesting","novel":[1,2],"user_message_uuids":null}"#);
    assert!(matches!(status, Ok(Inbound::System(_))));

    let stream = decode(r#"{"type":"stream_event","event":{"type":"ping"}}"#).unwrap();
    assert!(matches!(stream, Inbound::StreamEvent(frame) if frame.event == StreamEvent::Other));

    assert_eq!(decode(r#"{"type":"keep_alive"}"#).unwrap(), Inbound::KeepAlive);
}

#[test]
fn broken_lines_are_errors_without_their_content() {
    let error = decode("{not json secret-value").unwrap_err();
    assert_eq!(error.frame_type, None);
    assert!(!error.error.contains("secret-value"));

    let error = decode(r#"{"type":"result","errors":"secret-value"}"#).unwrap_err();
    assert_eq!(error.frame_type.as_deref(), Some("result"));
    assert!(!error.error.contains("secret-value"));

    let error = decode(r#"{"type":"system","subtype":"init","tools":"secret-value"}"#).unwrap_err();
    assert_eq!(error.frame_type.as_deref(), Some("system/init"));
}
