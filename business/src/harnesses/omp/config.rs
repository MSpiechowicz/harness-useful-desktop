//! OMP configuration view: settings (`config.yml`), MCP files, the active profile and the
//! project layer.

use std::path::{Path, PathBuf};

use super::paths::{omp_paths, profile_names};
use crate::env::Env;
use crate::harness::HarnessPaths;
use crate::model::{
    ConfigFormat, ConfigLayer, ConfigSection, ConfigSource, ConfigView, HarnessId, Notice, NoticeLevel,
};
use crate::pi_family::{Document, SectionText, first_existing, optional_section, read_document, section_from, source};

/// Main settings file names, canonical first (`MAIN_CONFIG_FILENAMES` in OMP).
const MAIN_CONFIG_FILES: [&str; 2] = ["config.yml", "config.yaml"];

const DATABASES_NOTICE: &str = "Credential and history databases (agent.db and other *.db files) are never read.";

/// The layer and profile name of user-level files (a profile replaces the default agent dir).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserLayer {
    /// `User`, or `Profile` when a profile is active.
    pub layer: ConfigLayer,
    /// The active profile.
    pub profile: Option<String>,
}

impl UserLayer {
    /// The user layer of `paths`.
    pub fn of(paths: &HarnessPaths) -> Self {
        let layer = match paths.profile {
            Some(_) => ConfigLayer::Profile,
            None => ConfigLayer::User,
        };

        Self {
            layer,
            profile: paths.profile.clone(),
        }
    }
}

/// One MCP configuration file OMP reads.
#[derive(Debug, Clone)]
pub struct McpFile {
    /// Section identity.
    pub text: SectionText<'static>,
    /// Location.
    pub path: PathBuf,
    /// Layer the file belongs to.
    pub layer: ConfigLayer,
    /// Key path of the servers object (`mcpServers`, or `mcp.servers` for VS Code).
    pub servers_key: &'static [&'static str],
}

impl McpFile {
    /// The file's source; user-level files carry the active profile.
    pub fn source(&self, user: &UserLayer) -> ConfigSource {
        let profile = match self.layer {
            ConfigLayer::Project => None,
            _ => user.profile.as_deref(),
        };

        source(&self.path, self.layer, profile, ConfigFormat::Json)
    }
}

/// Every MCP file in OMP precedence order (first definition of a server name wins): native
/// project files, native user files, VS Code, then standalone project files.
pub fn mcp_files(paths: &HarnessPaths, project: Option<&Path>) -> Vec<McpFile> {
    let user_layer = UserLayer::of(paths).layer;
    let mut files = Vec::new();

    if let Some(dir) = &paths.project_dir {
        files.push(project_file("project.mcp", "Project MCP servers", dir.join("mcp.json")));
        files.push(project_file(
            "project.mcpDot",
            "Project MCP servers",
            dir.join(".mcp.json"),
        ));
    }

    for (id, name) in [("user.mcp", "mcp.json"), ("user.mcpDot", ".mcp.json")] {
        let path = paths.agent_dir.join(name);
        files.push(mcp_file(id, "MCP servers", path, user_layer, MCP_SERVERS));
    }

    if let Some(project) = project {
        let vscode = project.join(".vscode").join("mcp.json");
        files.push(mcp_file(
            "project.vscodeMcp",
            "VS Code MCP servers",
            vscode,
            ConfigLayer::Project,
            VSCODE_SERVERS,
        ));
        files.push(project_file(
            "project.rootMcp",
            "Standalone MCP servers",
            project.join("mcp.json"),
        ));
        files.push(project_file(
            "project.rootMcpDot",
            "Standalone MCP servers",
            project.join(".mcp.json"),
        ));
    }

    files
}

const MCP_SERVERS: &[&str] = &["mcpServers"];
const VSCODE_SERVERS: &[&str] = &["mcp", "servers"];

fn project_file(id: &'static str, title: &'static str, path: PathBuf) -> McpFile {
    mcp_file(id, title, path, ConfigLayer::Project, MCP_SERVERS)
}

fn mcp_file(
    id: &'static str,
    title: &'static str,
    path: PathBuf,
    layer: ConfigLayer,
    servers_key: &'static [&'static str],
) -> McpFile {
    McpFile {
        text: SectionText {
            id,
            title,
            description: "Model Context Protocol servers OMP starts for sessions.",
        },
        path,
        layer,
        servers_key,
    }
}

