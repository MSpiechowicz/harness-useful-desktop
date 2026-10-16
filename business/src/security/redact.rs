//! Redaction of parsed configuration into [`ConfigValue`].
//!
//! Callers parse JSON (or YAML via `serde_norway` into `serde_json::Value`) and pass the value
//! here; only the redacted [`ConfigValue`] may cross the IPC boundary.
//!
//! # Rules (tables live in `security::rules`)
//!
//! Applied to each string leaf, first match wins:
//!
//! 1. **Command** — a value starting with `!` is a secret-producing command: `Secret(Command)`.
//!    Its text is hidden and it is never executed.
//! 2. **Env reference** — exactly `$NAME` or `${NAME}`: `Secret(EnvRef { name })`. The variable
//!    name is shown; its value is never read.
//! 3. **Container** — anything under a key named `env`, `environment` or `headers` (any depth;
//!    keys stay visible), and anything under `auth`/`oauth` except their direct children `type`, `scope`,
//!    `redirectUri`, `callbackPort`, `callbackPath`, `prompt`, `clientId`: `Secret(Value)`.
//!    Numbers in these containers are hidden too.
//! 4. **Key name** — any key on the path whose name (case-insensitive, ignoring `_`/`-`) contains
//!    `apikey`, `token`, `secret`, `password`, `passwd`, `credential`, `authorization`, `bearer`,
//!    `privatekey`, `clientsecret`, `accesskey`, `sessionkey`, `cookie`, `signature`,
//!    `credentialid`: `Secret(Value)`. Numbers and booleans under such keys stay visible
//!    (`maxTokens: 4096` is configuration, not a credential).
//! 5. **Value pattern** — well-known credential formats anywhere in the string (`sk-`, `sk-ant-`,
//!    `ghp_`/`gho_`/`ghs_`/`ghu_`, `github_pat_`, `xox?-`, `AKIA…`, `AIza…`, `glpat-`, `hf_`,
//!    `npm_`, JWTs, `Bearer `/`Basic ` credentials, PEM blocks): `Secret(Value)`.
//! 6. **Entropy** — long random-looking strings (see `rules::is_high_entropy`): `Secret(Value)`.
//!    UUIDs, paths, URLs and word-like identifiers are exempt.
//! 7. **URL userinfo** — otherwise the string is shown, with URL userinfo replaced by `••••`
//!    (`https://••••@host`) and secret-looking query values masked.
//!
//! Booleans and `null` always pass through; numbers keep their textual form.

use serde_json::Value;

use super::rules::{
    AUTH_CONTAINERS, AUTH_PUBLIC_KEYS, SECRET_CONTAINERS, contains_token, env_ref_name, is_high_entropy, is_secret_key,
    mask_tokens, mask_urls, normalize_key,
};
use crate::model::{ConfigField, ConfigValue, SecretKind};

/// How strictly the current position must be hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    /// Only value rules apply.
    Visible,
    /// Strings are secret (key-name rule).
    SecretStrings,
    /// Strings and numbers are secret (container rule).
    SecretContainer,
}

/// The key path of the value being redacted (list items appear as `[index]`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RedactionContext {
    path: Vec<String>,
}

impl RedactionContext {
    /// Context for redacting a value found at `path` inside a larger document
    /// (e.g. `["mcpServers", "github"]`).
    pub fn at<I, S>(path: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            path: path.into_iter().map(Into::into).collect(),
        }
    }

    /// The current key path.
    pub fn path(&self) -> &[String] {
        &self.path
    }

    fn scope(&self) -> Scope {
        let keys: Vec<String> = self.path.iter().map(|key| normalize_key(key)).collect();

        if keys.iter().any(|key| SECRET_CONTAINERS.contains(&key.as_str())) {
            return Scope::SecretContainer;
        }

        if let Some(index) = keys.iter().position(|key| AUTH_CONTAINERS.contains(&key.as_str())) {
            let public_child = keys
                .get(index + 1)
                .is_some_and(|child| AUTH_PUBLIC_KEYS.contains(&child.as_str()));

            if !public_child {
                return Scope::SecretContainer;
            }
        }

        if self.path.iter().any(|key| is_secret_key(key)) {
            return Scope::SecretStrings;
        }

        Scope::Visible
    }
}

/// Converts parsed configuration into [`ConfigValue`] following the module rules.
#[derive(Debug, Clone, Copy, Default)]
pub struct Redactor;

impl Redactor {
    /// Redacts a whole document.
    pub fn redact(&self, value: &Value) -> ConfigValue {
        self.redact_at(value, &mut RedactionContext::default())
    }

