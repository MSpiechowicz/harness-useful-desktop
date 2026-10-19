//! Wire format of the contract types (what the webview receives).

use harness_business::BusinessError;
use harness_business::model::{
    AgentAction, AgentActionKind, AgentEnvelope, AgentReply, AppInfo, AppPrefs, CommandInvocation, CommandSource,
    CommandSpec, ConfigField, ConfigValue, Detection, GuiTarget, HarnessId, ImageAttachment, InstallHint, IpcError,
    IpcErrorCode, MAX_ACTION_IMAGE_BYTES, MAX_ACTION_IMAGES, MAX_ACTION_TEXT_BYTES, MAX_AGENT_ID_BYTES, ModelOption,
    SecretKind, UiRequest, UiResponse, VersionSupport,
};
use harness_business::security::Redactor;
use pretty_assertions::assert_eq;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

#[test]
fn default_prefs_serialize_with_camel_case_fields() {
    let json = serde_json::to_value(AppPrefs::default()).unwrap();

    assert_eq!(
        json,
        json!({
            "version": 1,
            "theme": "system",
            "motion": "full",
            "language": "en",
            "introEnabled": true,
            "soundEnabled": false,
            "showThinking": true,
            "showMessageUsage": true,
            "chatDefaults": [],
            "defaultHarness": null,
            "activeProject": null,
            "rememberRecentProjects": true,
            "recentProjects": [],
            "updatePolicy": "startupAsk",
            "checkCompanionUpdates": true,
            "autoUpdateExtensions": true,
            "autoRefreshCatalogs": true,
            "openAnyLink": false,
            "aiChatNames": false,
            "onboardingCompleted": false,
            "approvalNoticeSeen": false,
            "binaryOverrides": [],
            "memoryEnabled": true,
            "keepRunningInTray": false
        })
    );
}

#[test]
fn partial_prefs_fill_defaults_and_ignore_unknown_fields() {
    let prefs: AppPrefs = serde_json::from_value(json!({
        "theme": "dark",
        "defaultHarness": "pi",
        "updatePolicy": "autoInstall",
        "futureField": { "x": 1 }
    }))
    .unwrap();

    assert_eq!(prefs.default_harness, Some(HarnessId::Pi));
    assert_eq!(serde_json::to_value(prefs.theme).unwrap(), "dark");
    assert_eq!(serde_json::to_value(prefs.update_policy).unwrap(), "autoInstall");
    assert!(prefs.intro_enabled);
    // Files written before the companion check existed turn it on.
    assert!(prefs.check_companion_updates);
    // Files written before extension auto-update existed turn it on.
    assert!(prefs.auto_update_extensions);
    // The serde default is on. `PrefsStore` carries `autoUpdateExtensions` over for files without it.
    assert!(prefs.auto_refresh_catalogs);
    // Any web link stays closed until the user opts in.
    assert!(!prefs.open_any_link);
    assert!(!prefs.ai_chat_names);
    // Files written before the language choice existed show English.
    assert_eq!(prefs.language, "en");
    assert_eq!(prefs.version, 1);
}

#[test]
fn config_value_is_an_adjacently_tagged_union() {
    let value = ConfigValue::Map(vec![
        ConfigField {
            key: "n".into(),
            value: ConfigValue::Null,
        },
        ConfigField {
            key: "num".into(),
            value: ConfigValue::Number("1.50".into()),
        },
        ConfigField {
            key: "s".into(),
            value: ConfigValue::Secret(SecretKind::Value),
        },
        ConfigField {
            key: "c".into(),
            value: ConfigValue::Secret(SecretKind::Command),
        },
        ConfigField {
            key: "e".into(),
            value: ConfigValue::Secret(SecretKind::EnvRef { name: "TOKEN".into() }),
        },
        ConfigField {
            key: "l".into(),
            value: ConfigValue::List(vec![ConfigValue::Bool(true)]),
        },
    ]);

    assert_eq!(
        serde_json::to_value(&value).unwrap(),
        json!({ "kind": "map", "value": [
            { "key": "n", "value": { "kind": "null" } },
            { "key": "num", "value": { "kind": "number", "value": "1.50" } },
            { "key": "s", "value": { "kind": "secret", "value": { "type": "value" } } },
            { "key": "c", "value": { "kind": "secret", "value": { "type": "command" } } },
            { "key": "e", "value": { "kind": "secret", "value": { "type": "envRef", "name": "TOKEN" } } },
            { "key": "l", "value": { "kind": "list", "value": [{ "kind": "bool", "value": true }] } }
        ]})
    );
}

