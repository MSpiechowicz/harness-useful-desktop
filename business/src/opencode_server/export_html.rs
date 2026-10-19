//! A chat transcript as one self-contained HTML page (`exportHtml` of the OpenCode chat). Pure: the
//! caller writes the file.
//!
//! - Every text from the session (title, messages, thinking, tool names, inputs and results,
//!   models, errors) is redacted ([`redact_text`]) and HTML-escaped (`& < > " '`) before it is
//!   placed, and it is only ever placed as element text.
//! - The page holds no script and loads nothing: its Content-Security-Policy allows inline styles
//!   only, so even markup that slipped through could neither run nor fetch.
//! - Messages in order with their time (UTC). Thinking and tool calls (input as JSON, then the
//!   result) are collapsed; failed calls and failed replies are marked.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::time::{Duration, UNIX_EPOCH};

use serde_json::Value;

use crate::agent::rfc3339;
use crate::model::{ChatBlock, ChatMessage, MessageRole};
use crate::security::redact_text;

/// The page's styles (inline: the page loads nothing).
const STYLE: &str = "\
:root{color-scheme:light dark;--fg:#1d1f23;--muted:#6b7079;--line:#d9dce1;--card:#f5f6f8;--accent:#2f6fde;--bad:#c23b3b}
@media (prefers-color-scheme:dark){:root{--fg:#e6e8eb;--muted:#9aa0a8;--line:#30343a;--card:#1c1f24;--accent:#6b9bf0;--bad:#e06a6a}}
body{margin:0 auto;max-width:860px;padding:32px 20px;font:15px/1.55 system-ui,sans-serif;color:var(--fg)}
header{border-bottom:1px solid var(--line);margin-bottom:24px}
h1{font-size:22px;margin:0 0 4px}
.meta,time,.role small{color:var(--muted);font-size:13px}
section{margin:0 0 20px}
.head{display:flex;gap:12px;align-items:baseline;margin-bottom:6px}
.role{font-weight:600}
.user .role{color:var(--accent)}
.text{white-space:pre-wrap;overflow-wrap:anywhere}
details{background:var(--card);border:1px solid var(--line);border-radius:8px;margin:8px 0;padding:6px 12px}
summary{cursor:pointer;color:var(--muted)}
pre{white-space:pre-wrap;overflow-wrap:anywhere;font:13px/1.45 ui-monospace,monospace;margin:6px 0}
h3{font-size:12px;text-transform:uppercase;letter-spacing:.04em;color:var(--muted);margin:10px 0 0}
.failed summary,.error{color:var(--bad)}
.note{color:var(--muted);font-style:italic}
";

/// What the page says about the session.
#[derive(Debug, Clone, Copy)]
pub struct ExportPage<'a> {
    /// The session's title.
    pub title: &'a str,
    /// The harness as shown (`OpenCode`).
    pub harness: &'a str,
    /// The session id.
    pub session_id: &'a str,
    /// When the export was made (`YYYY-MM-DDTHH:MM:SSZ`).
    pub exported_at: &'a str,
}

/// The page for `messages` (oldest first; tool results are shown with their calls).
pub fn render(page: &ExportPage<'_>, messages: &[ChatMessage]) -> String {
    let results: HashMap<&str, &ChatMessage> = messages
        .iter()
        .filter(|message| message.role == MessageRole::ToolResult)
        .filter_map(|message| Some((message.tool_call_id.as_deref()?, message)))
        .collect();

    let title = safe(page.title);
    let mut html = String::with_capacity(16 * 1024);

    html.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    html.push_str(
        "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\">\n",
    );
    html.push_str("<meta name=\"referrer\" content=\"no-referrer\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    let _ = write!(
        html,
        "<title>{title}</title>\n<style>\n{STYLE}</style>\n</head>\n<body>\n"
    );
    let _ = write!(
        html,
        "<header><h1>{title}</h1><p class=\"meta\">{} session {} · exported {}</p></header>\n<main>\n",
        safe(page.harness),
        safe(page.session_id),
        safe(page.exported_at)
    );

    for message in messages {
        if message.role == MessageRole::ToolResult {
            let call = message.tool_call_id.as_deref();
            let paired = call.is_some_and(|call| has_call(messages, call));

            if !paired {
                section(&mut html, message, &results);
            }

            continue;
        }

        section(&mut html, message, &results);
    }

    html.push_str("</main>\n</body>\n</html>\n");
    html
}

/// Escapes `text` for HTML text and attribute values.
pub fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());

    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            other => escaped.push(other),
        }
    }

    escaped
}