    /// Redacts `value` located at `context`'s key path. The context is restored on return.
    pub fn redact_at(&self, value: &Value, context: &mut RedactionContext) -> ConfigValue {
        match value {
            Value::Null => ConfigValue::Null,
            Value::Bool(flag) => ConfigValue::Bool(*flag),
            Value::Number(number) => redact_number(number.to_string(), context.scope()),
            Value::String(text) => redact_string(text, context.scope()),
            Value::Array(items) => ConfigValue::List(
                items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| self.redact_child(format!("[{index}]"), item, context))
                    .collect(),
            ),
            Value::Object(entries) => ConfigValue::Map(
                entries
                    .iter()
                    .map(|(key, item)| ConfigField {
                        key: key.clone(),
                        value: self.redact_child(key.clone(), item, context),
                    })
                    .collect(),
            ),
        }
    }

    fn redact_child(&self, segment: String, value: &Value, context: &mut RedactionContext) -> ConfigValue {
        context.path.push(segment);
        let redacted = self.redact_at(value, context);
        context.path.pop();

        redacted
    }

    /// A git package source (`git:github.com/<owner>/<repo>@<ref>`). The entropy rule judges each
    /// segment on its own, so a mixed-case owner name does not hide the whole source. Commands,
    /// env references, credential formats and userinfo (any but the plain `git@` of an SSH
    /// source, the same rule the CLI actions apply) still make it a secret.
    pub fn redact_package_source(&self, text: &str) -> ConfigValue {
        let userinfo = has_userinfo(text);
        let random_segment = text.split(['/', ':', '@', '#']).any(is_high_entropy);

        if userinfo || random_segment || contains_token(text) {
            return redact_string(text, Scope::SecretContainer);
        }

        redact_string_segmented(text)
    }
}

/// True when the source's authority carries userinfo other than SSH's plain `git@`. A whole
/// authority counts, up to its first `/`, so a password with an `@` in it is never half shown.
fn has_userinfo(source: &str) -> bool {
    let authority = |rest: &str| rest.split('/').next().unwrap_or_default().to_owned();

    if let Some((_, rest)) = source.split_once("://") {
        return authority(rest).contains('@');
    }

    let rest = source.strip_prefix("git:").unwrap_or(source);

    authority(rest).rsplit_once('@').is_some_and(|(user, _)| user != "git")
}

/// Rules 1, 2 and 7 only: the caller has already applied the token and entropy checks.
fn redact_string_segmented(text: &str) -> ConfigValue {
    if text.starts_with('!') {
        return ConfigValue::Secret(SecretKind::Command);
    }

    if let Some(name) = env_ref_name(text) {
        return ConfigValue::Secret(SecretKind::EnvRef { name: name.to_owned() });
    }

    ConfigValue::Text(mask_urls(text))
}

fn redact_number(text: String, scope: Scope) -> ConfigValue {
    if scope == Scope::SecretContainer {
        return ConfigValue::Secret(SecretKind::Value);
    }

    ConfigValue::Number(text)
}

fn redact_string(text: &str, scope: Scope) -> ConfigValue {
    if text.starts_with('!') {
        return ConfigValue::Secret(SecretKind::Command);
    }

    if let Some(name) = env_ref_name(text) {
        return ConfigValue::Secret(SecretKind::EnvRef { name: name.to_owned() });
    }

    if scope != Scope::Visible || contains_token(text) || is_high_entropy(text) {
        return ConfigValue::Secret(SecretKind::Value);
    }

    ConfigValue::Text(mask_urls(text))
}

/// Masks secrets in free text (CLI output, stderr lines): credential formats, `NAME=value`
/// assignments with secret-looking names, URL userinfo and secret query values, and
/// whitespace-separated words that pass the entropy heuristic.
pub fn redact_text(text: &str) -> String {
    let masked = mask_tokens(text);

    if !masked.split_whitespace().any(is_high_entropy) {
        return masked;
    }

    let mut out = String::with_capacity(masked.len());
    let mut rest = masked.as_str();

    // Rebuild word by word so the original whitespace (including newlines) is preserved.
    while let Some(start) = rest.find(|character: char| !character.is_whitespace()) {
        out.push_str(&rest[..start]);
        rest = &rest[start..];

        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let word = &rest[..end];
        out.push_str(if is_high_entropy(word) {
            super::rules::MASK
        } else {
            word
        });
        rest = &rest[end..];
    }

    out.push_str(rest);
    out
}
