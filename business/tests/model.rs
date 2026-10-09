//! Wire format of the C0 contract types (what the webview receives).

use harness_business::BusinessError;
use harness_business::model::{
    AppInfo, AppPrefs, ConfigField, ConfigValue, HarnessId, IpcError, IpcErrorCode, SecretKind,
};
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn default_prefs_serialize_with_camel_case_fields() {
    let json = serde_json::to_value(AppPrefs::default()).unwrap();

    assert_eq!(
        json,
        json!({
            "version": 1,
            "theme": "system",
            "motion": "full",
            "introEnabled": true,
            "defaultHarness": null,
            "activeProject": null,
            "rememberRecentProjects": true,
            "recentProjects": [],
            "updatePolicy": "startupAsk",
            "onboardingCompleted": false,
            "approvalNoticeSeen": false,
            "binaryOverrides": []
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
    assert_eq!(serde_json::to_value(HarnessId::ALL).unwrap(), json!(["omp", "pi"]));
    assert_eq!(HarnessId::Omp.binary_name(), "omp");
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
