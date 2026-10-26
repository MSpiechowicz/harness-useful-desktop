//! Claude Code's own permission mode and the app's approval modes (plan Decision 3).
//!
//! - **Configured mode:** `permissions.defaultMode` of the last settings layer that sets a known
//!   one (managed settings over local over project over user), `manual` read as `default`. Unset,
//!   Claude Code runs `default`.
//! - **As an app mode:** `default` asks before edits and commands, the app's Write. `auto` lets
//!   Claude Code's classifier approve the calls it judges safe and asks about the rest, the app's
//!   Auto. `bypassPermissions` asks for nothing, Yolo. `acceptEdits`, `plan` and `dontAsk` have no
//!   app mode, so the chat names them read-only ([`NativeApprovalMode`]).
//!   `bypassPermissions` and `auto` let tools run without asking, which onboarding points out.
//! - **Launch override:** Always ask and Write start Claude Code in [`WIRE_DEFAULT`] (the engine
//!   adds its ask-all hook for Always ask), Auto in [`WIRE_AUTO`] (no hook). Yolo is never
//!   offered: it would need Claude's bypass mode, which the app never starts.

use std::path::Path;

use super::extensions::settings::Settings;
use crate::env::Env;
use crate::model::{ApprovalMode, NativeApprovalMode};

/// The mode name the app passes to Claude Code (`manual` is reported back as `default`).
pub const WIRE_DEFAULT: &str = "default";

/// The mode name of the app's Auto: Claude Code's classifier decides, the rest still asks.
pub const WIRE_AUTO: &str = "auto";

/// The modes Claude Code 2.1.304 knows, by their settings names.
const MODES: [&str; 6] = [
    WIRE_DEFAULT,
    "acceptEdits",
    "plan",
    WIRE_AUTO,
    "dontAsk",
    "bypassPermissions",
];

/// The modes that let tools run without asking the user.
const UNATTENDED: [&str; 2] = ["bypassPermissions", WIRE_AUTO];

/// The mode Claude Code runs in `project` (the user's layers without one), see the module docs.
pub fn native_approval_mode(env: &Env, project: Option<&Path>) -> NativeApprovalMode {
    let settings = Settings::load(env, project);
    let name = settings
        .texts("/permissions/defaultMode")
        .into_iter()
        .rev()
        .find_map(known)
        .unwrap_or(WIRE_DEFAULT);

    NativeApprovalMode {
        name: name.to_owned(),
        unattended: UNATTENDED.contains(&name),
    }
}

/// The settings name of mode `mode` (`manual` is `default`); `None` for an unknown one.
fn known(mode: &str) -> Option<&'static str> {
    if mode == "manual" {
        return Some(WIRE_DEFAULT);
    }

    MODES.into_iter().find(|known| *known == mode)
}

/// The app's approval mode for Claude Code mode `name`; `None` for a mode the app has none for.
pub fn approval_mode(name: &str) -> Option<ApprovalMode> {
    match name {
        WIRE_DEFAULT => Some(ApprovalMode::Write),
        WIRE_AUTO => Some(ApprovalMode::Auto),
        "bypassPermissions" => Some(ApprovalMode::Yolo),
        _ => None,
    }
}
