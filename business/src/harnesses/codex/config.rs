//! The Codex configuration view (Codex CLI 0.154).
//!
//! Sections, in display order: the user's `config.toml` and the project's `.codex/config.toml`
//! (see [`super::settings`]). Missing files are left out. Every value is redacted by the shared
//! rules (`env` and `http_headers` hidden, keys such as `bearer_token` hidden, credential formats
//! masked), plus Codex's own: `shell_environment_policy.set` (variables Codex sets for its
//! commands) and `query_params` are hidden containers, a `*_env_var` or `env_key` is shown as the
//! variable it names, and the secret-looking words of MCP `args` are masked.
//!
//! Harness Skills (0.4.0 and newer, which write model roles for Codex) add a user layer section
//! with `harness-useful-skills/models.yml` of the Codex home: role to `<model>[:<effort>]`. It is
//! listed only when that install is there and the file exists, never for an older install, which
//! has no roles.
//!
//! Notices: API key variables set in the environment (by name only), the configured mode (model,
//! reasoning effort, approval policy, sandbox), an untrusted project layer and what is never read.

use std::path::Path;

use serde_json::{Map, Value};

use super::modes::ConfiguredMode;
use super::paths::codex_home;
use super::settings::{CONFIG_FILE, is_trusted, project_config, project_config_path, user_config, user_config_path};
use crate::ecosystem::catalog::SKILLS_UNIFIED_VERSION;
use crate::ecosystem::releases::is_newer_release;
use crate::ecosystem::skills_record::skills_install;
use crate::env::Env;
use crate::harness::SkillsHost;
use crate::harnesses::skills_config::{models_path, models_section};
use crate::model::{
    ConfigField, ConfigFormat, ConfigLayer, ConfigSection, ConfigValue, ConfigView, HarnessId, Notice, NoticeLevel,
};
use crate::pi_family::mcp_config::mask_server_args;
use crate::pi_family::{Document, SectionText, section_with, source};
use crate::security::{RedactionContext, Redactor};

const USER_TEXT: SectionText<'static> = SectionText {
    id: "user.config",
    title: "Settings",
    description: "Global Codex settings: model, approvals, sandbox, MCP servers, plugins, marketplaces and features.",
};

const PROJECT_TEXT: SectionText<'static> = SectionText {
    id: "project.config",
    title: "Project settings",
    description: "Settings for this project. Codex applies them only in a folder you trust.",
};

const SKILLS_MODELS_TEXT: SectionText<'static> = SectionText {
    id: "user.harnessSkillsModels",
    title: "Harness Skills model roles",
    description: "The model and reasoning effort of each Harness Skills role. Run the Harness Skills installer again \
                  to apply a change to the role agents Codex uses.",
};

/// Presence-only variables (see [`crate::env::PRESENCE_VARS`]) and what each does to Codex.
const SECRET_VARS: &[(&str, &str)] = &[
    ("OPENAI_API_KEY", "can sign Codex in with an API key"),
    ("CODEX_API_KEY", "signs Codex in with an API key"),
];

/// Settings whose whole value is hidden, keys aside.
const HIDDEN_SETTINGS: &[&str] = &["query_params"];

const NEVER_READ_NOTICE: &str =
    "The login (auth.json), prompt history, session files, logs, databases and caches are never read.";

const UNTRUSTED_NOTICE: &str =
    "Codex does not trust this folder yet, so it ignores the project settings until you trust it in Codex.";

/// See the module docs.
pub(super) fn config_view(env: &Env, project: Option<&Path>) -> ConfigView {
    let user_path = user_config_path(env);
    let user = user_config(env);
    let mut sections = Vec::new();

    if user_path.is_file() {
        sections.push(section(USER_TEXT, &user_path, ConfigLayer::User, &user));
    }

    sections.extend(skills_models(env));

    let mut notices: Vec<Notice> = SECRET_VARS
        .iter()
        .filter(|(name, _)| env.is_set(name))
        .map(|(name, effect)| Notice {
            level: NoticeLevel::Warning,
            message: format!("{name} is set in the environment and {effect}. Its value is not shown."),
        })
        .collect();

    notices.extend(mode_notice(&ConfiguredMode::load(env, project)));

    if let Some(project) = project {
        let path = project_config_path(project);

        if path.is_file() && path != user_path {
            sections.push(section(
                PROJECT_TEXT,
                &path,
                ConfigLayer::Project,
                &project_config(project),
            ));

            if !is_trusted(&user, project) {
                notices.push(info(UNTRUSTED_NOTICE.to_owned()));
            }
        }
    }

    notices.push(info(NEVER_READ_NOTICE.to_owned()));

    ConfigView {
        harness: HarnessId::Codex,
        sections,
        notices,
    }
}

