//! The model roles Harness Skills uses with OMP, the one-click mapping of the missing ones and
//! changing the model of a role the settings map.
//!
//! Harness Skills writes `harness-useful-skills/roles.json` below the agent dir (`version` 1, the
//! `roles`, the `agents` that use them and the roles in `inheritWhenAbsent` that follow the
//! session model until mapped). The file is untrusted: size capped, strictly shaped, and ignored
//! as a whole when anything is off. A missing file (older skills) yields no roles.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::config::user_settings;
use super::paths::omp_paths;
use super::roles_edit::{Edit, apply_edit, set_model};
use crate::ecosystem::skills_paths::skills_data_path;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::harness::HarnessPaths;
use crate::line_edit::edit_text_file;
use crate::model::UnmappedRole;
use crate::security::SafeReader;

/// Largest `roles.json` read.
const MAX_ROLES_BYTES: u64 = 16 * 1024;

/// Most roles or agents one file may list.
const MAX_ENTRIES: usize = 64;

/// A key under `modelRoles` the settings may hold (OMP's own roles and any the user added).
static ROLE_KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$").expect("static regex is valid"));

static NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z][a-z-]{0,39}$").expect("static regex is valid"));

/// `provider/model` with an optional `:thinking` suffix, as OMP writes them under `modelRoles`.
static SELECTOR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}/[A-Za-z0-9][A-Za-z0-9._:@+/-]{0,199}$")
        .expect("static regex is valid")
});

/// What Harness Skills published in `roles.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillsRoles {
    /// Every role, in file order.
    pub roles: Vec<String>,
    /// `(agent, role)` pairs in file order.
    pub agents: Vec<(String, String)>,
    /// Roles whose agents follow the session model until the role is mapped.
    pub inherit_when_absent: Vec<String>,
}

impl SkillsRoles {
    /// The agents that use `role`.
    pub fn agents_of(&self, role: &str) -> Vec<&str> {
        self.agents
            .iter()
            .filter(|(_, mapped)| mapped == role)
            .map(|(agent, _)| agent.as_str())
            .collect()
    }
}

/// The roles Harness Skills published for the active agent dir, or `None` when the file is
/// missing or does not pass validation.
pub fn read_skills_roles(paths: &HarnessPaths) -> Option<SkillsRoles> {
    let path = skills_data_path(&paths.agent_dir, &["roles.json"]);
    let text = SafeReader::read_to_string(&path, MAX_ROLES_BYTES).ok()?;

    parse_skills_roles(&text)
}

fn parse_skills_roles(text: &str) -> Option<SkillsRoles> {
    let value: Value = serde_json::from_str(text).ok()?;
    let object = value.as_object()?;

    if object.get("version")?.as_u64()? != 1 {
        return None;
    }

    let listed_roles = names(object.get("roles")?.as_array()?)?;
    let listed_inherit = match object.get("inheritWhenAbsent") {
        Some(list) => names(list.as_array()?)?,
        None => Vec::new(),
    };

    let mut agents = Vec::new();
    let listed = object.get("agents")?.as_object()?;

    if listed.len() > MAX_ENTRIES {
        return None;
    }

    for (agent, role) in listed {
        let role = role.as_str()?;

        if !NAME.is_match(agent) || !listed_roles.iter().any(|known| known == role) {
            return None;
        }

        // A role YAML would read as something else is not offered, so its agents stay as they are.
        if !is_yaml_keyword(role) {
            agents.push((agent.clone(), role.to_owned()));
        }
    }

    if !listed_inherit.iter().all(|role| listed_roles.contains(role)) {
        return None;
    }

    let roles = listed_roles.into_iter().filter(|role| !is_yaml_keyword(role)).collect();
    let inherit_when_absent = listed_inherit
        .into_iter()
        .filter(|role| !is_yaml_keyword(role))
        .collect();

    Some(SkillsRoles {
        roles,
        agents,
        inherit_when_absent,
    })
}

/// Words a YAML parser reads as a boolean or null, so `no: model` would not be the key `no`.
fn is_yaml_keyword(name: &str) -> bool {
    matches!(
        name,
        "null" | "true" | "false" | "yes" | "no" | "on" | "off" | "y" | "n"
    )
}

/// Valid, unique names of a JSON list.
fn names(list: &[Value]) -> Option<Vec<String>> {
    if list.len() > MAX_ENTRIES {
        return None;
    }

    let mut names: Vec<String> = Vec::new();

    for item in list {
        let name = item.as_str()?;

        if !NAME.is_match(name) || names.iter().any(|seen| seen == name) {
            return None;
        }

        names.push(name.to_owned());
    }

    Some(names)
}

