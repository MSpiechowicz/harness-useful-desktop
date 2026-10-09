//! Redaction rules: one behavioral case per rule (see `security::redact` docs).

use harness_business::model::{ConfigField, ConfigValue, SecretKind};
use harness_business::security::{RedactionContext, Redactor, redact_text};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const HIDDEN: ConfigValue = ConfigValue::Secret(SecretKind::Value);

fn text(value: &str) -> ConfigValue {
    ConfigValue::Text(value.to_owned())
}

/// Redacts `value` as the value of `key` in a one-entry object and returns the redacted value.
fn under_key(key: &str, value: Value) -> ConfigValue {
    let redacted = Redactor.redact(&json!({ key: value }));
    let ConfigValue::Map(fields) = redacted else {
        panic!("expected a map");
    };

    fields.into_iter().next().unwrap().value
}

#[test]
fn key_name_rules_hide_strings_regardless_of_case_and_separators() {
    let keys = [
        "apiKey",
        "api_key",
        "API-KEY",
        "authToken",
        "client_secret",
        "password",
        "db_passwd",
        "credential",
        "Authorization",
        "bearer",
        "private_key",
        "clientSecret",
        "accessKey",
        "sessionKey",
        "cookie",
        "signature",
        "credentialId",
    ];

    for key in keys {
        assert_eq!(under_key(key, json!("plain-value")), HIDDEN, "key {key}");
    }
}

#[test]
fn key_name_rules_leave_numbers_booleans_and_ordinary_keys_visible() {
    assert_eq!(under_key("maxTokens", json!(4096)), ConfigValue::Number("4096".into()));
    assert_eq!(under_key("cookieConsent", json!(true)), ConfigValue::Bool(true));
    assert_eq!(
        under_key("model", json!("claude-sonnet-4-5")),
        text("claude-sonnet-4-5")
    );
}

#[test]
fn key_name_rules_cover_nested_values_and_lists() {
    let redacted = under_key("credentials", json!({ "user": "me", "list": ["a", "b"] }));

    // Fields keep source order (serde_json `preserve_order`).
    let expected = ConfigValue::Map(vec![
        ConfigField {
            key: "user".into(),
            value: HIDDEN,
        },
        ConfigField {
            key: "list".into(),
            value: ConfigValue::List(vec![HIDDEN, HIDDEN]),
        },
    ]);
    assert_eq!(redacted, expected);
}

#[test]
fn env_and_headers_containers_hide_every_value_but_keep_keys() {
    let redacted = Redactor.redact(&json!({
        "server": { "env": { "LOG_LEVEL": "debug", "PORT": 8080 }, "headers": { "X-Org": "acme" } }
    }));

    let json = serde_json::to_value(&redacted).unwrap();
    let env = &json["value"][0]["value"]["value"][0]["value"]["value"];
    assert_eq!(env[0]["key"], "LOG_LEVEL");
    assert_eq!(
        env[0]["value"],
        json!({ "kind": "secret", "value": { "type": "value" } })
    );
    assert_eq!(env[1]["key"], "PORT");
    assert_eq!(env[1]["value"]["kind"], "secret");

    let headers = &json["value"][0]["value"]["value"][1]["value"]["value"];
    assert_eq!(headers[0]["key"], "X-Org");
    assert_eq!(headers[0]["value"]["kind"], "secret");
}

#[test]
fn auth_containers_hide_everything_except_public_settings() {
    let auth = under_key(
        "auth",
        json!({
            "type": "oauth", "scope": "repo", "redirectUri": "http://127.0.0.1:1/cb",
            "callbackPort": 7777, "callbackPath": "/cb", "prompt": "consent", "clientId": "app",
            "clientSecret": "s", "refresh": "r", "nested": { "a": "b" }
        }),
    );

    let ConfigValue::Map(fields) = auth else {
        panic!("expected a map")
    };
    let hidden: Vec<&str> = fields
        .iter()
        .filter(|field| matches!(field.value, ConfigValue::Secret(_)))
        .map(|field| field.key.as_str())
        .collect();

    assert_eq!(hidden, ["clientSecret", "refresh"]);

    let nested = fields.iter().find(|field| field.key == "nested").unwrap();
    let expected = ConfigValue::Map(vec![ConfigField {
        key: "a".into(),
        value: HIDDEN,
    }]);
    assert_eq!(nested.value, expected);
    assert_eq!(under_key("oauth", json!("scalar-token")), HIDDEN);
}

#[test]
fn command_values_are_hidden_and_flagged_as_commands() {
    let expected = ConfigValue::Secret(SecretKind::Command);

    assert_eq!(under_key("apiKey", json!("!op read op://vault/key")), expected);
    assert_eq!(under_key("anything", json!("!pass show x")), expected);
}

#[test]
fn env_references_keep_only_the_variable_name() {
    let reference = |name: &str| ConfigValue::Secret(SecretKind::EnvRef { name: name.into() });

    assert_eq!(
        under_key("apiKey", json!("$OPENAI_API_KEY")),
        reference("OPENAI_API_KEY")
    );
    assert_eq!(under_key("token", json!("${GH_TOKEN}")), reference("GH_TOKEN"));
    assert_eq!(under_key("note", json!("$HOME/dir")), text("$HOME/dir"));
}

