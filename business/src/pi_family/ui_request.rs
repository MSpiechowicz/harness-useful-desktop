//! The extension UI sub-protocol (`extension_ui_request` / `extension_ui_response`).
//!
//! Dialog methods (`select`, `confirm`, `input`, `editor`) become [`UiRequest`]s the user answers;
//! fire-and-forget methods become events. OMP's `cancel` closes an open dialog. `ask` is only sent
//! to hosts that opt in to ask dialogs, which the desktop never does, so one that arrives anyway
//! (like any unknown method) is answered with `cancelled` so the agent never waits forever.

use serde_json::{Value, json};

use super::normalize::{plain_text, string, strings, unsigned};
use super::rpc::Inbound;
use crate::model::{AgentEvent, NoticeLevel, UiOutcome, UiPurpose, UiRequest, UiRequestKind, UiResponse};
use crate::security::redact_text;

/// The answer OMP's `ask` tool adds to every question (`OTHER_OPTION` in `tools/ask.ts`). A question can
/// not leave it out and an OMP approval never has it, so it tells them apart when the labels look alike.
const ASK_OTHER_OPTION: &str = "Other (type your own)";

/// How an OMP tool approval's title starts (`tools/approval.ts`).
const APPROVAL_TITLE: &str = "Allow tool: ";

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