/// The Harness Skills roles missing from the `modelRoles` of `settings` (the parsed file at
/// `settings_path`) that [`map_role`] can add. Empty unless the file already maps roles in a
/// block and the edit applies to its layout (a dry run per role), so no row offers a mapping
/// that would then be refused.
pub fn unmapped_roles(paths: &HarnessPaths, settings_path: &Path, settings: Option<&Value>) -> Vec<UnmappedRole> {
    let Some(mapped) = settings
        .and_then(|value| value.get("modelRoles"))
        .and_then(Value::as_object)
        .filter(|roles| !roles.is_empty())
    else {
        return Vec::new();
    };

    let Some(skills) = read_skills_roles(paths) else {
        return Vec::new();
    };

    let Ok(text) = SafeReader::read_to_string(settings_path, SafeReader::DEFAULT_MAX_BYTES) else {
        return Vec::new();
    };

    skills
        .roles
        .iter()
        .filter(|role| !mapped.contains_key(role.as_str()))
        .filter(|role| {
            let edit = Edit {
                role,
                selector: DRY_RUN_SELECTOR,
                agents: skills.agents_of(role),
            };

            apply_edit(&text, &edit).is_ok()
        })
        .map(|role| UnmappedRole {
            role: role.clone(),
            inherits_session_model: skills.inherit_when_absent.contains(role),
        })
        .collect()
}

/// A model for the dry run: only the layout of the file decides whether the edit applies.
const DRY_RUN_SELECTOR: &str = "provider/model";

/// Maps `role` to `selector` in the OMP settings file: adds `modelRoles.<role>` and an
/// `@<role>` entry under `task.agentModelOverrides` for each agent of the role that has none.
/// Nothing else in the file changes. See [`map_role_with`].
pub fn map_role(env: &Env, role: &str, selector: &str) -> Result<()> {
    map_role_with(env, role, selector, || {})
}

/// [`map_role`], running `before_write` after the edit is planned and before the file is
/// replaced, where a change made meanwhile by another program is caught and refused.
pub fn map_role_with(env: &Env, role: &str, selector: &str, before_write: impl FnOnce()) -> Result<()> {
    check_selector(selector)?;

    let paths = omp_paths(env, None);
    let skills = read_skills_roles(&paths)
        .ok_or_else(|| BusinessError::Unsupported("Harness Skills has not published its model roles".to_owned()))?;

    if !skills.roles.iter().any(|known| known == role) {
        return Err(BusinessError::InvalidInput(format!(
            "{role} is not a Harness Skills role"
        )));
    }

    let (settings, _) = user_settings(&paths);
    let edit = Edit {
        role,
        selector,
        agents: skills.agents_of(role),
    };

    edit_text_file(
        &settings,
        "the OMP settings",
        SafeReader::DEFAULT_MAX_BYTES,
        None,
        |text| apply_edit(text, &edit),
        before_write,
    )
}

/// Sets the model of `role`, which `modelRoles` of the OMP settings maps already, to `selector`.
/// Any role the file maps can change, not only a Harness Skills one. Only that value changes.
/// `notFound` for a role the file does not map. See [`set_role_with`].
pub fn set_role(env: &Env, role: &str, selector: &str) -> Result<()> {
    set_role_with(env, role, selector, || {})
}

/// [`set_role`], running `before_write` after the edit is planned and before the file is
/// replaced, where a change made meanwhile by another program is caught and refused.
pub fn set_role_with(env: &Env, role: &str, selector: &str, before_write: impl FnOnce()) -> Result<()> {
    if !ROLE_KEY.is_match(role) {
        return Err(BusinessError::InvalidInput(format!("{role} is not a role name")));
    }

    check_selector(selector)?;

    let (settings, _) = user_settings(&omp_paths(env, None));

    edit_text_file(
        &settings,
        "the OMP settings",
        SafeReader::DEFAULT_MAX_BYTES,
        None,
        |text| set_model(text, role, selector),
        before_write,
    )
}

/// Refuses a model that is not `provider/model[:thinking]`.
fn check_selector(selector: &str) -> Result<()> {
    if !SELECTOR.is_match(selector) || selector.ends_with(':') {
        return Err(BusinessError::InvalidInput(
            "the model must look like provider/model or provider/model:thinking".to_owned(),
        ));
    }

    Ok(())
}
