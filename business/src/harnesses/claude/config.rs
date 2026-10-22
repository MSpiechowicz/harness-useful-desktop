//! The Claude Code configuration view (Claude Code 2.1.304).
//!
//! Sections, in display order (see [`files`] and [`global_file`]): managed settings and their
//! drop-ins, the user's `settings.json`, the user's MCP servers from `.claude.json`, the project's
//! `.claude/settings.json` and `.claude/settings.local.json`, and the project's entry in
//! `.claude.json`. What Harness Skills keeps for Claude Code (its model roles and workflow
//! switches, [`skills`]) follows the user's sections. Claude applies them, lowest first: user, project, local, then managed settings
//! over everything. Every file is strict JSON, and missing files are left out.
//!
//! Secrets are hidden by the shared rules plus Claude's own secret settings ([`redact`]). The
//! global file is read through an allow-list. Login and provider variables are reported by name,
//! and the CLAUDE.md memory files by name and layer ([`notices`]).

mod files;
mod global_file;
mod notices;
mod redact;
mod skills;
mod workflow;

use std::path::Path;

pub(super) use self::files::{SettingsFile, managed_files, settings_files};
pub(super) use self::global_file::{GlobalSections, ProjectTrust, global_sections, names_project, project_trust};
use crate::env::Env;
use crate::model::{ConfigFormat, ConfigLayer, ConfigSection, ConfigView, HarnessId};
use crate::pi_family::{Document, SectionText, section_with, source};

/// See the module docs.
pub fn config_view(env: &Env, project: Option<&Path>) -> ConfigView {
    let global = global_sections(env, project);
    let (user, project_files): (Vec<SettingsFile>, Vec<SettingsFile>) = settings_files(env, project)
        .into_iter()
        .partition(|file| file.layer == ConfigLayer::User);

    let mut sections: Vec<ConfigSection> = managed_files(env).iter().chain(&user).map(read_section).collect();
    sections.extend(global.user.map(|(file, document)| section(&file, &document)));

    let (skills, skills_notice) = skills::sections(env, project);
    sections.extend(skills);

    sections.extend(project_files.iter().map(read_section));
    sections.extend(global.project.map(|(file, document)| section(&file, &document)));

    ConfigView {
        harness: HarnessId::Claude,
        sections,
        notices: notices::notices(env, project, skills_notice),
    }
}

fn read_section(file: &SettingsFile) -> ConfigSection {
    section(file, &file.read())
}

/// `file` rendered with the redacted fields of `document`.
fn section(file: &SettingsFile, document: &Document) -> ConfigSection {
    let text = SectionText {
        id: &file.id,
        title: &file.title,
        description: file.description,
    };
    let source = source(&file.path, file.layer, None, ConfigFormat::Json);

    section_with(text, source, document, redact::settings_fields)
}
