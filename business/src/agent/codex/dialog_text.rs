//! What the approval dialogs of a Codex chat say (`dialogs.rs`): the command, file change or
//! extra permissions in the title, redacted, and the dialog id of a request. A title holds
//! everything that is granted, never cut and with a command's text as it runs (the approval card
//! lets a long one expand). What widens access most comes first: a folder to write below, then
//! writes, then paths outside the project, then reads and the project's files.

use std::path::{Component, Path, PathBuf};

use crate::claude_stream::permissions::{ALLOW_ONCE, ALWAYS_ALLOW, REJECT};
use crate::codex_rpc::wire::RequestId;
use crate::codex_rpc::wire::items::FileUpdateChange;
use crate::codex_rpc::wire::requests::{CommandApproval, PermissionsApproval};
use crate::model::{AgentEvent, UiPurpose, UiRequest, UiRequestKind};
use crate::security::redact_text;

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
        Some(host) => format!("Allow network access to {host}?"),
        None => titled(label, approval.shown_command()),
    };
    let always = always.then_some("Allows this command again until this chat ends");

    approval_dialog(dialog, &title, approval.reason.as_deref(), always)
}

/// The dialog of extra permissions: what they add in its title, writes first.
pub(super) fn permissions_dialog(dialog: &str, approval: &PermissionsApproval) -> AgentEvent {
    let mut parts = Vec::new();
    let writes = approval.write_paths();
    let reads = approval.read_paths();

    if !writes.is_empty() {
        parts.push(format!("writing {}", writes.join(", ")));
    }

    if approval.network() {
        parts.push("network access".to_owned());
    }

    if !reads.is_empty() {
        parts.push(format!("reading {}", reads.join(", ")));
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

/// `Allow <label>: <subject>?`, or `Allow <label>?`.
pub(super) fn titled(label: &str, subject: Option<&str>) -> String {
    match subject.filter(|subject| !subject.trim().is_empty()) {
        Some(subject) => format!("Allow {label}: {subject}?"),
        None => format!("Allow {label}?"),
    }
}

/// What a file change asks for: the folder `grant_root` it asks to write below first, then every
/// path of `changes` outside `project`, then every project file (relative).
pub(super) fn file_change_subject(
    changes: &[FileUpdateChange],
    project: &Path,
    grant_root: Option<&str>,
) -> Option<String> {
    let (inside, outside): (Vec<&Path>, Vec<&Path>) = changes
        .iter()
        .map(|change| Path::new(&change.path))
        .partition(|path| in_project(path, project));

    let mut parts = Vec::new();

    if let Some(root) = grant_root {
        parts.push(format!("writing anywhere below {root}"));
    }

    if !outside.is_empty() {
        let paths: Vec<String> = outside.iter().map(|path| path.to_string_lossy().into_owned()).collect();
        parts.push(format!("outside the project {}", paths.join(", ")));
    }

    let names: Vec<String> = inside
        .iter()
        .map(|path| {
            path.strip_prefix(project)
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned()
        })
        .collect();

    if !names.is_empty() {
        parts.push(names.join(", "));
    }

    (!parts.is_empty()).then(|| parts.join(", "))
}

/// True for a path of the project: one without `..` that is relative to it or lies inside it once
/// the symbolic links of its deepest existing folder are resolved (a link in the project to a
/// folder outside counts as outside).
fn in_project(path: &Path, project: &Path) -> bool {
    if path.components().any(|part| part == Component::ParentDir) {
        return false;
    }

    let absolute = match path.is_relative() {
        true => project.join(path),
        false => path.to_path_buf(),
    };

    resolved(&absolute).starts_with(project)
}

/// `path` with its deepest existing ancestor canonicalized and the rest as written.
fn resolved(path: &Path) -> PathBuf {
    let mut existing = path;
    let mut rest = Vec::new();

    loop {
        if let Ok(canonical) = dunce::canonicalize(existing) {
            return rest.iter().rev().fold(canonical, |joined, part| joined.join(part));
        }

        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_owned());
                existing = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
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
