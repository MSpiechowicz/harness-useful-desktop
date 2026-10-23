//! What the approval dialogs of a Codex chat say (`dialogs.rs`): the command, file change or
//! extra permissions in the title, and the dialog id of a request. A title holds everything that
//! is granted, never cut and with a command's text as it runs, unredacted (a secret in it is the
//! user's own, and masking could hide what runs), the rest redacted. Hidden characters are spelled
//! out (`⟨U+202E⟩`), so no bidirectional control reorders what the user reads. What widens access
//! most comes first: a folder to write below, then writes, then paths outside the project (a move
//! names where it goes), then reads and the project's files.

use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::claude_stream::permissions::{ALLOW_ONCE, ALWAYS_ALLOW, REJECT};
use crate::codex_rpc::wire::RequestId;
use crate::codex_rpc::wire::items::FileUpdateChange;
use crate::codex_rpc::wire::requests::{CommandApproval, PermissionsApproval};
use crate::model::{AgentEvent, UiPurpose, UiRequest, UiRequestKind};
use crate::security::redact_text;
use crate::signed_docs::unicode::spelled_out;

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
        Some(host) => redact_text(&format!("Allow network access to {host}?")),
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
    let title = redact_text(&titled("extra permissions", subject.as_deref()));

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

/// What a file change asks for (see [`file_change_subject`]).
pub(super) struct ChangeSubject {
    pub(super) text: Option<String>,
    /// A file it changes, or the one a move goes to, lies outside the project.
    pub(super) outside: bool,
}

/// What a file change asks for: the folder `grant_root` it asks to write below first, then every
/// change of `changes` with a path outside `project`, then the project's files (relative). A move
/// names both paths, listed by the one outside: `/x (moved from a.rs)`, `/y (moved to b.rs)`,
/// `a.rs (moved to b.rs)`.
pub(super) fn file_change_subject(
    changes: &[FileUpdateChange],
    project: &Path,
    grant_root: Option<&str>,
) -> ChangeSubject {
    let mut outside = Vec::new();
    let mut inside = Vec::new();

    for change in changes {
        let from = Path::new(&change.path);
        let to = change
            .kind
            .move_path
            .as_deref()
            .filter(|to| !to.is_empty())
            .map(Path::new);
        let from_inside = in_project(from, project);
        let to_inside = to.is_none_or(|to| in_project(to, project));

        let named = |path: &Path, inside: bool| match inside {
            true => path
                .strip_prefix(project)
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned(),
            false => path.to_string_lossy().into_owned(),
        };
        let from_named = named(from, from_inside);

        let text = match to {
            Some(to) if !to_inside => format!("{} (moved from {from_named})", named(to, false)),
            Some(to) => format!("{from_named} (moved to {})", named(to, true)),
            None => from_named,
        };

        if from_inside && to_inside {
            inside.push(text);
        } else {
            outside.push(text);
        }
    }

    let mut parts = Vec::new();

    if let Some(root) = grant_root {
        parts.push(format!("writing anywhere below {root}"));
    }

    if !outside.is_empty() {
        parts.push(format!("outside the project {}", outside.join(", ")));
    }

    if !inside.is_empty() {
        parts.push(inside.join(", "));
    }

    ChangeSubject {
        text: (!parts.is_empty()).then(|| parts.join(", ")),
        outside: !outside.is_empty(),
    }
}

/// True for a path of the project (canonical): one without `..` that is relative to it or lies
/// inside it once the symbolic links of its deepest existing folder are resolved (a link in the
/// project to a folder outside counts as outside), a dangling link followed to where it points.
/// A path that cannot be told counts as outside.
pub(super) fn in_project(path: &Path, project: &Path) -> bool {
    if path.components().any(|part| part == Component::ParentDir) {
        return false;
    }

    let absolute = match path.is_relative() {
        true => project.join(path),
        false => path.to_path_buf(),
    };

    resolved(&absolute, MAX_LINKS).is_some_and(|resolved| resolved.starts_with(project))
}

/// The most dangling links [`resolved`] follows in a row (as the system's limit of 40).
const MAX_LINKS: usize = 40;

/// `path` with its deepest existing ancestor canonicalized and the rest as written. A dangling
/// link on the way (whose target does not exist) is resolved to its target, relative to its
/// folder, at most `links` deep. `None` when that cannot be told: a `..` below what exists, too
/// many links.
fn resolved(path: &Path, links: usize) -> Option<PathBuf> {
    let mut existing = path;
    let mut rest = Vec::new();

    loop {
        if let Ok(canonical) = dunce::canonicalize(existing) {
            return Some(rest.iter().rev().fold(canonical, |joined, part| joined.join(part)));
        }

        if let Ok(target) = fs::read_link(existing) {
            let folder = existing.parent()?;
            let target = resolved(&folder.join(target), links.checked_sub(1)?)?;

            return Some(rest.iter().rev().fold(target, |joined, part| joined.join(part)));
        }

        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_owned());
                existing = parent;
            }
            _ => {
                let lexical = !path.components().any(|part| part == Component::ParentDir);
                return lexical.then(|| path.to_path_buf());
            }
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

/// An approval `select` as `dialog`, titled `title` (redacted where it should be, see the module
/// docs), with `reason` as the detail of `Allow once` and `always` as the one of `Always allow`
/// (left out without it). Hidden characters are spelled out in all of it.
pub(super) fn approval_dialog(dialog: &str, title: &str, reason: Option<&str>, always: Option<&str>) -> AgentEvent {
    let mut options = vec![ALLOW_ONCE.to_owned()];
    let mut details = vec![reason.map(|reason| spelled_out(&redact_text(reason)))];

    if let Some(always) = always {
        options.push(ALWAYS_ALLOW.to_owned());
        details.push(Some(always.to_owned()));
    }

    options.push(REJECT.to_owned());
    details.push(None);

    let kind = UiRequestKind::Select {
        title: spelled_out(title),
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
