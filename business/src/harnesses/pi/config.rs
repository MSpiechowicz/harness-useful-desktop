//! PI configuration view: agent-directory files and the trust-gated project `.pi/` layer.
//!
//! `auth.json`, `mcp-auth.json` and `models-store.json` hold credentials; they are never listed
//! and the deny-list refuses them anyway.

use std::path::Path;

use super::pi_paths;
use crate::env::Env;
use crate::model::{ConfigFormat, ConfigLayer, ConfigView, HarnessId, Notice, NoticeLevel};
use crate::pi_family::{SectionText, optional_section, section, source};

/// Shown whenever a project is open.
pub const TRUST_NOTICE: &str = "Project resources load only for trusted projects (Pi project trust).";

const CREDENTIALS_NOTICE: &str = "Credential files (auth.json, mcp-auth.json, models-store.json) are never read.";

/// `(section text, file name)` of the agent-directory files; the first is always listed.
const USER_FILES: &[(SectionText<'static>, &str)] = &[
    (
        SectionText {
            id: "user.settings",
            title: "Settings",
            description: "Default model, thinking level, theme, resources and packages.",
        },
        "settings.json",
    ),
    (
        SectionText {
            id: "user.models",
            title: "Models",
            description: "Custom providers, endpoints and models.",
        },
        "models.json",
    ),
    (
        SectionText {
            id: "user.keybindings",
            title: "Keybindings",
            description: "Terminal UI shortcuts.",
        },
        "keybindings.json",
    ),
    (
        SectionText {
            id: "user.mcp",
            title: "MCP servers",
            description: "Model Context Protocol servers available in every project.",
        },
        "mcp.json",
    ),
    (
        SectionText {
            id: "user.trust",
            title: "Project trust",
            description: "Saved project trust decisions.",
        },
        "trust.json",
    ),
];

/// `(section text, file name)` of the project `.pi/` files.
const PROJECT_FILES: &[(SectionText<'static>, &str)] = &[
    (
        SectionText {
            id: "project.settings",
            title: "Project settings",
            description: "Overrides for this project; Pi applies them only to trusted projects.",
        },
        "settings.json",
    ),
    (
        SectionText {
            id: "project.mcp",
            title: "Project MCP servers",
            description: "Servers for this project; Pi starts them only in trusted projects.",
        },
        "mcp.json",
    ),
];

/// See the module docs.
pub fn pi_config_view(env: &Env, project: Option<&Path>) -> ConfigView {
    let paths = pi_paths(env, project);
    let mut sections = Vec::new();

    for (index, (text, file)) in USER_FILES.iter().enumerate() {
        let file_source = source(&paths.agent_dir.join(file), ConfigLayer::User, None, ConfigFormat::Json);

        if index == 0 {
            sections.push(section(*text, file_source));
        } else {
            sections.extend(optional_section(*text, file_source));
        }
    }

    if let Some(dir) = &paths.project_dir {
        for (text, file) in PROJECT_FILES {
            let file_source = source(&dir.join(file), ConfigLayer::Project, None, ConfigFormat::Json);
            sections.extend(optional_section(*text, file_source));
        }
    }

    let mut notices = Vec::new();
    if project.is_some() {
        notices.push(info(TRUST_NOTICE));
    }
    notices.push(info(CREDENTIALS_NOTICE));

    ConfigView {
        harness: HarnessId::Pi,
        sections,
        notices,
    }
}

/// An informational notice.
pub fn info(message: &str) -> Notice {
    Notice {
        level: NoticeLevel::Info,
        message: message.to_owned(),
    }
}
