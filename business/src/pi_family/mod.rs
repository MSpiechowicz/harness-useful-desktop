//! Parsing shared by the PI-family harnesses (OMP is a PI fork and keeps its file formats).
//!
//! Adapters compose these helpers instead of copying code: configuration files into redacted
//! sections ([`config_files`]), Markdown frontmatter ([`frontmatter`]), `SKILL.md` discovery
//! ([`skills`]), MCP server entries ([`mcp_config`]) and shallow scans for themes and extension
//! modules ([`resources`]). Every file read goes through [`crate::security::SafeReader`].

pub mod config_files;
pub mod frontmatter;
pub mod mcp_config;
pub mod resources;
pub mod skills;

pub use config_files::{
    Document, SectionText, first_existing, optional_section, parse_text, read_document, read_json, section,
    section_from, source, string_list, text_field,
};
pub use frontmatter::{parse_frontmatter, read_frontmatter};
pub use mcp_config::{ParsedServer, parse_servers};
pub use resources::{
    builtin_themes, extension_module, extension_module_paths, extension_name, list_dir, origin_label, package_manifest,
    themes_in_dir, tilde,
};
pub use skills::{SKILL_FILE, SkillRoot, dedupe_skills, load_skill, project_ancestors, scan_skills};
