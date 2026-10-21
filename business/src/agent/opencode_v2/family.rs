//! The chat's subagent sessions and the asks OpenCode waits for, read when the chat follows the
//! event stream again (`catch_up.rs`) and, for the subagent sessions, when it resumes a session
//! (`stream.rs`): what the chat missed meanwhile shows as it would have live.
//!
//! - **Subagent sessions:** `GET /api/session?parentID=` of the chat's session and, breadth first,
//!   of each session found, at most [`MAX_CHILD_LISTS`] lists of the [`CHILD_PAGE`] most recently
//!   changed. The mapper counts them as the chat's, so their asks reach the chat.
//! - **Asks:** `GET /api/permission/request` and `GET /api/form` of the chat's directory and of
//!   each subagent session's own. An ask of the chat or its subagent sessions the chat does not
//!   show yet is taken as its `permission.asked` or `form.created` (an "Always allow" answer
//!   covers a permission ask as it would live, `approvals.rs`). A dialog whose ask is no longer
//!   pending closes, as a missed `permission.replied`, `form.replied` or `form.cancelled` would
//!   close it; only when every list was read.
//! - A read that fails leaves the chat as it is.

use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde_json::Value;

use super::startup::get_data;
use super::{Chat, Live, approvals, stream};
use crate::opencode_server_v2::events::{Event, EventKind, FormCreated};
use crate::opencode_server_v2::wire::{Form, PermissionRequest, Session};
use crate::opencode_server_v2::{RouteV2, SessionId};

/// Sessions read per list of subagent sessions.
const CHILD_PAGE: u32 = 50;

/// Lists of subagent sessions read at most (one per session whose own subagents are looked for).
const MAX_CHILD_LISTS: usize = 16;

/// A session started from one of the chat's sessions.
pub(super) struct Child {
    id: String,
    parent: String,
    directory: String,
}

/// The subagent sessions of `session` (see the module docs), parents before their children.
pub(super) async fn children(chat: &Chat, session: &SessionId) -> Vec<Child> {
    let mut found: Vec<Child> = Vec::new();
    let mut known = HashSet::from([session.to_string()]);
    let mut queue = VecDeque::from([session.clone()]);
    let mut lists = 0;

    while let Some(parent) = queue.pop_front() {
        if lists == MAX_CHILD_LISTS {
            break;
        }

        lists += 1;

        let route = RouteV2::SessionChildren {
            parent: &parent,
            limit: CHILD_PAGE,
        };
        let listed = match get_data::<Vec<Value>>(&chat.client, route).await {
            Ok(listed) => listed,
            Err(error) => {
                tracing::debug!(%error, "the OpenCode subagent sessions were not read");
                continue;
            }
        };

        for session in readable::<Session>(listed) {
            let Ok(id) = SessionId::parse(&session.id) else {
                continue;
            };

            if session.parent_id.as_deref() != Some(parent.as_str()) || !known.insert(session.id.clone()) {
                continue;
            }

            found.push(Child {
                id: session.id,
                parent: parent.to_string(),
                directory: session.location.directory,
            });
            queue.push_back(id);
        }
    }

    found
}

/// The asks OpenCode waits for in the chat's directories.
pub(super) struct Pending {
    permissions: Vec<PermissionRequest>,
    forms: Vec<Form>,
    /// Every list was read: an ask missing from them is no longer pending.
    complete: bool,
}

/// Reads the pending asks of the chat's directory and of `children`'s own.
pub(super) async fn pending(chat: &Chat, children: &[Child], known: &[String]) -> Pending {
    let mut directories = vec![chat.directory.clone()];
    let named = children.iter().map(|child| &child.directory).chain(known);

    for directory in named {
        if !directory.is_empty() && !directories.contains(directory) {
            directories.push(directory.clone());
        }
    }

    let mut pending = Pending {
        permissions: Vec::new(),
        forms: Vec::new(),
        complete: true,
    };

    for directory in &directories {
        let directory = directory.as_str();

        match get_data::<Vec<Value>>(&chat.client, RouteV2::PermissionRequests { directory }).await {
            Ok(listed) => pending.permissions.extend(readable(listed)),
            Err(error) => {
                tracing::debug!(%error, "the pending OpenCode permission asks were not read");
                pending.complete = false;
            }
        }

        match get_data::<Vec<Value>>(&chat.client, RouteV2::FormList { directory }).await {
            Ok(listed) => pending.forms.extend(readable(listed)),
            Err(error) => {
                tracing::debug!(%error, "the pending OpenCode forms were not read");
                pending.complete = false;
            }
        }
    }

    pending.permissions.sort_by(|a, b| a.id.cmp(&b.id));
    pending.forms.sort_by(|a, b| a.id.cmp(&b.id));
    pending
}

/// Counts `children` as the chat's subagent sessions.
pub(super) fn adopt(live: &mut Live, children: &[Child]) {
    for child in children {
        let directory = Some(child.directory.clone()).filter(|directory| !directory.is_empty());
        live.mapper.adopt(&child.id, &child.parent, directory);
    }
}

/// Shows the pending asks the chat does not show yet and closes the dialogs of the asks that are
/// no longer pending (see the module docs).
pub(super) fn take(chat: &Arc<Chat>, live: &mut Live, pending: Pending) {
    let mut ids: Vec<String> = Vec::new();
    let mut unseen: Vec<EventKind> = Vec::new();

    for permission in pending.permissions {
        ids.push(permission.id.clone());
        unseen.push(EventKind::PermissionAsked(permission));
    }

    for form in pending.forms {
        ids.push(form.id.clone());
        unseen.push(EventKind::FormCreated(FormCreated { form }));
    }

    for kind in unseen {
        let (id, session) = match &kind {
            EventKind::PermissionAsked(permission) => (&permission.id, &permission.session_id),
            EventKind::FormCreated(created) => (&created.form.id, &created.form.session_id),
            _ => continue,
        };

        if live.mapper.knows_ask(id) || !live.mapper.is_chat_session(session) {
            continue;
        }

        let event = Event {
            id: String::new(),
            created: None,
            location: None,
            session_id: Some(session.clone()),
            kind,
        };

        if approvals::answered_by_always(chat, live, &event) {
            continue;
        }

        let mapped = live.mapper.map(&event);
        stream::apply(chat, live, mapped);
    }

    if pending.complete {
        let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        let settled = live.mapper.settle_asks(&ids);
        settled.into_iter().for_each(|event| chat.shared.emit(event));
    }
}

/// The items of `listed` that read as `T`; the others are skipped.
fn readable<T: DeserializeOwned>(listed: Vec<Value>) -> Vec<T> {
    listed
        .into_iter()
        .filter_map(|item| serde_json::from_value(item).ok())
        .collect()
}