/// The main settings file of the user (or profile) layer and its parsed content.
pub fn user_settings(paths: &HarnessPaths) -> (PathBuf, Document) {
    let candidates: Vec<PathBuf> = MAIN_CONFIG_FILES
        .iter()
        .map(|name| paths.agent_dir.join(name))
        .collect();
    let path = first_existing(&candidates);
    let document = read_document(&path, ConfigFormat::Yaml);

    (path, document)
}

/// The project `.omp/config.yml`, if a project is open.
pub fn project_settings(paths: &HarnessPaths) -> Option<(PathBuf, Document)> {
    let path = paths.project_dir.as_ref()?.join("config.yml");
    let document = read_document(&path, ConfigFormat::Yaml);

    Some((path, document))
}

/// See the module docs.
pub fn omp_config_view(env: &Env, project: Option<&Path>) -> ConfigView {
    let paths = omp_paths(env, project);
    let user = UserLayer::of(&paths);
    let mut sections = vec![settings_section(&paths, &user)];

    let mcp: Vec<McpFile> = mcp_files(&paths, project);
    let (user_mcp, project_mcp): (Vec<McpFile>, Vec<McpFile>) =
        mcp.into_iter().partition(|file| file.layer != ConfigLayer::Project);

    sections.extend(user_mcp.iter().filter_map(|file| mcp_section(file, &user)));

    if let Some(dir) = &paths.project_dir {
        sections.extend(project_settings_sections(dir));
    }

    sections.extend(project_mcp.iter().filter_map(|file| mcp_section(file, &user)));

    let mut notices = Vec::new();
    notices.extend(profiles_notice(&paths));
    notices.push(Notice {
        level: NoticeLevel::Info,
        message: DATABASES_NOTICE.to_owned(),
    });

    ConfigView {
        harness: HarnessId::Omp,
        sections,
        notices,
    }
}

fn settings_section(paths: &HarnessPaths, user: &UserLayer) -> ConfigSection {
    let (path, document) = user_settings(paths);
    let file = source(&path, user.layer, user.profile.as_deref(), ConfigFormat::Yaml);

    let text = match user.profile {
        Some(_) => SectionText {
            id: "profile.settings",
            title: "Profile settings",
            description: "Settings of the active profile; OMP uses them instead of the default agent settings.",
        },
        None => SectionText {
            id: "user.settings",
            title: "Settings",
            description: "Global OMP settings: models, theme, tools and approvals.",
        },
    };

    section_from(text, file, &document)
}

fn mcp_section(file: &McpFile, user: &UserLayer) -> Option<ConfigSection> {
    optional_section(file.text, file.source(user))
}

fn project_settings_sections(dir: &Path) -> Vec<ConfigSection> {
    let yaml = SectionText {
        id: "project.settings",
        title: "Project settings",
        description: "Overrides OMP applies in this project.",
    };
    let json = SectionText {
        id: "project.settingsJson",
        title: "Project settings (JSON)",
        description: "Legacy JSON overrides for this project.",
    };

    let files = [
        (yaml, dir.join("config.yml"), ConfigFormat::Yaml),
        (json, dir.join("settings.json"), ConfigFormat::Json),
    ];

    files
        .into_iter()
        .filter_map(|(text, path, format)| optional_section(text, source(&path, ConfigLayer::Project, None, format)))
        .collect()
}

/// Names of the profiles (never their content), pointing out the active one.
fn profiles_notice(paths: &HarnessPaths) -> Option<Notice> {
    let others: Vec<String> = profile_names(paths)
        .into_iter()
        .filter(|name| Some(name) != paths.profile.as_ref())
        .collect();

    let message = match (&paths.profile, others.is_empty()) {
        (Some(active), true) => format!("Active profile: {active}."),
        (Some(active), false) => format!("Active profile: {active}. Other profiles: {}.", others.join(", ")),
        (None, false) => format!(
            "Profiles: {}. Start OMP with --profile <name> to use one.",
            others.join(", ")
        ),
        (None, true) => return None,
    };

    Some(Notice {
        level: NoticeLevel::Info,
        message,
    })
}
