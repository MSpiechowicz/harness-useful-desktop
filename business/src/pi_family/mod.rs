//! Parsing shared by the PI-family harnesses (OMP is a PI fork and keeps its file formats).
//!
//! Adapters compose these helpers instead of copying code: configuration files into redacted
//! sections ([`config_files`], JSON with comments in [`jsonc`]), Markdown frontmatter
//! ([`frontmatter`]), `SKILL.md` discovery ([`skills`]), MCP server entries ([`mcp_config`]) and
//! shallow scans for themes and extension modules ([`resources`]). Every file read goes through
//! [`crate::security::SafeReader`].
//!
//! The agent RPC protocol lives here too: framing and the dialect trait ([`rpc`]), the shared
//! event decoder ([`events`]), extension UI requests ([`ui_request`]), payload normalization
//! ([`normalize`]) and touched-path extraction ([`touched`]).
//!
//! Saved sessions on disk (listing, branch trees, transcripts, lookup by id) live in
//! [`sessions`].

pub mod config_files;
pub mod events;
pub mod frontmatter;
pub mod jsonc;
pub mod mcp_config;
pub mod normalize;
pub mod resources;
pub mod rpc;
pub mod sessions;
pub mod skills;
pub mod touched;
pub mod ui_request;

pub use config_files::{
    Document, SectionText, first_existing, optional_section, parse_text, read_document, read_json, section,
    section_from, source, string_list, text_field,
};
pub use frontmatter::{parse_frontmatter, read_frontmatter};
pub use mcp_config::{ParsedServer, parse_servers};
pub use resources::{
    MAX_DECLARED_ENTRIES, MAX_WALK_ENTRIES, PerLayer, ScanBudget, Truncation, WalkBudget, builtin_themes,
    canonical_inside, distinct, distinct_targets, distinct_within, extension_module, extension_module_paths,
    extension_name, list_dir, origin_label, package_manifest, themes_in_dir, themes_in_dir_budgeted, tilde,
};
pub use skills::{
    SKILL_FILE, SkillRoot, dedupe_skills, load_skill, project_ancestors, scan_skills, scan_skills_budgeted,
};
