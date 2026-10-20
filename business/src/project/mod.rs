//! Project directories: inspection and the server-managed active/recent list.
//!
//! New directories enter only through [`Projects::open`], which the shell calls with the result of
//! the native folder picker. Paths sent by the webview are accepted only when they are already
//! known (the active project or a recent one), so the webview cannot point the app at an
//! arbitrary directory.

mod scopes;

use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{AppPrefs, ProjectInfo};
use crate::prefs::PrefsStore;
use crate::security::validate_project_dir;

pub use scopes::{KEYS_AT_ONCE, ProjectKeys, RETRY_AFTER, with_keys};

impl ProjectInfo {
    /// Describes the directory at `path` without reading any file: the name is the last path
    /// component, and a `.git` directory or file (worktrees, submodules) marks a repository root.
    /// Git is never invoked.
    pub fn inspect(path: &Path) -> Self {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());

        Self {
            path: path.to_string_lossy().into_owned(),
            name,
            exists: path.is_dir(),
            is_git_repo: path.join(".git").exists(),
        }
    }
}

/// Project operations over a [`PrefsStore`]. Every change is one load-modify-save cycle.
#[derive(Debug, Clone, Copy)]
pub struct Projects<'a> {
    store: &'a PrefsStore,
    env: &'a Env,
}

impl<'a> Projects<'a> {
    /// Operations on the prefs in `store`, resolving `~` with `env`.
    pub fn new(store: &'a PrefsStore, env: &'a Env) -> Self {
        Self { store, env }
    }

    /// Opens a directory chosen in the native picker: validates it (see [`validate_project_dir`]),
    /// makes it the active project and, when recent projects are remembered, moves it to the front
    /// of the recent list.
    pub fn open(&self, path: &Path) -> Result<ProjectInfo> {
        let canonical = validate_project_dir(self.env, path)?;
        let stored = utf8(&canonical)?;

        self.store.update(|prefs| {
            prefs.active_project = Some(stored.clone());

            if prefs.remember_recent_projects {
                prefs.recent_projects.retain(|recent| *recent != stored);
                prefs.recent_projects.insert(0, stored.clone());
            }
        })?;

        Ok(ProjectInfo::inspect(&canonical))
    }

    /// Re-opens a known project (the active one or a recent one); any other path is `Forbidden`.
    pub fn reopen(&self, path: &str) -> Result<ProjectInfo> {
        let known = self.require_known(&self.store.load(), path)?;

        self.open(&known)
    }

    /// Clears the active project.
    pub fn close(&self) -> Result<()> {
        self.store.update(|prefs| prefs.active_project = None).map(|_| ())
    }

    /// Removes `path` from the recent projects (clearing the active project when it is `path`)
    /// and returns the remaining recent projects.
    pub fn forget(&self, path: &str) -> Result<Vec<ProjectInfo>> {
        let prefs = self.store.update(|prefs| {
            prefs.recent_projects.retain(|recent| recent != path);

            if prefs.active_project.as_deref() == Some(path) {
                prefs.active_project = None;
            }
        })?;

        Ok(inspect_all(&prefs.recent_projects))
    }

    /// The recent projects, most recent first, with their current `exists` flags.
    pub fn recent(&self) -> Vec<ProjectInfo> {
        inspect_all(&self.store.load().recent_projects)
    }

    /// The active project, if one is set.
    pub fn active(&self) -> Option<PathBuf> {
        self.store.load().active_project.map(PathBuf::from)
    }

    /// Authorizes a project argument sent by the webview: `None` stays `None`; a path must be the
    /// active project or a recent one (else `Forbidden`) and pass [`validate_project_dir`].
    /// Returns the canonical directory.
    pub fn authorize(&self, path: Option<&str>) -> Result<Option<PathBuf>> {
        let Some(path) = path else {
            return Ok(None);
        };

        let known = self.require_known(&self.store.load(), path)?;

        validate_project_dir(self.env, &known).map(Some)
    }

    fn require_known(&self, prefs: &AppPrefs, path: &str) -> Result<PathBuf> {
        let active = prefs.active_project.as_deref() == Some(path);
        let recent = prefs.recent_projects.iter().any(|recent| recent == path);

        if active || recent {
            Ok(PathBuf::from(path))
        } else {
            Err(BusinessError::Forbidden(format!("{path} is not a known project")))
        }
    }
}

fn inspect_all(paths: &[String]) -> Vec<ProjectInfo> {
    paths.iter().map(|path| ProjectInfo::inspect(Path::new(path))).collect()
}

fn utf8(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| BusinessError::InvalidInput("the project path is not valid UTF-8".to_owned()))
}
