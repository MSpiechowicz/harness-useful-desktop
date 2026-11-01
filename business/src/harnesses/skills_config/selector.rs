//! The model selectors of the Harness Skills model roles, checked like Harness Skills' own
//! parsers do (harness-useful-skills `claude/models.js`, `opencode/models.js` and
//! `codex/models.js`), so a row the app writes is one Harness Skills accepts.

use std::sync::LazyLock;

use regex::Regex;

use crate::error::{BusinessError, Result};
use crate::model::HarnessId;

/// Claude Code efforts.
const CLAUDE_EFFORTS: &[&str] = &["low", "medium", "high", "xhigh", "max"];

/// OpenCode reasoning variants. Any other `:suffix` is part of the model ID.
const OPENCODE_VARIANTS: &[&str] = &["none", "minimal", "low", "medium", "high", "xhigh", "max"];

/// Codex reasoning efforts. Any other `:suffix` is part of the model ID.
const CODEX_EFFORTS: &[&str] = &["none", "minimal", "low", "medium", "high", "xhigh"];

static CLAUDE_MODEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(inherit|opus|sonnet|haiku|fable|claude-[a-z0-9][a-z0-9.-]{0,80})$").expect("static regex is valid")
});

static PROVIDER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9][a-z0-9._-]{0,63}$").expect("static regex is valid"));

/// An OpenCode or Codex model ID.
static MODEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-z0-9](?:[A-Za-z0-9._:/@+-]{0,198}[A-Za-z0-9._@+-])?$").expect("static regex is valid")
});

/// Whether `selector` is a valid value of a model role row of `harness`; `invalidInput` with the
/// shape that is expected when it is not, `unsupported` for a harness without model roles.
pub(super) fn validate(harness: HarnessId, selector: &str) -> Result<()> {
    let (valid, shape) = match harness {
        HarnessId::Claude => (
            claude(selector),
            "the model must look like model or model:effort, for example claude-opus-5-5:high",
        ),
        HarnessId::OpenCode => (
            opencode(selector),
            "the model must look like provider/model or provider/model:variant, or be inherit",
        ),
        HarnessId::Codex => (
            codex(selector),
            "the model must look like model or model:effort, for example gpt-5.5:high, or be inherit",
        ),
        HarnessId::Omp | HarnessId::Pi => {
            return Err(BusinessError::Unsupported(
                "this harness has no Harness Skills model roles file".to_owned(),
            ));
        }
    };

    if valid {
        Ok(())
    } else {
        Err(BusinessError::InvalidInput(shape.to_owned()))
    }
}

/// `<model>[:<effort>]` with an optional `anthropic/` prefix.
fn claude(selector: &str) -> bool {
    let unprefixed = selector.strip_prefix("anthropic/").unwrap_or(selector);
    let mut parts = unprefixed.split(':');
    let model = parts.next().unwrap_or_default();
    let effort = parts.next();

    parts.next().is_none()
        && CLAUDE_MODEL.is_match(model)
        && effort.is_none_or(|effort| CLAUDE_EFFORTS.contains(&effort))
}

/// `inherit`, or `<provider>/<model>[:<variant>]`.
fn opencode(selector: &str) -> bool {
    if selector == "inherit" {
        return true;
    }

    let model = without_suffix(selector, OPENCODE_VARIANTS);

    match model.split_once('/') {
        Some((provider, id)) => !provider.is_empty() && PROVIDER.is_match(provider) && MODEL.is_match(id),
        None => false,
    }
}

/// `<model>[:<effort>]` or `inherit[:<effort>]`.
fn codex(selector: &str) -> bool {
    let model = without_suffix(selector, CODEX_EFFORTS);

    model == "inherit" || MODEL.is_match(model)
}

/// `selector` without its last `:suffix` when that is one of `known`.
fn without_suffix<'a>(selector: &'a str, known: &[&str]) -> &'a str {
    match selector.rsplit_once(':') {
        Some((model, suffix)) if known.contains(&suffix) => model,
        _ => selector,
    }
}
