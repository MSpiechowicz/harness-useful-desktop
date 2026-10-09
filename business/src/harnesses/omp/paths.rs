//! OMP directory resolution, mirroring `DirResolver` in `@oh-my-pi/pi-utils/src/dirs.ts` (18.6.1).
//!
//! - Config root: `~/<PI_CONFIG_DIR or .omp>`.
//! - Profile: `OMP_PROFILE` (even when empty, which selects the default profile), else
//!   `PI_PROFILE`; invalid names fall back to the default profile like OMP does at startup.
//! - Agent dir: `<root>/profiles/<profile>/agent` for a profile, else `PI_CODING_AGENT_DIR`
//!   (unless it is merely the agent dir derived from `PI_PROFILE`), else `<root>/agent`.
//! - Data (plugins, marketplaces): on Linux and macOS with the default agent dir, an existing
//!   `$XDG_DATA_HOME/omp` (or `$XDG_DATA_HOME/omp/profiles/<profile>`) replaces the profile root.
//! - Project layer: `<project>/.omp` (always `.omp`, independent of `PI_CONFIG_DIR`).

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::env::{Env, OsKind};
use crate::harness::HarnessPaths;
use crate::pi_family::list_dir;

/// Default config directory name.
const CONFIG_DIR_NAME: &str = ".omp";

/// Project-local configuration directory name.
pub const PROJECT_DIR_NAME: &str = ".omp";

/// Application name used under XDG directories.
const APP_NAME: &str = "omp";

static PROFILE_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9][a-z0-9._-]{0,63}$").expect("static regex is valid"));

static WINDOWS_RESERVED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:CON|PRN|AUX|NUL|COM[0-9]|LPT[0-9])(?:\..*)?$").expect("static regex is valid")
});

/// Resolves every OMP directory for `env` (see the module docs).
pub fn omp_paths(env: &Env, project: Option<&Path>) -> HarnessPaths {
    let base_root = base_config_root(env);
    let profile = active_profile(env);

    let profile_root = match &profile {
        Some(name) => base_root.join("profiles").join(name),
        None => base_root.clone(),
    };

    let default_agent = profile_root.join("agent");
    let agent_dir = match (&profile, agent_dir_override(env, &base_root)) {
        (None, Some(dir)) => dir,
        _ => default_agent.clone(),
    };

    let xdg_data = if agent_dir == default_agent {
        xdg_data_root(env, profile.as_deref())
    } else {
        None
    };

    HarnessPaths {
        config_root: base_root,
        agent_dir,
        data_root: xdg_data.unwrap_or(profile_root),
        profile,
        project_dir: project.map(|project| project.join(PROJECT_DIR_NAME)),
    }
}

/// `<data root>/plugins`.
pub fn plugins_dir(paths: &HarnessPaths) -> PathBuf {
    paths.data_root.join("plugins")
}

/// `<data root>/marketplaces.json`.
pub fn marketplaces_registry(paths: &HarnessPaths) -> PathBuf {
    paths.data_root.join("marketplaces.json")
}

/// Valid profile names found under `<root>/profiles`, sorted.
pub fn profile_names(paths: &HarnessPaths) -> Vec<String> {
    list_dir(&paths.config_root.join("profiles"))
        .into_iter()
        .filter(|entry| entry.is_dir)
        .filter_map(|entry| normalize_profile(&entry.name))
        .collect()
}

/// The profile selected by the environment, if any.
pub fn active_profile(env: &Env) -> Option<String> {
    // An explicitly empty OMP_PROFILE selects the default profile; `Env::var` would hide it.
    let raw = env.vars.get("OMP_PROFILE").or_else(|| env.vars.get("PI_PROFILE"))?;
    normalize_profile(raw)
}

/// OMP's `normalizeProfileName`: `None` for the default profile and for invalid names.
fn normalize_profile(raw: &str) -> Option<String> {
    let name = raw.trim();

    if name.is_empty() || name == "default" || name.ends_with('.') {
        return None;
    }

    if !PROFILE_NAME.is_match(name) || WINDOWS_RESERVED.is_match(name) {
        return None;
    }

    Some(name.to_owned())
}

fn base_config_root(env: &Env) -> PathBuf {
    // `path.join(home, name)` in OMP: an absolute-looking name is still joined below home.
    let name = env.var("PI_CONFIG_DIR").unwrap_or(CONFIG_DIR_NAME);
    env.home.join(name.trim_start_matches(['/', '\\']))
}

/// `PI_CODING_AGENT_DIR` when it is absolute and not just the agent dir of `PI_PROFILE`.
fn agent_dir_override(env: &Env, base_root: &Path) -> Option<PathBuf> {
    let dir = PathBuf::from(env.var("PI_CODING_AGENT_DIR")?);

    if !dir.is_absolute() {
        return None;
    }

    let pi_profile = env.var("PI_PROFILE").and_then(normalize_profile);
    let derived = pi_profile.map(|name| base_root.join("profiles").join(name).join("agent"));

    if derived.as_ref() == Some(&dir) {
        return None;
    }

    Some(dir)
}

/// An existing `$XDG_DATA_HOME/omp[/profiles/<profile>]` on Linux and macOS.
fn xdg_data_root(env: &Env, profile: Option<&str>) -> Option<PathBuf> {
    if env.os == OsKind::Windows {
        return None;
    }

    let app_root = PathBuf::from(env.var("XDG_DATA_HOME")?).join(APP_NAME);
    let root = match profile {
        Some(name) => app_root.join("profiles").join(name),
        None => app_root,
    };

    root.is_dir().then_some(root)
}
