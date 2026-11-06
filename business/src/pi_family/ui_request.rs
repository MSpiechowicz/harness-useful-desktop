//! The extension UI sub-protocol (`extension_ui_request` / `extension_ui_response`).
//!
//! Dialog methods (`select`, `confirm`, `input`, `editor`) become [`UiRequest`]s the user answers;
//! fire-and-forget methods become events. OMP's `cancel` closes an open dialog. `ask` carries every
//! question of a call at once, because the OMP dialect opts in with `set_ask_dialog`. An unknown
//! method is answered with `cancelled` so the agent never waits forever.

use serde_json::{Value, json};

use super::normalize::{plain_text, string, strings, unsigned};
use super::rpc::Inbound;
use crate::model::{
    AgentEvent, AskAnswer, AskInput, AskOption, AskQuestion, NoticeLevel, UiOutcome, UiPurpose, UiRequest,
    UiRequestKind, UiResponse,
};
use crate::security::redact_text;

/// The answer OMP's `ask` tool adds to every question (`OTHER_OPTION` in `tools/ask.ts`). A question can
/// not leave it out and an OMP approval never has it, so it tells them apart when the labels look alike.
const ASK_OTHER_OPTION: &str = "Other (type your own)";

/// How an OMP tool approval's title starts (`tools/approval.ts`).
const APPROVAL_TITLE: &str = "Allow tool: ";

/// How the header of an OMP ratchet approval starts, and its options.
const RATCHET_HEADER: &str = "ratchet · ";
const RATCHET_OPTIONS: [&str; 3] = ["Approve", "Revise", "Abort"];

/// Placement used when a widget does not name one.
const DEFAULT_WIDGET_PLACEMENT: &str = "aboveEditor";

/// Decodes an `extension_ui_request` frame.
pub fn decode_ui_request(frame: &Value) -> Vec<Inbound> {
    let Some(id) = string(frame, "id") else {
        return Vec::new();
    };

    let method = frame.get("method").and_then(Value::as_str).unwrap_or_default();

    let dialog = |kind: UiRequestKind| vec![Inbound::UiRequest(UiRequest::new(id.clone(), kind))];
    let event = |event: AgentEvent| vec![Inbound::Event(event)];
    // An approval's title says what it covers, so a mask there could hide it (or the tool it names).
    let shown_title = || {
        let title = string(frame, "title").unwrap_or_default();

        // OMP puts the model's content in `Allow tool:` titles: stripping an escape there can cut what follows it.
        if title.starts_with(APPROVAL_TITLE) {
            title
        } else {
            plain_text(&title)
        }
    };
    let title = || redact_text(&shown_title());
    let timeout_ms = || unsigned(frame, "timeout");

    match method {
        "select" => {
            let options = strings(frame, "options");
            let purpose = options
                .iter()
                .any(|option| option == ASK_OTHER_OPTION)
                .then_some(UiPurpose::Question);
            let kind = UiRequestKind::Select {
                title: if purpose.is_some() { title() } else { shown_title() },
                options,
                details: option_details(frame),
                timeout_ms: timeout_ms(),
            };

            vec![Inbound::UiRequest(UiRequest {
                purpose,
                ..UiRequest::new(id.clone(), kind)
            })]
        }
        "ask"
            if frame
                .get("questions")
                .and_then(Value::as_array)
                .is_some_and(|list| !list.is_empty()) =>
        {
            vec![Inbound::UiRequest(UiRequest {
                purpose: Some(UiPurpose::Question),
                ..UiRequest::new(
                    id.clone(),
                    UiRequestKind::Questions {
                        questions: ask_questions(frame),
                        timeout_ms: timeout_ms(),
                    },
                )
            })]
        }
        "confirm" => dialog(UiRequestKind::Confirm {
            title: shown_title(),
            message: string(frame, "message").unwrap_or_default(),
            timeout_ms: timeout_ms(),
        }),
        "input" => {
            let kind = UiRequestKind::Input {
                title: title(),
                placeholder: string(frame, "placeholder").map(|text| redact_text(&text)),
                timeout_ms: timeout_ms(),
            };

            vec![Inbound::UiRequest(UiRequest {
                secret: frame.get("secret").and_then(Value::as_bool).filter(|secret| *secret),
                ..UiRequest::new(id.clone(), kind)
            })]
        }
        "editor" => dialog(UiRequestKind::Editor {
            title: title(),
            prefill: string(frame, "prefill"),
        }),
        "cancel" => match string(frame, "targetId") {
            Some(request_id) => event(AgentEvent::UiCancel { request_id }),
            None => Vec::new(),
        },
        "notify" => event(AgentEvent::Notice {
            level: notice_level(frame.get("notifyType")),
            message: redact_text(&plain_text(&string(frame, "message").unwrap_or_default())),
        }),
        "setStatus" => event(AgentEvent::Status {
            key: string(frame, "statusKey").unwrap_or_default(),
            text: string(frame, "statusText").map(|text| plain_text(&text)),
        }),
        "setWidget" => event(AgentEvent::Widget {
            key: string(frame, "widgetKey").unwrap_or_default(),
            lines: strings(frame, "widgetLines")
                .iter()
                .map(|line| plain_text(line))
                .collect(),
            placement: string(frame, "widgetPlacement").unwrap_or_else(|| DEFAULT_WIDGET_PLACEMENT.to_owned()),
        }),
        "setTitle" => event(AgentEvent::Title { title: title() }),
        "set_editor_text" => event(AgentEvent::EditorText {
            text: string(frame, "text").unwrap_or_default(),
        }),
        "open_url" => match string(frame, "url") {
            Some(url) => event(AgentEvent::OpenUrl {
                url,
                instructions: string(frame, "instructions"),
            }),
            None => Vec::new(),
        },
        _ => vec![Inbound::AutoReply(cancelled_response(&id))],
    }
}

