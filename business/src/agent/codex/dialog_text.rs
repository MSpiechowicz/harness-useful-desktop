//! What the approval dialogs of a Codex chat say (`dialogs.rs`): the command, file change or
//! extra permissions in the title, redacted, and the dialog id of a request.

use std::path::Path;

use crate::claude_stream::permissions::{ALLOW_ONCE, ALWAYS_ALLOW, REJECT};
use crate::codex_rpc::wire::RequestId;
use crate::codex_rpc::wire::items::FileUpdateChange;
use crate::codex_rpc::wire::requests::{CommandApproval, PermissionsApproval};
use crate::model::{AgentEvent, UiPurpose, UiRequest, UiRequestKind};
use crate::security::redact_text;

/// The files a file change's title names before it counts the rest.
const NAMED_FILES: usize = 3;

/// The longest subject a title shows, in characters (the rest is counted).
const MAX_SUBJECT_CHARS: usize = 2_000;

/// The dialog of a command: the command (or the host of a network approval) in its title.
pub(super) fn command_dialog(dialog: &str, approval: &CommandApproval, always: bool) -> AgentEvent {
    let host = approval
        .network_approval_context
        .as_ref()
        .map(|context| context.host.as_str())
        .filter(|host| !host.is_empty());

    let label = match approval.kind.as_deref() {
        Some("writeStdin") => "input to the running command",
        _ => "shell",
    };
    let title = match host {
        Some(host) => format!("Allow network access to {}?", one_line(host)),
        None => titled(label, approval.shown_command()),
    };
    let always = always.then_some("Allows this command again until this chat ends");

    approval_dialog(dialog, &title, approval.reason.as_deref(), always)
}

/// The dialog of extra permissions: what they add in its title.
pub(super) fn permissions_dialog(dialog: &str, approval: &PermissionsApproval) -> AgentEvent {
    let mut parts = Vec::new();

    if approval.network() {
        parts.push("network access".to_owned());
    }

    for (access, paths) in [("reading", approval.read_paths()), ("writing", approval.write_paths())] {
        if !paths.is_empty() {
            parts.push(format!("{access} {}", paths.join(", ")));
        }
    }

    let subject = (!parts.is_empty()).then(|| parts.join(", "));
    let title = titled("extra permissions", subject.as_deref());

    approval_dialog(
        dialog,
        &title,
        approval.reason.as_deref(),
        Some("Keeps these permissions until this chat ends"),
    )
}

/// `Allow <label>: <subject>?` on one line, or `Allow <label>?`.
pub(super) fn titled(label: &str, subject: Option<&str>) -> String {
    match subject.map(one_line).filter(|subject| !subject.is_empty()) {
        Some(subject) => format!("Allow {label}: {subject}?"),
        None => format!("Allow {label}?"),
    }
}

/// What a file change asks for: every path of `changes` outside `project` first, then the
/// project's files (relative, the first few and a count of the rest), then the folder `grant_root`
/// it asks to write below.
pub(super) fn file_change_subject(
    changes: &[FileUpdateChange],
    project: &Path,
    grant_root: Option<&str>,
) -> Option<String> {
    let (inside, outside): (Vec<&Path>, Vec<&Path>) =
        changes.iter().map(|change| Path::new(&change.path)).partition(|path| {
            let climbs = path.components().any(|part| part == std::path::Component::ParentDir);
            !climbs && (path.is_relative() || path.starts_with(project))
        });

    let names: Vec<String> = inside
        .iter()
        .map(|path| {
            path.strip_prefix(project)
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let (named, rest) = names.split_at(names.len().min(NAMED_FILES));

    let mut parts = Vec::new();

    if !outside.is_empty() {
        let paths: Vec<String> = outside.iter().map(|path| path.to_string_lossy().into_owned()).collect();
        parts.push(format!("outside the project {}", paths.join(", ")));
    }

    match rest.len() {
        0 if named.is_empty() => {}
        0 => parts.push(named.join(", ")),
        more => parts.push(format!("{} and {more} more", named.join(", "))),
    }

    if let Some(root) = grant_root {
        parts.push(format!("writing anywhere below {root}"));
    }

    (!parts.is_empty()).then(|| parts.join(", "))
}

/// The id of the dialog of Codex's request `id`, by its type: `n-<number>` or `s-<text>` (a `:`
/// written `%3A`, as question steps add `:<n>`).
pub(super) fn dialog_id(id: &RequestId) -> String {
    match id {
        RequestId::Number(number) => format!("n-{number}"),
        RequestId::Text(text) => format!("s-{}", text.replace('%', "%25").replace(':', "%3A")),
    }
}

/// An approval `select` as `dialog`, titled `title`, with `reason` as the detail of `Allow once`
/// and `always` as the one of `Always allow` (left out without it).
pub(super) fn approval_dialog(dialog: &str, title: &str, reason: Option<&str>, always: Option<&str>) -> AgentEvent {
    let mut options = vec![ALLOW_ONCE.to_owned()];
    let mut details = vec![reason.map(redact_text)];

    if let Some(always) = always {
        options.push(ALWAYS_ALLOW.to_owned());
        details.push(Some(always.to_owned()));
    }

    options.push(REJECT.to_owned());
    details.push(None);

    let kind = UiRequestKind::Select {
        title: redact_text(title),
        options,
        details,
        timeout_ms: None,
    };

    AgentEvent::UiRequest {
        request: UiRequest {
            purpose: Some(UiPurpose::Approval),
            ..UiRequest::new(dialog, kind)
        },
    }
}

/// `text` on one line, at most [`MAX_SUBJECT_CHARS`] characters and a count of the rest.
fn one_line(text: &str) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");

    match line.char_indices().nth(MAX_SUBJECT_CHARS) {
        Some((end, _)) => {
            let left_out = line[end..].chars().count();
            format!("{}… ({left_out} more characters)", &line[..end])
        }
        None => line,
    }
}
