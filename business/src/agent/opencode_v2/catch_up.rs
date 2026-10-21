//! Catching up after the event stream was opened again (`stream.rs`): what happened while the
//! chat did not follow is read from the session and replayed through the chat's
//! [`Mapper`](crate::opencode_server_v2::Mapper) as the events OpenCode would have sent, so it shows
//! as it would have live, and nothing shows twice.
//!
//! - **Session:** `GET /api/session/{id}`: a new title, agent or model is a fresh `state`.
//! - **Messages:** the newest [`CATCH_UP_PAGE`] messages, oldest first. A prompt the chat does not
//!   have shows once the run took it; a reply the chat does not have, or one still open, is
//!   replayed from its parts (text, reasoning, tool calls and their results; the mapper sends only
//!   what it did not send yet) and ends when the message is complete. The run that holds them
//!   starts first, if the chat did not see it start.
//! - **Ends:** an `idle` marker newer than the newest event the chat read ([`Live::mark`]) ends the
//!   chat's turn as it ended (succeeded, failed or interrupted), with the prompt results, the next
//!   held follow-up or `settled` that follow.
//! - A read that fails leaves the chat as it is: the live events that follow still show.

use std::sync::Arc;

use serde_json::{Value, json};

use super::startup::get_data;
use super::{Chat, Live, stream};
use crate::opencode_server_v2::events;
use crate::opencode_server_v2::mapper::SessionChange;
use crate::opencode_server_v2::wire::{Envelope, Session};
use crate::opencode_server_v2::{MessageOrder, RouteV2, SessionId};

/// Messages read to catch up.
const CATCH_UP_PAGE: u32 = 50;

/// Reads the session and its newest messages and replays what the chat missed.
pub(super) async fn run(chat: &Arc<Chat>) {
    let session = chat.lock().session.clone();
    let directory = chat.directory.as_str();

    let info = get_data::<Session>(
        &chat.client,
        RouteV2::SessionGet {
            session: &session,
            directory,
        },
    )
    .await;
    let page = newest_messages(chat, &session).await;

    let mut live = chat.lock();

    if live.session != session {
        return;
    }

    match info {
        Ok(info) => session_changes(chat, &mut live, info),
        Err(error) => tracing::debug!(%error, "the OpenCode session was not read to catch up"),
    }

    match page {
        Ok(mut records) => {
            records.reverse();
            replay(chat, &mut live, &records);
        }
        Err(error) => tracing::debug!(%error, "the OpenCode messages were not read to catch up"),
    }
}

/// The session's newest messages, newest first.
async fn newest_messages(chat: &Chat, session: &SessionId) -> crate::error::Result<Vec<Value>> {
    let route = RouteV2::Messages {
        session,
        directory: &chat.directory,
        limit: CATCH_UP_PAGE,
        cursor: None,
        order: Some(MessageOrder::Descending),
    };
    let response = chat.client.send(route, None).await?.error_for_status()?;

    Ok(response.json::<Envelope<Vec<Value>>>()?.data)
}

/// The title, agent and model the session has now.
fn session_changes(chat: &Chat, live: &mut Live, info: Session) {
    let changes = [
        info.title.map(SessionChange::Title),
        info.agent.map(SessionChange::Agent),
        info.model.map(SessionChange::Model),
    ];

    for change in changes.into_iter().flatten() {
        chat.session_changed(live, change);
    }
}

/// Replays `records` (oldest first) through the mapper (see the module docs).
fn replay(chat: &Arc<Chat>, live: &mut Live, records: &[Value]) {
    let session = live.session.to_string();
    let mark = live.mark;
    let mut running = false;

    for record in records {
        let id = text(record, "id");
        let created = record.pointer("/time/created").and_then(Value::as_u64);
        let known = live.mapper.has_message(id);

        let synthetic = match text(record, "type") {
            "user" if !known => user_events(&session, record, &mut running),
            // A reply the chat has gets the parts that arrived meanwhile (the mapper ignores
            // what it already sent, and every event of a reply that ended).
            "assistant" if known => reply_events(&session, record),
            "assistant" => {
                let mut events = run_started(&session, &mut running);
                events.extend(reply_events(&session, record));
                events
            }
            "idle" if created.is_some_and(|created| created > mark) => {
                live.mark = live.mark.max(created.unwrap_or_default());
                running = false;
                vec![run_ended(&session, record)]
            }
            _ => Vec::new(),
        };

        for (kind, data) in synthetic {
            let raw = json!({ "id": "", "created": created, "type": kind, "data": data });

            let Ok(event) = events::parse(&raw.to_string()) else {
                continue;
            };

            let mapped = live.mapper.map(&event);
            stream::apply(chat, live, mapped);
        }
    }
}

