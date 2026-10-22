//! Notices from the CLI's own output: local command output (`<local-command-stdout>` user frames,
//! `system/local_command_output`), `system/notification`, and the interruption marker. Texts are
//! stripped of terminal escapes and redacted.

use super::Out;
use crate::claude_stream::wire::system::{ApiRetry, Notification};
use crate::model::{AgentEvent, NoticeLevel};
use crate::pi_family::normalize::plain_text;
use crate::security::redact_text;

/// How the CLI marks a turn the user interrupted (also `… for tool use]`).
const INTERRUPTED: &str = "[Request interrupted by user";

/// The tags local command output comes in, with the level each shows at.
const COMMAND_TAGS: [(&str, NoticeLevel); 2] = [
    ("local-command-stdout", NoticeLevel::Info),
    ("local-command-stderr", NoticeLevel::Warning),
];

/// True for the user frame an interrupt leaves.
pub(super) fn is_interruption(text: &str) -> bool {
    text.trim_start().starts_with(INTERRUPTED)
}

/// A user frame holding local command output: each tagged part as a notice.
pub(super) fn local_command(text: &str, out: &mut Out) {
    for (tag, level) in COMMAND_TAGS {
        let open = format!("<{tag}>");
        let close = format!("</{tag}>");

        let Some(start) = text.find(&open) else {
            continue;
        };

        let body = &text[start + open.len()..];
        let body = body.find(&close).map_or(body, |end| &body[..end]);

        notice(level, body, out);
    }
}

/// `system/local_command_output`.
pub(super) fn command_output(content: &str, out: &mut Out) {
    notice(NoticeLevel::Info, content, out);
}

/// `system/notification`: urgent ones as warnings.
pub(super) fn notification(notification: &Notification, out: &mut Out) {
    let level = match notification.priority.as_deref() {
        Some("high" | "immediate") => NoticeLevel::Warning,
        _ => NoticeLevel::Info,
    };

    notice(level, &notification.text, out);
}

fn notice(level: NoticeLevel, text: &str, out: &mut Out) {
    let message = redact_text(plain_text(text).trim());

    if message.is_empty() {
        return;
    }

    out.event(AgentEvent::Notice { level, message });
}

/// Why a model call is tried again: its error kind in words, with the HTTP status.
pub(super) fn retry_text(retry: &ApiRetry) -> Option<String> {
    let kind = retry.error.as_deref().map(|kind| kind.replace('_', " "));

    match (kind, retry.error_status) {
        (Some(kind), Some(status)) => Some(format!("{kind} ({status})")),
        (Some(kind), None) => Some(kind),
        (None, Some(status)) => Some(format!("HTTP {status}")),
        (None, None) => None,
    }
}

/// The first line of `text`, redacted and trimmed.
pub(super) fn first_line(text: &str) -> String {
    redact_text(text).lines().next().unwrap_or_default().trim().to_owned()
}