/// The questions of an `ask`. Their text is redacted, their labels stay exact: OMP refuses one it did not offer.
fn ask_questions(frame: &Value) -> Vec<AskQuestion> {
    let shown = |text: &str| redact_text(&plain_text(text));

    frame
        .get("questions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|question| {
            let header = string(question, "header");
            let text = string(question, "question").unwrap_or_default();
            let options: Vec<AskOption> = question
                .get("options")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|option| AskOption {
                    // Sent back as it came: OMP refuses a label it did not offer.
                    label: string(option, "label").unwrap_or_default(),
                    description: string(option, "description").map(|text| shown(&text)),
                })
                .collect();

            AskQuestion {
                // What a ratchet approval covers stays whole, as on any approval: a mask could hide part of it.
                question: if is_ratchet_approval(header.as_deref(), &options) {
                    text
                } else {
                    shown(&text)
                },
                header: header.as_deref().map(shown),
                options,
                id: string(question, "id").unwrap_or_default(),
                multi: question.get("multi").and_then(Value::as_bool).unwrap_or(false),
                custom: true,
                input: AskInput::Text,
                recommended: unsigned(question, "recommended"),
            }
        })
        .collect()
}

/// True for the shape of OMP's ratchet approval (`ratchet/prelude-definition.ts`). A model can write the same, so
/// this only leaves its own text unmasked: it does not make the question count as an approval.
fn is_ratchet_approval(header: Option<&str>, options: &[AskOption]) -> bool {
    header.is_some_and(|header| header.starts_with(RATCHET_HEADER))
        && options.iter().map(|option| option.label.as_str()).eq(RATCHET_OPTIONS)
}

/// `{id, selectedOptions, customInput}`, leaving out a missing `customInput`: OMP refuses a null.
fn ask_answer(answer: &AskAnswer) -> Value {
    let mut encoded = json!({ "id": answer.id, "selectedOptions": answer.selected_options });

    if let Some(custom) = &answer.custom_input {
        encoded["customInput"] = json!(custom);
    }

    encoded
}