/// `text` redacted, then escaped.
fn safe(text: &str) -> String {
    escape(&redact_text(text))
}

fn has_call(messages: &[ChatMessage], id: &str) -> bool {
    messages.iter().any(|message| {
        message
            .blocks
            .iter()
            .any(|block| matches!(block, ChatBlock::ToolCall { id: call, .. } if call == id))
    })
}

/// One message as a `<section>`.
fn section(html: &mut String, message: &ChatMessage, results: &HashMap<&str, &ChatMessage>) {
    let (class, label) = role(message.role);
    let model = message
        .model
        .as_deref()
        .map(|model| format!(" <small>{}</small>", safe(model)));
    let time = message
        .timestamp
        .and_then(|millis| u64::try_from(millis).ok())
        .map(|millis| rfc3339(UNIX_EPOCH + Duration::from_millis(millis)));

    let _ = write!(
        html,
        "<section class=\"{class}\"><div class=\"head\"><span class=\"role\">{label}{}</span>",
        model.unwrap_or_default()
    );
    if let Some(time) = time {
        let _ = write!(html, "<time>{time}</time>");
    }
    html.push_str("</div>\n");

    for block in &message.blocks {
        block_html(html, block, results);
    }

    if let Some(error) = &message.error_message {
        let _ = writeln!(html, "<p class=\"error\">{}</p>", safe(error));
    }

    html.push_str("</section>\n");
}

/// The class and label of a message of `role`.
fn role(role: MessageRole) -> (&'static str, &'static str) {
    match role {
        MessageRole::User => ("user", "You"),
        MessageRole::Assistant => ("assistant", "Assistant"),
        MessageRole::ToolResult => ("tool", "Tool result"),
        MessageRole::Bash => ("bash", "Command"),
        MessageRole::CompactionSummary => ("summary", "Compacted context"),
        MessageRole::BranchSummary => ("summary", "Branch summary"),
        _ => ("other", "Note"),
    }
}

fn block_html(html: &mut String, block: &ChatBlock, results: &HashMap<&str, &ChatMessage>) {
    match block {
        ChatBlock::Text { text } => {
            let _ = writeln!(html, "<div class=\"text\">{}</div>", safe(text));
        }
        ChatBlock::Thinking { text, redacted } => {
            let body = if *redacted || text.is_empty() {
                "<p class=\"note\">The provider withheld this reasoning.</p>".to_owned()
            } else {
                format!("<div class=\"text\">{}</div>", safe(text))
            };
            let _ = writeln!(
                html,
                "<details class=\"thinking\"><summary>Thinking</summary>{body}</details>"
            );
        }
        ChatBlock::ToolCall { id, name, args, intent } => {
            tool_html(html, name, args, intent.as_deref(), results.get(id.as_str()))
        }
        ChatBlock::Image { mime_type } => {
            let _ = writeln!(html, "<p class=\"note\">Image ({})</p>", safe(mime_type));
        }
        ChatBlock::Other { kind } => {
            let _ = writeln!(html, "<p class=\"note\">{} (not shown)</p>", safe(kind));
        }
    }
}

/// A tool call with its input and, once it finished, its result.
fn tool_html(html: &mut String, name: &str, args: &Value, intent: Option<&str>, result: Option<&&ChatMessage>) {
    let failed = result.is_some_and(|result| result.is_error == Some(true));
    let class = if failed { "tool failed" } else { "tool" };
    let intent = intent.map(|intent| format!(" · {}", safe(intent))).unwrap_or_default();
    let input = serde_json::to_string_pretty(args).unwrap_or_default();

    let _ = write!(
        html,
        "<details class=\"{class}\"><summary>{}{intent}{}</summary><h3>Input</h3><pre>{}</pre>",
        safe(name),
        if failed { " · failed" } else { "" },
        safe(&input)
    );

    let output = result.map(|result| {
        result
            .blocks
            .iter()
            .filter_map(|block| match block {
                ChatBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    });

    match output {
        Some(output) => {
            let _ = write!(html, "<h3>Result</h3><pre>{}</pre>", safe(&output));
        }
        None => html.push_str("<p class=\"note\">No result.</p>"),
    }

    html.push_str("</details>\n");
}