/// The Harness Skills model roles of the Codex home: when the installed Harness Skills are
/// [`SKILLS_UNIFIED_VERSION`] or newer under the new name (an older install, or one still under
/// the old name, has no Codex roles) and the file exists.
fn skills_models(env: &Env) -> Option<ConfigSection> {
    let record = skills_install(env, SkillsHost::Codex);
    let version = record.installed.version()?;

    if record.legacy || is_newer_release(SKILLS_UNIFIED_VERSION, version) {
        return None;
    }

    models_section(
        SKILLS_MODELS_TEXT,
        &models_path(&codex_home(env)),
        ConfigLayer::User,
        None,
    )
}

fn section(text: SectionText<'_>, path: &Path, layer: ConfigLayer, document: &Document) -> ConfigSection {
    section_with(
        text,
        source(path, layer, None, ConfigFormat::Toml),
        document,
        config_fields,
    )
}

/// The configured mode in one line; only the values that are set (none: no notice).
fn mode_notice(mode: &ConfiguredMode) -> Option<Notice> {
    let parts: Vec<String> = [
        ("model", &mode.model),
        ("reasoning effort", &mode.effort),
        ("approval policy", &mode.approval_policy),
        ("sandbox", &mode.sandbox_mode),
    ]
    .into_iter()
    .filter_map(|(label, value)| value.as_ref().map(|value| format!("{label} {value}")))
    .collect();

    if parts.is_empty() {
        return None;
    }

    Some(info(format!("Codex is configured with {}.", parts.join(", "))))
}

fn info(message: String) -> Notice {
    Notice {
        level: NoticeLevel::Info,
        message,
    }
}

/// The redacted top-level fields of a `config.toml` (see the module docs).
pub(super) fn config_fields(value: &Value) -> Result<Vec<ConfigField>, String> {
    let entries = match value {
        Value::Null => return Ok(Vec::new()),
        Value::Object(entries) => entries,
        _ => return Err(format!("Expected a table at the top level of {CONFIG_FILE}")),
    };

    let mut fields: Vec<ConfigField> = entries
        .iter()
        .map(|(key, value)| ConfigField {
            key: key.clone(),
            value: redact_setting(key, &marked_env_names(value)),
        })
        .collect();

    mask_server_args(&mut fields, "mcp_servers");
    Ok(fields)
}

fn redact_setting(key: &str, value: &Value) -> ConfigValue {
    if key == "shell_environment_policy" {
        return redact_shell_policy(value);
    }

    redact_at(value, &[key])
}

/// The redaction of `value` at `path`, with [`HIDDEN_SETTINGS`] hidden wherever they are.
fn redact_at(value: &Value, path: &[&str]) -> ConfigValue {
    let Value::Object(entries) = value else {
        return Redactor.redact_at(value, &mut RedactionContext::at(path.iter().copied()));
    };

    let fields = entries
        .iter()
        .map(|(key, item)| {
            let mut child = path.to_vec();
            child.push(key);

            let value = if HIDDEN_SETTINGS.contains(&key.as_str()) {
                Redactor.redact_hidden(item)
            } else {
                redact_at(item, &child)
            };

            ConfigField {
                key: key.clone(),
                value,
            }
        })
        .collect();

    ConfigValue::Map(fields)
}

/// `shell_environment_policy` with its `set` table (literal variable values) hidden.
fn redact_shell_policy(value: &Value) -> ConfigValue {
    let Value::Object(entries) = value else {
        return redact_at(value, &["shell_environment_policy"]);
    };

    let fields = entries
        .iter()
        .map(|(key, item)| ConfigField {
            key: key.clone(),
            value: if key == "set" {
                Redactor.redact_hidden(item)
            } else {
                redact_at(item, &["shell_environment_policy", key])
            },
        })
        .collect();

    ConfigValue::Map(fields)
}

