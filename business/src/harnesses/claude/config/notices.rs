//! The notices of the Claude Code config view: login and provider variables set in the
//! environment (by name only), the memory files Claude loads (by name only) and what is never read.

use std::path::Path;

use super::super::paths::{PROJECT_DIR_NAME, config_dir};
use crate::env::Env;
use crate::model::{Notice, NoticeLevel};
use crate::pi_family::origin_label;

/// Presence-only variables (see [`crate::env::PRESENCE_VARS`]) and what each does to Claude Code.
const SECRET_VARS: &[(&str, &str)] = &[
    ("ANTHROPIC_API_KEY", "is used instead of your Claude login"),
    (
        "ANTHROPIC_AUTH_TOKEN",
        "is sent with every request instead of your Claude login",
    ),
    ("CLAUDE_CODE_OAUTH_TOKEN", "is used instead of your Claude login"),
    ("CLAUDE_CODE_OAUTH_REFRESH_TOKEN", "is used to sign Claude Code in"),
    ("ANTHROPIC_CUSTOM_HEADERS", "adds headers to every request"),
    ("AWS_BEARER_TOKEN_BEDROCK", "signs Claude Code in to Amazon Bedrock"),
    ("ANTHROPIC_FOUNDRY_API_KEY", "signs Claude Code in to Microsoft Foundry"),
    (
        "ANTHROPIC_FOUNDRY_AUTH_TOKEN",
        "signs Claude Code in to Microsoft Foundry",
    ),
];

/// Provider switches and the provider each one selects.
const PROVIDER_SWITCHES: &[(&str, &str)] = &[
    ("CLAUDE_CODE_USE_BEDROCK", "Amazon Bedrock"),
    ("CLAUDE_CODE_USE_VERTEX", "Google Vertex AI"),
    ("CLAUDE_CODE_USE_FOUNDRY", "Microsoft Foundry"),
];

const NEVER_READ_NOTICE: &str = "The login (.credentials.json), prompt history, caches and backups are never read. \
                                 From .claude.json only MCP servers and this project's tool and trust settings are shown.";

/// Every notice of the view, in display order.
pub(super) fn notices(env: &Env, project: Option<&Path>) -> Vec<Notice> {
    let mut notices = Vec::new();

    for (name, effect) in SECRET_VARS {
        if env.is_set(name) {
            notices.push(Notice {
                level: NoticeLevel::Warning,
                message: format!("{name} is set in the environment and {effect}. Its value is not shown."),
            });
        }
    }

    for (name, provider) in PROVIDER_SWITCHES {
        if env.var(name).is_some_and(is_truthy) {
            notices.push(info(format!("{name} is set, so Claude Code runs on {provider}.")));
        }
    }

    notices.extend(memory_files_notice(env, project));
    notices.push(info(NEVER_READ_NOTICE.to_owned()));
    notices
}

/// The CLAUDE.md memory files that exist, by name and layer. Their text is not shown.
fn memory_files_notice(env: &Env, project: Option<&Path>) -> Option<Notice> {
    let mut files = vec![(config_dir(env).join("CLAUDE.md"), "user")];

    if let Some(project) = project {
        files.push((project.join("CLAUDE.md"), "project"));
        files.push((project.join("CLAUDE.local.md"), "project, local"));
        files.push((project.join(PROJECT_DIR_NAME).join("CLAUDE.md"), "project"));
    }

    let names: Vec<String> = files
        .iter()
        .filter(|(path, _)| path.is_file())
        .map(|(path, layer)| format!("{} ({layer})", origin_label(env, project, path)))
        .collect();

    if names.is_empty() {
        return None;
    }

    Some(info(format!(
        "Claude Code also loads these memory files, whose text is not shown here: {}.",
        names.join(", ")
    )))
}

/// True for the values Claude Code reads as on (`1`, `true`, `yes`, `on`).
fn is_truthy(value: &str) -> bool {
    matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")
}

fn info(message: String) -> Notice {
    Notice {
        level: NoticeLevel::Info,
        message,
    }
}