#[test]
fn harness_ids_are_lowercase() {
    assert_eq!(
        serde_json::to_value(HarnessId::ALL).unwrap(),
        json!(["omp", "pi", "opencode"])
    );
    assert_eq!(HarnessId::Omp.binary_name(), "omp");
    assert_eq!(HarnessId::OpenCode.binary_name(), "opencode");
}

#[test]
fn business_errors_map_onto_ipc_codes() {
    let cases = [
        (
            BusinessError::InvalidInput("x".into()),
            IpcErrorCode::InvalidInput,
            "invalidInput",
        ),
        (BusinessError::NotFound("x".into()), IpcErrorCode::NotFound, "notFound"),
        (
            BusinessError::Forbidden("x".into()),
            IpcErrorCode::Forbidden,
            "forbidden",
        ),
        (
            BusinessError::Unsupported("x".into()),
            IpcErrorCode::Unsupported,
            "unsupported",
        ),
        (
            BusinessError::Io {
                context: "x".into(),
                source: std::io::Error::other("disk"),
            },
            IpcErrorCode::Io,
            "io",
        ),
        (BusinessError::Process("x".into()), IpcErrorCode::Process, "process"),
        (BusinessError::Timeout("x".into()), IpcErrorCode::Timeout, "timeout"),
        (BusinessError::Protocol("x".into()), IpcErrorCode::Protocol, "protocol"),
        (BusinessError::Internal("x".into()), IpcErrorCode::Internal, "internal"),
    ];

    for (error, code, wire) in cases {
        let ipc = IpcError::from(error);
        assert_eq!(ipc.code, code);
        assert_eq!(serde_json::to_value(&ipc).unwrap()["code"], wire);
    }
}

#[test]
fn io_errors_are_promoted_to_specific_codes() {
    let missing = BusinessError::from_io("reading prefs", std::io::Error::from(std::io::ErrorKind::NotFound));
    let denied = BusinessError::from_io(
        "reading prefs",
        std::io::Error::from(std::io::ErrorKind::PermissionDenied),
    );

    assert_eq!(IpcError::from(missing).code, IpcErrorCode::NotFound);
    assert_eq!(IpcError::from(denied).code, IpcErrorCode::Forbidden);
}

#[test]
fn app_info_reports_the_build_target() {
    let info = AppInfo::current("Harness Desktop", "0.1.0");

    assert_eq!(info.os, std::env::consts::OS);
    assert_eq!(info.arch, std::env::consts::ARCH);
    assert_eq!(serde_json::to_value(&info).unwrap()["version"], "0.1.0");
}

#[test]
fn command_invocations_are_tagged_by_kind() {
    let invocations = [
        CommandInvocation::PromptText,
        CommandInvocation::Rpc {
            action: AgentActionKind::SetSessionName,
        },
        CommandInvocation::Gui {
            target: GuiTarget::ModelPicker,
        },
        CommandInvocation::Unsupported {
            reason: "Use the terminal".into(),
        },
    ];

    assert_eq!(
        serde_json::to_value(invocations).unwrap(),
        json!([
            { "kind": "promptText" },
            { "kind": "rpc", "action": "setSessionName" },
            { "kind": "gui", "target": "modelPicker" },
            { "kind": "unsupported", "reason": "Use the terminal" }
        ])
    );
}

#[test]
fn command_specs_round_trip_with_camel_case_fields() {
    let wire = json!({
        "name": "mcp-triage",
        "description": "Triage issues",
        "argumentHint": "[team]",
        "source": "mcpPrompt",
        "invocation": { "kind": "promptText" }
    });

    let spec: CommandSpec = serde_json::from_value(wire.clone()).unwrap();

    assert_eq!(spec.source, CommandSource::McpPrompt);
    assert_eq!(serde_json::to_value(&spec).unwrap(), wire);
}

#[test]
fn missing_harness_detection_carries_an_install_hint() {
    let detection = Detection {
        installed: false,
        binary_path: None,
        is_batch_script: false,
        version: None,
        version_support: VersionSupport::Unknown,
        config_dir: Some("/home/demo/.pi".into()),
        config_dir_exists: false,
        install_hint: Some(InstallHint {
            command: "npm install -g example".into(),
            docs_url: "https://example.invalid/docs".into(),
        }),
        error: None,
    };

    assert_eq!(
        serde_json::to_value(&detection).unwrap(),
        json!({
            "installed": false,
            "binaryPath": null,
            "isBatchScript": false,
            "version": null,
            "versionSupport": "unknown",
            "configDir": "/home/demo/.pi",
            "configDirExists": false,
            "installHint": { "command": "npm install -g example", "docsUrl": "https://example.invalid/docs" },
            "error": null
        })
    );
}

