//! Codex's configured mode and the app's approval modes (plan Decision 2).
//!
//! - **Configured mode:** `approval_policy` and `sandbox_mode` of the settings layers
//!   ([`Layers`]), plus the model and its reasoning effort for the config view. Unset, Codex asks
//!   on request, in a workspace-write sandbox in a trusted folder and a read-only one elsewhere.
//!   A `granular` approval policy is named as such.
//! - **As an app mode:** `untrusted` with `workspace-write` is Always ask, `on-request` with
//!   `workspace-write` is Write, `never` with `danger-full-access` is Yolo. Other combinations have
//!   no app mode and are named read-only ([`NativeApprovalMode`]).
//! - **Unattended:** `never` (nothing waits for an answer) or `danger-full-access` (commands run
//!   with full access to the machine), which onboarding points out.

use std::path::Path;

use serde_json::Value;

use super::settings::Layers;
use crate::env::Env;
use crate::model::{ApprovalMode, NativeApprovalMode};

/// The approval policy Codex uses when none is set.
const DEFAULT_APPROVAL: &str = "on-request";

/// The sandbox Codex uses when none is set, in a trusted folder.
const DEFAULT_SANDBOX: &str = "workspace-write";

/// The sandbox Codex uses when none is set, in a folder it does not trust.
const UNTRUSTED_SANDBOX: &str = "read-only";

/// What the settings layers configure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ConfiguredMode {
    pub model: Option<String>,
    pub effort: Option<String>,
    /// `untrusted`, `on-failure`, `on-request`, `never` or `granular`; `None` when unset.
    pub approval_policy: Option<String>,
    /// `read-only`, `workspace-write` or `danger-full-access`; `None` when unset.
    pub sandbox_mode: Option<String>,
    /// Whether Codex trusts the project, which picks the default sandbox.
    pub trusted: bool,
}

impl ConfiguredMode {
    /// The mode for `project` (the user's file alone without one).
    pub(super) fn load(env: &Env, project: Option<&Path>) -> Self {
        let layers = Layers::load(env, project);
        let approval_policy = layers.value("approval_policy").and_then(|value| match value {
            Value::String(name) => Some(name.clone()),
            Value::Object(fields) if fields.contains_key("granular") => Some("granular".to_owned()),
            _ => None,
        });

        Self {
            model: layers.text("model"),
            effort: layers.text("model_reasoning_effort"),
            approval_policy,
            sandbox_mode: layers.text("sandbox_mode"),
            trusted: layers.trusted,
        }
    }

    fn approval(&self) -> &str {
        self.approval_policy.as_deref().unwrap_or(DEFAULT_APPROVAL)
    }

    fn sandbox(&self) -> &str {
        let default = if self.trusted {
            DEFAULT_SANDBOX
        } else {
            UNTRUSTED_SANDBOX
        };

        self.sandbox_mode.as_deref().unwrap_or(default)
    }

    /// The mode as Codex names it: `<approval policy> / <sandbox>`.
    pub(super) fn native(&self) -> NativeApprovalMode {
        NativeApprovalMode {
            name: format!("{} / {}", self.approval(), self.sandbox()),
            unattended: self.approval() == "never" || self.sandbox() == "danger-full-access",
        }
    }

    /// The app's approval mode for this combination, when it has one.
    pub(super) fn approval_mode(&self) -> Option<ApprovalMode> {
        match (self.approval(), self.sandbox()) {
            ("untrusted", "workspace-write") => Some(ApprovalMode::AlwaysAsk),
            ("on-request", "workspace-write") => Some(ApprovalMode::Write),
            ("never", "danger-full-access") => Some(ApprovalMode::Yolo),
            _ => None,
        }
    }
}