type Synthetic = (&'static str, Value);

/// The run's start, unless a replayed one runs (the mapper ignores a start while its turn runs).
fn run_started(session: &str, running: &mut bool) -> Vec<Synthetic> {
    if std::mem::replace(running, true) {
        return Vec::new();
    }

    vec![("session.execution.started", json!({ "sessionID": session }))]
}

/// A prompt the run took: admitted, (the run's start,) delivered.
fn user_events(session: &str, record: &Value, running: &mut bool) -> Vec<Synthetic> {
    let id = text(record, "id");
    let payload = json!({
        "text": text(record, "text"),
        "files": record.get("files").cloned().unwrap_or(Value::Array(Vec::new())),
    });

    let mut events = vec![(
        "session.inbox.enqueued",
        json!({ "sessionID": session, "inboxID": id, "item": { "type": "user", "payload": payload } }),
    )];
    events.extend(run_started(session, running));
    events.push((
        "session.inbox.delivered",
        json!({ "sessionID": session, "inboxID": id }),
    ));

    events
}

/// A reply as its step's events: the start, each part's full text, each tool call and its result,
/// and the end once the message is complete.
fn reply_events(session: &str, record: &Value) -> Vec<Synthetic> {
    let message = text(record, "id");
    let base = |extra: Value| {
        let mut data = json!({ "sessionID": session, "assistantMessageID": message });
        merge(&mut data, extra);
        data
    };

    let mut events = vec![(
        "session.step.started",
        base(json!({ "agent": text(record, "agent"), "model": record.get("model") })),
    )];
    let (mut texts, mut thoughts) = (0_u64, 0_u64);

    for item in record.get("content").and_then(Value::as_array).into_iter().flatten() {
        match text(item, "type") {
            "text" => {
                events.push(("session.text.ended", base(part(texts, item))));
                texts += 1;
            }
            "reasoning" => {
                events.push(("session.reasoning.ended", base(part(thoughts, item))));
                thoughts += 1;
            }
            "tool" => events.extend(tool_events(item, &base)),
            _ => {}
        }
    }

    if record
        .pointer("/time/completed")
        .is_some_and(|completed| !completed.is_null())
    {
        events.push(step_end(record, &base));
    }

    events
}

/// A tool call's events as far as its state goes.
fn tool_events(item: &Value, base: &dyn Fn(Value) -> Value) -> Vec<Synthetic> {
    let id = text(item, "id");
    let state = item.get("state").unwrap_or(&Value::Null);
    let input = state.get("input").cloned().unwrap_or(Value::Null);
    let content = state.get("content").cloned().unwrap_or(Value::Array(Vec::new()));

    let mut events = vec![(
        "session.tool.input.started",
        base(json!({ "id": id, "name": text(item, "name") })),
    )];
    let called = ("session.tool.called", base(json!({ "id": id, "input": input })));

    match text(state, "status") {
        "running" => events.push(called),
        "completed" => {
            events.push(called);
            events.push(("session.tool.success", base(json!({ "id": id, "content": content }))));
        }
        "error" => {
            let error = state.get("error").cloned().unwrap_or(Value::Null);
            events.push(called);
            events.push((
                "session.tool.failed",
                base(json!({ "id": id, "error": error, "content": content })),
            ));
        }
        _ => {}
    }

    events
}

/// The end of a complete reply: failed with its error, else ended with its finish and usage.
fn step_end(record: &Value, base: &dyn Fn(Value) -> Value) -> Synthetic {
    match record.get("error").filter(|error| !error.is_null()) {
        Some(error) => ("session.step.failed", base(json!({ "error": error }))),
        None => (
            "session.step.ended",
            base(json!({
                "finish": text(record, "finish"),
                "cost": record.get("cost").cloned().unwrap_or(json!(0)),
                "tokens": record.get("tokens").cloned().unwrap_or(json!({})),
            })),
        ),
    }
}

/// The end of a run as its `idle` marker tells it.
fn run_ended(session: &str, record: &Value) -> Synthetic {
    match text(record, "outcome") {
        "failed" => (
            "session.execution.failed",
            json!({ "sessionID": session, "error": record.get("error").cloned().unwrap_or(json!({})) }),
        ),
        "interrupted" => (
            "session.execution.interrupted",
            json!({ "sessionID": session, "reason": "user" }),
        ),
        _ => ("session.execution.succeeded", json!({ "sessionID": session })),
    }
}

/// `{ordinal, text}` of a text or reasoning part.
fn part(ordinal: u64, item: &Value) -> Value {
    json!({ "ordinal": ordinal, "text": text(item, "text") })
}

fn merge(data: &mut Value, extra: Value) {
    if let (Value::Object(data), Value::Object(extra)) = (data, extra) {
        data.extend(extra.into_iter().filter(|(_, value)| !value.is_null()));
    }
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}