/// `optionDetails[i].description`, aligned with `options`.
fn option_details(frame: &Value) -> Vec<Option<String>> {
    let count = frame.get("options").and_then(Value::as_array).map_or(0, Vec::len);
    let details = frame.get("optionDetails").and_then(Value::as_array);

    (0..count)
        .map(|index| {
            details
                .and_then(|details| details.get(index))
                .and_then(|detail| string(detail, "description"))
                .map(|detail| redact_text(&detail))
        })
        .collect()
}

/// `info` (default), `warning` or `error`.
pub(crate) fn notice_level(value: Option<&Value>) -> NoticeLevel {
    match value.and_then(Value::as_str) {
        Some("warning") => NoticeLevel::Warning,
        Some("error") => NoticeLevel::Error,
        _ => NoticeLevel::Info,
    }
}

/// The `extension_ui_response` for the webview's answer.
pub fn encode_ui_response(response: &UiResponse) -> Value {
    match &response.outcome {
        UiOutcome::Value { value } => json!({ "type": "extension_ui_response", "id": response.id, "value": value }),
        UiOutcome::Confirmed { confirmed } => {
            json!({ "type": "extension_ui_response", "id": response.id, "confirmed": confirmed })
        }
        UiOutcome::Answers { answers } => {
            let answers: Vec<Value> = answers.iter().map(ask_answer).collect();

            json!({ "type": "extension_ui_response", "id": response.id, "answers": answers })
        }
        UiOutcome::Cancelled => cancelled_response(&response.id),
    }
}

