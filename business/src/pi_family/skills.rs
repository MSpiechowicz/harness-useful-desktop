//! `SKILL.md` discovery shared by OMP and PI.
//!
//! A skill is a directory holding `SKILL.md` with YAML frontmatter. Like both harnesses, skills
//! without a `description` are skipped, `enabled: false` hides a skill, the name falls back to
//! the directory name and names containing path separators are rejected.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::config_files::text_field;
use super::frontmatter::read_frontmatter;
use super::resources::{MAX_WALK_ENTRIES, Truncation, WalkBudget, origin_label, walk_tree_budgeted};
use crate::env::Env;
use crate::model::{ExtensionScope, SkillInfo};
use crate::security::canonical_within;

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

impl SkillRoot {
    /// A root at `dir` labeled by [`origin_label`] (project-relative inside `project`).
    pub fn labeled(env: &Env, project: Option<&Path>, dir: PathBuf, scope: ExtensionScope) -> Self {
        let origin = origin_label(env, project, &dir);

        Self { dir, scope, origin }
    }
}

/// Skills directly below `root.dir` (`<dir>/<skill>/SKILL.md`), descending up to `max_depth`
/// levels through directories that are not skills themselves (`1` = direct children only).
/// Symlinked skill directories count; symlink loops are not followed (see [`walk_tree`](super::resources::walk_tree)).
pub fn scan_skills(root: &SkillRoot, max_depth: usize) -> Vec<SkillInfo> {
    scan(root, max_depth, None, &mut WalkBudget::new(MAX_WALK_ENTRIES))
}

/// Each of `roots` scanned with a walk budget of its own (so one cannot use up another's), any
/// root cut short noted in `truncation` under its scope.
pub fn scan_roots(roots: &[SkillRoot], max_depth: usize, truncation: &mut Truncation) -> Vec<SkillInfo> {
    let mut found = Vec::new();

    for root in roots {
        let mut walk = WalkBudget::new(MAX_WALK_ENTRIES);
        found.extend(scan_skills_budgeted(root, max_depth, None, &mut walk));
        truncation.note(root.scope, walk.truncated());
    }

    found
}

/// [`scan_skills`] charging the walk to `budget` (shared by many roots) and, with a `boundary` (a
/// package directory), confined to it: directories and `SKILL.md` files that resolve outside it,
/// through `..` or symlinks, are neither entered nor read.
///
/// Skill directories resolving to one directory (links to one skill) are read once, the first in
/// walk order.
pub fn scan_skills_budgeted(
    root: &SkillRoot,
    max_depth: usize,
    boundary: Option<&Path>,
    budget: &mut WalkBudget,
) -> Vec<SkillInfo> {
    scan(root, max_depth, boundary, budget)
}

fn scan(root: &SkillRoot, max_depth: usize, boundary: Option<&Path>, budget: &mut WalkBudget) -> Vec<SkillInfo> {
    let outside = |path: &Path| boundary.is_some_and(|boundary| canonical_within(boundary, path).is_err());
    let mut skills = Vec::new();
    let mut read_dirs = HashSet::new();

    walk_tree_budgeted(&root.dir, max_depth, budget, &mut |entry| {
        if !entry.is_dir || entry.name.starts_with('.') || outside(&entry.path) {
            return false;
        }

        let skill_file = entry.path.join(SKILL_FILE);
        if !skill_file.is_file() {
            return true;
        }

        let resolved = dunce::canonicalize(&entry.path).unwrap_or_else(|_| entry.path.clone());
        let first_read = read_dirs.insert(resolved);

        if first_read && !outside(&skill_file) {
            skills.extend(load_skill(root, &entry.name, &skill_file));
        }

        false
    });

    skills.sort_by(|left, right| left.name.cmp(&right.name).then_with(|| left.path.cmp(&right.path)));
    skills
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
        package: None,
    })
}

/// Keeps the first skill of each name (callers pass roots in precedence order).
pub fn dedupe_skills(skills: Vec<SkillInfo>) -> Vec<SkillInfo> {
    let mut seen = HashSet::new();

    skills
        .into_iter()
        .filter(|skill| seen.insert(skill.name.clone()))
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(name: &str, origin: &str) -> SkillInfo {
        SkillInfo {
            name: name.to_owned(),
            description: String::new(),
            scope: ExtensionScope::User,
            origin: origin.to_owned(),
            path: String::new(),
            package: None,
        }
    }

    #[test]
    fn the_first_skill_of_each_name_wins_in_order() {
        let skills = vec![
            skill("b", "first"),
            skill("a", "first"),
            skill("b", "second"),
            skill("a", "second"),
            skill("c", "first"),
        ];

        let kept: Vec<(String, String)> = dedupe_skills(skills)
            .into_iter()
            .map(|skill| (skill.name, skill.origin))
            .collect();

        let expected: Vec<(String, String)> = [("b", "first"), ("a", "first"), ("c", "first")]
            .iter()
            .map(|(name, origin)| ((*name).to_owned(), (*origin).to_owned()))
            .collect();
        assert_eq!(kept, expected);
    }
}