/// `value` with the string of every `*_env_var` and `env_key` written as `${NAME}`, so the
/// redactor shows it as the variable it names (its value is never read). Other strings are kept,
/// and so is a "name" the redactor would hide on its own (a key pasted where the name belongs).
pub(super) fn marked_env_names(value: &Value) -> Value {
    match value {
        Value::Object(entries) => Value::Object(
            entries
                .iter()
                .map(|(key, item)| {
                    let names_variable = key.ends_with("_env_var") || key == "env_key";
                    let marked = match item {
                        Value::String(name) if names_variable && is_variable_name(name) => {
                            Value::String(format!("${{{name}}}"))
                        }
                        other => marked_env_names(other),
                    };

                    (key.clone(), marked)
                })
                .collect::<Map<String, Value>>(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(marked_env_names).collect()),
        other => other.clone(),
    }
}

/// Longest variable name without `_` shown as a name (`TOKEN`, `GITHUB_PAT`); a longer run of capitals
/// and digits could be a pasted key.
const MAX_PLAIN_NAME_CHARS: usize = 16;

/// A conventional environment variable name: capitals, digits and `_`, not starting with a digit,
/// with a `_` or short, and nothing the redactor takes for a secret by itself. Anything else stays
/// hidden, since the field may hold a key pasted where the name belongs.
fn is_variable_name(name: &str) -> bool {
    let mut characters = name.chars();

    let shaped = characters
        .next()
        .is_some_and(|first| first.is_ascii_uppercase() || first == '_')
        && characters.all(|character| character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_');
    let word_like = name.contains('_') || name.chars().count() <= MAX_PLAIN_NAME_CHARS;

    shaped && word_like && matches!(Redactor.redact(&Value::String(name.to_owned())), ConfigValue::Text(_))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::model::SecretKind;

    fn field<'a>(fields: &'a [ConfigField], path: &[&str]) -> &'a ConfigValue {
        let (first, rest) = path.split_first().unwrap();
        let mut value = &fields.iter().find(|field| field.key == *first).unwrap().value;

        for key in rest {
            let ConfigValue::Map(entries) = value else {
                panic!("{key} is not in a map");
            };
            value = &entries.iter().find(|field| field.key == *key).unwrap().value;
        }

        value
    }

    #[test]
    fn codex_secrets_are_hidden_and_variable_names_shown() {
        let fields = config_fields(&json!({
            "model": "gpt-5",
            "shell_environment_policy": { "inherit": "core", "set": { "PATH_EXTRA": "/opt/bin" } },
            "model_providers": { "azure": {
                "env_key": "AZURE_OPENAI_API_KEY",
                "query_params": { "api-version": "2025-04-01" },
            } },
            "mcp_servers": { "remote": {
                "url": "https://mcp.example.com",
                "bearer_token_env_var": "REMOTE_TOKEN",
                "http_headers": { "X-Team": "alpha" },
            } },
        }))
        .unwrap();

        assert_eq!(field(&fields, &["model"]), &ConfigValue::Text("gpt-5".to_owned()));
        assert_eq!(
            field(&fields, &["shell_environment_policy", "inherit"]),
            &ConfigValue::Text("core".to_owned())
        );
        assert_eq!(
            field(&fields, &["shell_environment_policy", "set", "PATH_EXTRA"]),
            &ConfigValue::Secret(SecretKind::Value)
        );
        assert_eq!(
            field(&fields, &["model_providers", "azure", "env_key"]),
            &ConfigValue::Secret(SecretKind::EnvRef {
                name: "AZURE_OPENAI_API_KEY".to_owned()
            })
        );
        assert_eq!(
            field(&fields, &["model_providers", "azure", "query_params", "api-version"]),
            &ConfigValue::Secret(SecretKind::Value)
        );
        assert_eq!(
            field(&fields, &["mcp_servers", "remote", "bearer_token_env_var"]),
            &ConfigValue::Secret(SecretKind::EnvRef {
                name: "REMOTE_TOKEN".to_owned()
            })
        );
        assert_eq!(
            field(&fields, &["mcp_servers", "remote", "http_headers", "X-Team"]),
            &ConfigValue::Secret(SecretKind::Value)
        );
    }

    #[test]
    fn a_key_pasted_where_a_variable_name_belongs_stays_hidden() {
        for key in [
            ["A9f3K", "q81ZtXw0Lm7", "Bv2Rc6Yp4Ns"].concat(),
            ["Q7W2E9R4T1", "Y8U3I6O5P0", "A2S7D4F9G1"].concat(),
            ["3f9a1c7e5b2d", "8046af19c3e7"].concat(),
        ] {
            let fields = config_fields(&json!({
                "mcp_servers": { "remote": { "url": "https://mcp.example.com", "bearer_token_env_var": key } },
            }))
            .unwrap();

            assert_eq!(
                field(&fields, &["mcp_servers", "remote", "bearer_token_env_var"]),
                &ConfigValue::Secret(SecretKind::Value),
                "{key}"
            );
        }
    }

    #[test]
    fn a_config_file_must_hold_a_table() {
        assert!(config_fields(&json!([1])).is_err());
        assert_eq!(config_fields(&Value::Null), Ok(Vec::new()));
    }
}