#[test]
fn credential_formats_are_hidden_under_any_key() {
    let samples = [
        "sk-proj-abcdefghijklmnop1234",
        "sk-ant-api03-abcdefghijklmnopqrstu",
        "ghp_abcdefghijklmnopqrstuvwxyz0123",
        "gho_abcdefghijklmnopqrstuvwxyz0123",
        "ghs_abcdefghijklmnopqrstuvwxyz0123",
        "ghu_abcdefghijklmnopqrstuvwxyz0123",
        "github_pat_11ABCDEFG0123456789_abcdef",
        "xoxb-1234567890-abcdefghij",
        "AKIAIOSFODNN7EXAMPLE",
        "AIzaSyA-abcdefghijklmnopqrstuvwxyz01234",
        "glpat-abcdefghijklmnopqrst",
        "hf_abcdefghijklmnopqrstuvwxyz",
        "npm_abcdefghijklmnopqrstuvwxyz0123",
        "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0In0.signaturepart",
        "Bearer abcdefgh12345678",
        "Basic dXNlcjpwYXNzd29yZA==",
        "-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n-----END OPENSSH PRIVATE KEY-----",
        "prefix text with sk-ant-abcdefghijklmnopqrstuvwx inside",
    ];

    for sample in samples {
        assert_eq!(under_key("note", json!(sample)), HIDDEN, "{sample}");
    }
}

#[test]
fn url_userinfo_and_secret_query_values_are_masked_but_the_url_stays_readable() {
    assert_eq!(
        under_key("baseUrl", json!("https://user:pa55@proxy.local:3128/v1")),
        text("https://••••@proxy.local:3128/v1")
    );
    assert_eq!(
        under_key("url", json!("https://ghp-less-token@github.com/acme/repo.git")),
        text("https://••••@github.com/acme/repo.git")
    );
    assert_eq!(
        under_key("url", json!("https://mcp.example.com/sse?api_key=abc&region=eu&sig=zz")),
        text("https://mcp.example.com/sse?api_key=••••&region=eu&sig=••••")
    );
}

#[test]
fn high_entropy_strings_are_hidden() {
    for sample in [
        "aB3dE5fG7hJ9kL1mN3pQ5rS7tU9vW1xY3zA5bC7d",
        "Xk9#mP2$vL8@nQ4!wR6^tY0&uI3*oE5(",
    ] {
        assert_eq!(under_key("value", json!(sample)), HIDDEN, "{sample}");
    }
}

#[test]
fn ordinary_long_values_stay_visible() {
    let visible = [
        "This is a perfectly normal sentence about configuration.",
        "/Users/Frogius/Projekty/harness-useful-desktop2/src",
        "C:\\Users\\Frogius\\AppData\\Roaming\\npm\\node_modules",
        "https://github.com/MSpiechowicz/harness-useful-desktop",
        // UUIDs are identifiers (session ids), not credentials: shown.
        "7218123d-0746-47cd-b886-473b29bc5f49",
        "anthropic/claude-3-5-sonnet-20241022-v2:0",
        "us.anthropic.claude-sonnet-4-5-20250929-v1:0",
        "openrouter/meta-llama/llama-3.1-405b-instruct",
    ];

    for sample in visible {
        assert_eq!(under_key("value", json!(sample)), text(sample), "{sample}");
    }
}

#[test]
fn scalars_pass_through_with_exact_number_text() {
    let redacted = Redactor.redact(&json!([null, true, 42, -7, 1.5, "x"]));

    assert_eq!(
        redacted,
        ConfigValue::List(vec![
            ConfigValue::Null,
            ConfigValue::Bool(true),
            ConfigValue::Number("42".into()),
            ConfigValue::Number("-7".into()),
            ConfigValue::Number("1.5".into()),
            text("x"),
        ])
    );
}

#[test]
fn redact_at_applies_rules_from_the_given_key_path() {
    let mut context = RedactionContext::at(["mcpServers", "github", "env"]);

    assert_eq!(Redactor.redact_at(&json!("value"), &mut context), HIDDEN);
    assert_eq!(context.path(), ["mcpServers", "github", "env"]);
}

#[test]
fn redact_text_masks_tokens_assignments_userinfo_and_random_words() {
    let input = "token sk-ant-abcdefghijklmnopqrstuv used\n\
                 GITHUB_TOKEN=abc123 password: hunter2\n\
                 clone https://me:pw@host/x.git id 7218123d-0746-47cd-b886-473b29bc5f49\n\
                 blob aB3dE5fG7hJ9kL1mN3pQ5rS7tU9vW1xY3zA5bC7d end";

    let expected = "token •••• used\n\
                    GITHUB_TOKEN=•••• password: ••••\n\
                    clone https://••••@host/x.git id 7218123d-0746-47cd-b886-473b29bc5f49\n\
                    blob •••• end";

    assert_eq!(redact_text(input), expected);
}

#[test]
fn redact_text_leaves_plain_text_untouched() {
    let input = "Compiling harness_business v0.1.0\n  Finished in 1.2s";

    assert_eq!(redact_text(input), input);
}