fn top_level_keys(value: ConfigValue) -> Vec<String> {
    let ConfigValue::Map(fields) = value else {
        panic!("expected a map");
    };

    fields.into_iter().map(|field| field.key).collect()
}

#[test]
fn redacted_documents_keep_source_key_order() {
    let from_json: serde_json::Value = serde_json::from_str(r#"{"theme":1,"apiKey":"x","compaction":2}"#).unwrap();
    let from_yaml: serde_json::Value = serde_norway::from_str("zeta: 1\nalpha: 2\nmid: 3\n").unwrap();

    assert_eq!(
        top_level_keys(Redactor.redact(&from_json)),
        ["theme", "apiKey", "compaction"]
    );
    assert_eq!(top_level_keys(Redactor.redact(&from_yaml)), ["zeta", "alpha", "mid"]);
}

/// Asserts that every `wire` value parses as `T` and serializes back unchanged.
fn assert_round_trips<T: Serialize + DeserializeOwned>(wire: &[Value]) {
    for value in wire {
        let parsed: T = serde_json::from_value(value.clone()).unwrap_or_else(|error| panic!("{value}: {error}"));

        assert_eq!(&serde_json::to_value(parsed).unwrap(), value);
    }
}

#[test]
fn agent_actions_are_tagged_with_camel_case_fields() {
    assert_round_trips::<AgentAction>(&[
        json!({ "type": "prompt", "message": "hi", "images": [], "streamingBehavior": "followUp" }),
        json!({ "type": "abort" }),
        json!({ "type": "switchSession", "session": { "harness": "pi", "id": "0199c1f2" } }),
        json!({ "type": "setModel", "provider": "anthropic", "modelId": "claude-sonnet-4-5" }),
        json!({ "type": "setAgent", "agent": "plan" }),
        json!({ "type": "getAvailableAgents" }),
        json!({ "type": "compact", "instructions": null }),
        json!({ "type": "getHistory", "cursor": "page-2", "limit": 50 }),
    ]);
}

#[test]
fn agent_actions_outside_the_whitelist_are_rejected() {
    let raw_rpc = json!({ "type": "bash", "command": "cargo test" });

    assert!(serde_json::from_value::<AgentAction>(raw_rpc).is_err());
}

#[test]
fn agent_action_text_and_page_limits_are_enforced() {
    let prompt = |size: usize| AgentAction::Prompt {
        message: "x".repeat(size),
        images: Vec::new(),
        streaming_behavior: None,
    };
    let history = |limit: u32| AgentAction::GetHistory { cursor: None, limit };
    let agent = |size: usize| AgentAction::SetAgent {
        agent: "a".repeat(size),
    };

    assert!(prompt(MAX_ACTION_TEXT_BYTES).validate().is_ok());
    assert!(history(200).validate().is_ok());
    assert!(agent(MAX_AGENT_ID_BYTES).validate().is_ok());

    let refused = [
        prompt(MAX_ACTION_TEXT_BYTES + 1),
        history(0),
        history(201),
        agent(0),
        agent(MAX_AGENT_ID_BYTES + 1),
    ];
    for action in refused {
        let error = IpcError::from(action.validate().unwrap_err());
        assert_eq!(error.code, IpcErrorCode::InvalidInput);
    }
}

#[test]
fn agent_action_image_limits_are_enforced() {
    let with = |images: Vec<ImageAttachment>| AgentAction::FollowUp {
        message: "look".to_owned(),
        images,
    };
    let image = |data: &str, mime: &str| ImageAttachment {
        data: data.to_owned(),
        mime_type: mime.to_owned(),
    };

    assert!(with(vec![image("aGk=", "image/webp")]).validate().is_ok());

    let bad = [
        with(vec![image("aGk=", "image/svg+xml")]),
        with(vec![image("not base64!", "image/png")]),
        with(vec![image("", "image/png")]),
        with((0..=MAX_ACTION_IMAGES).map(|_| image("aGk=", "image/png")).collect()),
        with(vec![image(&"A".repeat(MAX_ACTION_IMAGE_BYTES + 1), "image/png")]),
    ];

    for action in bad {
        let error = IpcError::from(action.validate().unwrap_err());
        assert_eq!(error.code, IpcErrorCode::InvalidInput);
    }
}

#[test]
fn agent_events_are_enveloped_and_tagged_with_camel_case_fields() {
    let envelope = |event: Value| json!({ "sessionKey": "k", "seq": 7, "event": event });

    assert_round_trips::<AgentEnvelope>(&[
        envelope(json!({ "type": "agentStart" })),
        envelope(json!({ "type": "messageStart", "messageId": "m1", "role": "toolResult" })),
        envelope(json!({ "type": "textDelta", "messageId": "m1", "contentIndex": 2, "delta": "**hi**" })),
        envelope(json!({
            "type": "toolEnd", "toolCallId": "c1", "name": "edit", "isError": false,
            "result": { "content": [] }, "touchedPaths": ["src/lib.rs"]
        })),
        envelope(json!({
            "type": "promptResult", "requestId": null, "status": "error", "error": "429", "retryable": true
        })),
        envelope(json!({ "type": "queueUpdate", "steering": ["a"], "followUp": [], "liveSteered": 0 })),
        envelope(json!({ "type": "agentChanged", "agent": "plan" })),
        envelope(json!({ "type": "uiCancel", "requestId": "ui-1" })),
        envelope(json!({ "type": "frameError", "originalType": null, "error": "bad json" })),
        envelope(json!({ "type": "exited", "code": null, "signal": 9 })),
        envelope(json!({ "type": "unknown", "eventType": "live_start" })),
    ]);
}

#[test]
fn ui_requests_and_responses_are_tagged_by_type() {
    assert_round_trips::<UiRequest>(&[
        json!({ "id": "a", "kind": {
            "type": "select", "title": "Allow bash?", "options": ["Approve", "Deny"],
            "details": ["once", null], "timeoutMs": 30000
        }}),
        json!({ "id": "b", "kind": { "type": "confirm", "title": "T", "message": "M", "timeoutMs": null } }),
        json!({ "id": "c", "kind": { "type": "input", "title": "T", "placeholder": null, "timeoutMs": null } }),
        json!({ "id": "d", "kind": { "type": "editor", "title": "T", "prefill": "x" } }),
    ]);
    assert_round_trips::<UiResponse>(&[
        json!({ "id": "a", "outcome": { "type": "value", "value": "Approve" } }),
        json!({ "id": "b", "outcome": { "type": "confirmed", "confirmed": false } }),
        json!({ "id": "c", "outcome": { "type": "cancelled" } }),
    ]);
}

#[test]
fn agent_replies_are_tagged_by_type() {
    assert_round_trips::<AgentReply>(&[
        json!({ "type": "ack" }),
        json!({ "type": "prompt", "agentInvoked": false, "disposition": null }),
        json!({ "type": "thinkingLevels", "levels": ["off", "high"] }),
        json!({ "type": "agents", "agents": [
            { "id": "build", "name": "build", "description": null },
            { "id": "plan", "name": "plan", "description": "Read-only planning" }
        ] }),
        json!({ "type": "text", "text": null }),
        json!({ "type": "history", "messages": [{
            "id": "e1", "role": "assistant",
            "blocks": [
                { "type": "thinking", "text": "", "redacted": true },
                { "type": "toolCall", "id": "c1", "name": "read", "args": { "path": "a" }, "intent": null },
                { "type": "image", "mimeType": "image/png" }
            ],
            "model": "m", "provider": "p", "stopReason": "toolUse", "errorMessage": null,
            "usage": { "input": 1, "output": 2, "cacheRead": 3, "cacheWrite": 4, "total": 10 },
            "costUsd": 0.5, "timestamp": 1_791_540_000_000_i64, "toolCallId": null, "toolName": null, "isError": null
        }], "nextCursor": null, "total": 1 }),
    ]);
}

#[test]
fn model_options_never_carry_provider_payloads() {
    let harness_model = json!({
        "provider": "corp", "id": "m1", "name": "M1", "reasoning": false, "contextWindow": 128000,
        "maxTokens": 8192, "input": ["text"], "api": "openai-completions",
        "baseUrl": "https://llm.corp.invalid/v1", "headers": { "Authorization": "Bearer CANARY" },
        "compat": { "supportsStore": false }, "cost": { "input": 1.0 }
    });

    let model: ModelOption = serde_json::from_value(harness_model).unwrap();
    let wire = serde_json::to_value(model).unwrap();
    let keys: Vec<&str> = wire.as_object().unwrap().keys().map(String::as_str).collect();

    assert_eq!(
        keys,
        [
            "provider",
            "id",
            "name",
            "reasoning",
            "contextWindow",
            "maxTokens",
            "input"
        ]
    );
}
