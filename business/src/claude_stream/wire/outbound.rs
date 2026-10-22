//! What the app writes to the CLI's stdin, one JSON object per line: user messages
//! ([`user_message`]), the app's control requests ([`control_request`]) and its replies to the
//! CLI's requests ([`permission_allow`], [`permission_deny`], [`question_answers`],
//! [`hook_output`], [`control_error`]). The shapes are those of the recordings.

use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Map, Value, json};

/// The next id for one of the app's control requests: `req_<n>`, unique within this process.
pub fn next_request_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(1);

    format!("req_{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

/// A fresh `uuid` for a user message (it names the message in `command_lifecycle`,
/// `user_message_uuids` and `cancel_async_message`).
pub fn new_message_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// When a message sent while a turn runs is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    /// In the running turn, once its running tool calls finish (steering).
    Next,
    /// As its own turn after the running one (follow-up).
    Later,
}

impl Priority {
    fn wire(self) -> &'static str {
        match self {
            Self::Next => "next",
            Self::Later => "later",
        }
    }
}

/// An image attached to a user message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePart {
    /// MIME type (`image/png`, …).
    pub media_type: String,
    /// The image, base64-encoded.
    pub data: String,
}

/// A user message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserMessage {
    /// From [`new_message_uuid`].
    pub uuid: String,
    pub text: String,
    pub images: Vec<ImagePart>,
    /// `None` for a prompt sent while the chat is idle.
    pub priority: Option<Priority>,
    /// False for a message the model only sees with the next prompt (a `!command`'s output).
    pub should_query: bool,
}

impl UserMessage {
    /// A plain prompt.
    pub fn prompt(uuid: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            uuid: uuid.into(),
            text: text.into(),
            images: Vec::new(),
            priority: None,
            should_query: true,
        }
    }
}

/// The `user` frame of `message`: plain text as a string, with images as content blocks.
pub fn user_message(message: &UserMessage) -> Value {
    let content = if message.images.is_empty() {
        Value::String(message.text.clone())
    } else {
        let mut blocks = vec![json!({ "type": "text", "text": message.text })];
        blocks.extend(message.images.iter().map(|image| {
            json!({
                "type": "image",
                "source": { "type": "base64", "media_type": image.media_type, "data": image.data },
            })
        }));
        Value::Array(blocks)
    };

    let mut frame = json!({
        "type": "user",
        "message": { "role": "user", "content": content },
        "parent_tool_use_id": null,
        "uuid": message.uuid,
    });

    if let Some(priority) = message.priority {
        frame["priority"] = Value::from(priority.wire());
    }

    if !message.should_query {
        frame["shouldQuery"] = Value::Bool(false);
    }

    frame
}

/// A hook the app registers with `initialize`: `event` (`PreToolUse`, …) calls back with one of
/// `callback_ids` for tools matching `matcher`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookRegistration {
    pub event: String,
    pub matcher: Option<String>,
    pub callback_ids: Vec<String>,
}

/// A request the app makes of the CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostRequest {
    /// The handshake, with the app's hooks.
    Initialize {
        hooks: Vec<HookRegistration>,
    },
    /// Stops the running turn (`cancel_queued` also drops the queued messages).
    Interrupt {
        cancel_queued: bool,
    },
    /// Withdraws a queued message.
    CancelAsyncMessage {
        message_uuid: String,
    },
    SetPermissionMode {
        mode: String,
    },
    /// `None` returns to the default model.
    SetModel {
        model: Option<String>,
    },
    /// The effort level (`low` … `max`).
    SetEffort {
        level: String,
    },
    /// `None` lifts the limit.
    SetMaxThinkingTokens {
        tokens: Option<u64>,
    },
    RenameSession {
        title: String,
    },
    StopTask {
        task_id: String,
    },
    GetContextUsage,
    GetSessionCost,
    ListModels,
    McpStatus,
    ListPermissionRules,
    ReloadSkills,
    ReloadPlugins,
}

/// The `control_request` frame of `request` under the app's id `request_id`.
pub fn control_request(request_id: &str, request: &HostRequest) -> Value {
    json!({ "type": "control_request", "request_id": request_id, "request": request_body(request) })
}