/// Dismisses dialog `id`.
pub fn cancelled_response(id: &str) -> Value {
    json!({ "type": "extension_ui_response", "id": id, "cancelled": true })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown_select_title(title: &str) -> String {
        let frame = json!({ "id": "1", "method": "select", "title": title, "options": ["Approve", "Deny"] });

        match decode_ui_request(&frame).pop() {
            Some(Inbound::UiRequest(UiRequest {
                kind: UiRequestKind::Select { title, .. },
                ..
            })) => title,
            _ => panic!("a select is a dialog"),
        }
    }

    /// The questions of an `ask` frame, which is one question dialog.
    fn asked(frame: &Value) -> Vec<AskQuestion> {
        let requests = decode_ui_request(frame);
        let [Inbound::UiRequest(request)] = requests.as_slice() else {
            panic!("an ask is one dialog");
        };
        let UiRequestKind::Questions { questions, .. } = &request.kind else {
            panic!("an ask holds its questions");
        };

        assert_eq!(request.purpose, Some(UiPurpose::Question));

        questions.clone()
    }

    #[test]
    fn an_ask_opens_one_dialog_of_every_question() {
        let frame = json!({
            "id": "a1",
            "method": "ask",
            "questions": [
                { "id": "area", "question": "Which area?", "options": [{ "label": "Mark", "description": "The H" }], "recommended": 0 },
                { "id": "how", "question": "How far?", "options": [{ "label": "A" }, { "label": "B" }], "multi": true },
            ],
        });

        let questions = asked(&frame);

        assert_eq!(questions.len(), 2);
        assert_eq!(questions[0].recommended, Some(0));
        assert_eq!(questions[0].options[0].description.as_deref(), Some("The H"));
        assert!(questions[1].multi);
    }

    #[test]
    fn a_ratchet_approval_keeps_its_stage_and_its_text_unmasked_and_another_question_does_not() {
        let secret = format!("gh{}_{}", "p", "a".repeat(36));
        let ask = |header: &str, options: &[&str]| {
            let options: Vec<Value> = options.iter().map(|label| json!({ "label": label })).collect();
            let frame = json!({ "id": "a", "method": "ask", "questions": [
                { "id": "q", "header": header, "question": format!("Ship?\n\n{secret}"), "options": options },
            ] });
            asked(&frame).swap_remove(0)
        };

        let ratchet = ask("ratchet · plan", &["Approve", "Revise", "Abort"]);
        assert_eq!(ratchet.header.as_deref(), Some("ratchet · plan"));
        assert!(ratchet.question.contains(&secret));

        assert!(!ask("ratchet · plan", &["Approve", "Abort"]).question.contains(&secret));
        assert!(!ask("Scope", &["Approve", "Revise", "Abort"]).question.contains(&secret));
    }

    #[test]
    fn answers_are_held_to_the_size_limit_of_any_other_text() {
        let outcome = |bytes: usize| UiOutcome::Answers {
            answers: vec![AskAnswer {
                id: "0".to_owned(),
                selected_options: vec!["a".repeat(bytes / 2)],
                custom_input: Some("b".repeat(bytes / 2)),
            }],
        };

        assert!(outcome(1024).check_size().is_ok());
        assert!(outcome(crate::model::MAX_ACTION_TEXT_BYTES + 2).check_size().is_err());
    }

    #[test]
    fn answers_go_back_in_the_shape_omp_accepts() {
        let answer = |custom: Option<&str>| AskAnswer {
            id: "area".to_owned(),
            selected_options: if custom.is_some() {
                Vec::new()
            } else {
                vec!["Mark".to_owned()]
            },
            custom_input: custom.map(str::to_owned),
        };
        let response = UiResponse {
            id: "a1".to_owned(),
            outcome: UiOutcome::Answers {
                answers: vec![answer(None), answer(Some("Mine"))],
            },
        };

        assert_eq!(
            encode_ui_response(&response),
            json!({
                "type": "extension_ui_response",
                "id": "a1",
                "answers": [
                    { "id": "area", "selectedOptions": ["Mark"] },
                    { "id": "area", "selectedOptions": [], "customInput": "Mine" },
                ],
            })
        );
    }

    #[test]
    fn an_escape_in_an_approval_title_cannot_hide_what_follows() {
        let title = "Allow tool: write\nContent:\nalias ll\n\u{1b}]0;\ntouch /tmp/marker";

        assert_eq!(shown_select_title(title), title);
    }

    #[test]
    fn an_approval_title_keeps_the_tool_it_names() {
        let title = "Allow tool: mcp__Acme2Server__getCustomerInvoiceSummaryV2";

        assert_eq!(shown_select_title(title), title);
    }

    fn select_purpose(options: &[&str]) -> Option<UiPurpose> {
        let frame = json!({ "id": "1", "method": "select", "title": "Allow tool: bash", "options": options });

        match decode_ui_request(&frame).pop() {
            Some(Inbound::UiRequest(request)) => request.purpose,
            _ => panic!("a select is a dialog"),
        }
    }

    #[test]
    fn a_question_from_the_ask_tool_is_never_taken_for_an_approval() {
        assert_eq!(
            select_purpose(&["Approve", "Deny", ASK_OTHER_OPTION]),
            Some(UiPurpose::Question)
        );
        assert_eq!(select_purpose(&["Approve", "Deny"]), None);
    }

    #[test]
    fn a_secret_input_stays_secret() {
        let frame = json!({ "id": "1", "method": "input", "title": "Token", "secret": true });
        let requests = decode_ui_request(&frame);
        let [Inbound::UiRequest(request)] = requests.as_slice() else {
            panic!("expected one dialog");
        };

        assert_eq!(request.secret, Some(true));
    }

    #[test]
    fn extension_text_reaches_the_desktop_without_terminal_colors() {
        let frame = json!({
            "id": "n1",
            "method": "notify",
            "notifyType": "warning",
            "message": "\u{1b}[38;5;215mUsage Dashboard · Warning\u{1b}[39m \u{1b}[39mHistory could not be saved.\u{1b}[39m",
        });

        let notice = decode_ui_request(&frame);
        let [Inbound::Event(AgentEvent::Notice { level, message })] = notice.as_slice() else {
            panic!("expected one notice");
        };
        assert_eq!(*level, NoticeLevel::Warning);
        assert_eq!(message, "Usage Dashboard · Warning History could not be saved.");

        let frame = json!({ "id": "w1", "method": "setWidget", "widgetKey": "usage", "widgetLines": ["\u{1b}[1mtokens\u{1b}[22m 12k"] });
        let widget = decode_ui_request(&frame);
        let [Inbound::Event(AgentEvent::Widget { lines, .. })] = widget.as_slice() else {
            panic!("expected one widget");
        };
        assert_eq!(lines, &["tokens 12k"]);
    }
}
