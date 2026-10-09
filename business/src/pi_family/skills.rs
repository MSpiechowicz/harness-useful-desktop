//! `SKILL.md` discovery shared by OMP and PI.
//!
//! A skill is a directory holding `SKILL.md` with YAML frontmatter. Like both harnesses, skills
//! without a `description` are skipped, `enabled: false` hides a skill, the name falls back to
//! the directory name and names containing path separators are rejected.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::config_files::text_field;
use super::frontmatter::read_frontmatter;
use super::resources::list_dir;
use crate::env::Env;
use crate::model::{ExtensionScope, SkillInfo};

/// File name of a skill definition.
pub const SKILL_FILE: &str = "SKILL.md";

/// Where to look for skills and how to label what is found.
#[derive(Debug, Clone)]
pub struct SkillRoot {
    /// Directory whose children are skill directories.
    pub dir: PathBuf,
    /// Scope reported for skills found here.
    pub scope: ExtensionScope,
    /// Human-readable origin (e.g. `~/.omp/agent/skills`).
    pub origin: String,
}

/// Skills directly below `root.dir` (`<dir>/<skill>/SKILL.md`), descending up to `max_depth`
/// levels through directories that are not skills themselves (`1` = direct children only).
pub fn scan_skills(root: &SkillRoot, max_depth: usize) -> Vec<SkillInfo> {
    let mut skills = Vec::new();
    collect(root, &root.dir, max_depth, &mut skills);

    skills.sort_by(|left, right| left.name.cmp(&right.name).then_with(|| left.path.cmp(&right.path)));
    skills
}

fn collect(root: &SkillRoot, dir: &Path, depth_left: usize, skills: &mut Vec<SkillInfo>) {
    if depth_left == 0 {
        return;
    }

    for entry in list_dir(dir) {
        if !entry.is_dir || entry.name.starts_with('.') {
            continue;
        }

        let skill_file = entry.path.join(SKILL_FILE);
        if skill_file.is_file() {
            skills.extend(load_skill(root, &entry.name, &skill_file));
        } else {
            collect(root, &entry.path, depth_left - 1, skills);
        }
    }
}

/// One skill from its `SKILL.md`, or `None` when the harness would skip it.
pub fn load_skill(root: &SkillRoot, dir_name: &str, skill_file: &Path) -> Option<SkillInfo> {
    let frontmatter = read_frontmatter(skill_file)?;

    if frontmatter.get("enabled") == Some(&Value::Bool(false)) {
        return None;
    }

    let description = text_field(&frontmatter, "description")?;
    let name = text_field(&frontmatter, "name").unwrap_or_else(|| dir_name.to_owned());

    if name.contains(['/', '\\']) {
        return None;
    }

    Some(SkillInfo {
        name,
        description,
        scope: root.scope,
        origin: root.origin.clone(),
        path: skill_file.to_string_lossy().into_owned(),
    })
}

/// Keeps the first skill of each name (callers pass roots in precedence order).
pub fn dedupe_skills(skills: Vec<SkillInfo>) -> Vec<SkillInfo> {
    let mut kept: Vec<SkillInfo> = Vec::new();

    for skill in skills {
        if !kept.iter().any(|existing| existing.name == skill.name) {
            kept.push(skill);
        }
    }

    kept
}

/// `project` and its ancestors, closest first, as both harnesses walk them for project-local
/// resources: the walk stops after the first directory containing `.git` (the repository root)
/// and never includes the home directory, which holds user rather than project configuration.
pub fn project_ancestors(env: &Env, project: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    for dir in project.ancestors() {
        if dir == env.home {
            break;
        }

        dirs.push(dir.to_path_buf());

        if dir.join(".git").exists() {
            break;
        }
    }

    dirs
}