fn request_body(request: &HostRequest) -> Value {
    let mut body = Map::new();
    let mut field = |key: &str, value: Value| {
        body.insert(key.to_owned(), value);
    };

    let subtype = match request {
        HostRequest::Initialize { hooks } => {
            if !hooks.is_empty() {
                field("hooks", hooks_object(hooks));
            }
            "initialize"
        }
        HostRequest::Interrupt { cancel_queued } => {
            if *cancel_queued {
                field("cancel_queued", Value::Bool(true));
            }
            "interrupt"
        }
        HostRequest::CancelAsyncMessage { message_uuid } => {
            field("message_uuid", Value::from(message_uuid.as_str()));
            "cancel_async_message"
        }
        HostRequest::SetPermissionMode { mode } => {
            field("mode", Value::from(mode.as_str()));
            "set_permission_mode"
        }
        HostRequest::SetModel { model } => {
            if let Some(model) = model {
                field("model", Value::from(model.as_str()));
            }
            "set_model"
        }
        HostRequest::SetEffort { level } => {
            field("settings", json!({ "effortLevel": level }));
            "apply_flag_settings"
        }
        HostRequest::SetMaxThinkingTokens { tokens } => {
            field("max_thinking_tokens", tokens.map_or(Value::Null, Value::from));
            "set_max_thinking_tokens"
        }
        HostRequest::RenameSession { title } => {
            field("title", Value::from(title.as_str()));
            field("source", Value::from("host"));
            "rename_session"
        }
        HostRequest::StopTask { task_id } => {
            field("task_id", Value::from(task_id.as_str()));
            "stop_task"
        }
        HostRequest::GetContextUsage => "get_context_usage",
        HostRequest::GetSessionCost => "get_session_cost",
        HostRequest::ListModels => "list_models",
        HostRequest::McpStatus => "mcp_status",
        HostRequest::ListPermissionRules => "list_permission_rules",
        HostRequest::ReloadSkills => "reload_skills",
        HostRequest::ReloadPlugins => "reload_plugins",
    };

    let mut ordered = Map::new();
    ordered.insert("subtype".to_owned(), Value::from(subtype));
    ordered.extend(body);

    Value::Object(ordered)
}

/// `hooks` of `initialize`: event → `[{matcher, hookCallbackIds}]`.
fn hooks_object(hooks: &[HookRegistration]) -> Value {
    let mut events = Map::new();

    for hook in hooks {
        let mut matcher = Map::new();

        if let Some(pattern) = &hook.matcher {
            matcher.insert("matcher".to_owned(), Value::from(pattern.as_str()));
        }
        matcher.insert("hookCallbackIds".to_owned(), json!(hook.callback_ids));

        let list = events.entry(hook.event.clone()).or_insert_with(|| json!([]));
        if let Value::Array(list) = list {
            list.push(Value::Object(matcher));
        }
    }

    Value::Object(events)
}

/// A successful reply to the CLI's request `request_id`.
pub fn control_success(request_id: &str, response: Value) -> Value {
    json!({
        "type": "control_response",
        "response": { "subtype": "success", "request_id": request_id, "response": response },
    })
}

/// A failed reply to the CLI's request `request_id` (a request the app does not handle).
pub fn control_error(request_id: &str, error: &str) -> Value {
    json!({
        "type": "control_response",
        "response": { "subtype": "error", "request_id": request_id, "error": error },
    })
}

/// Lets a tool run with `input` (the asked input, or the user's edit). `permissions` are the
/// rules to add ("always allow": the request's `permission_suggestions`), else empty.
pub fn permission_allow(request_id: &str, input: &Value, permissions: &[Value]) -> Value {
    control_success(
        request_id,
        json!({ "behavior": "allow", "updatedInput": input, "updatedPermissions": permissions }),
    )
}

/// Denies a tool call; the model sees `message` as the call's error.
pub fn permission_deny(request_id: &str, message: &str) -> Value {
    control_success(request_id, json!({ "behavior": "deny", "message": message }))
}

/// Answers an `AskUserQuestion` permission request whose input was `input`: each question with
/// the labels the user picked (several joined with `", "`).
pub fn question_answers(request_id: &str, input: &Value, answers: &[(String, Vec<String>)]) -> Value {
    let picked: Map<String, Value> = answers
        .iter()
        .map(|(question, labels)| (question.clone(), Value::from(labels.join(", "))))
        .collect();

    let mut updated = match input {
        Value::Object(fields) => fields.clone(),
        _ => Map::new(),
    };
    updated.insert("answers".to_owned(), Value::Object(picked));

    control_success(
        request_id,
        json!({ "behavior": "allow", "updatedInput": Value::Object(updated) }),
    )
}

/// The reply to a `hook_callback`: the hook's output (`{}` for no opinion).
pub fn hook_output(request_id: &str, output: Value) -> Value {
    control_success(request_id, output)
}

/// The output of a `PreToolUse` hook that makes the CLI ask the app before every tool call.
pub fn ask_hook_output() -> Value {
    json!({ "hookSpecificOutput": { "hookEventName": "PreToolUse", "permissionDecision": "ask" } })
}
