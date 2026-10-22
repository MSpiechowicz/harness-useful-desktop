//! Projects: inspection, opening (validated), the recent list, forgetting, and authorization of
//! project paths sent by the webview.

use std::fs;
use std::path::{Path, PathBuf};

use harness_business::env::Env;
use harness_business::error::BusinessError;
use harness_business::model::{AppPrefs, ProjectInfo};
use harness_business::prefs::{MAX_RECENT_PROJECTS, PrefsStore};
use harness_business::project::Projects;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

/// A temporary home with a prefs store and project directories.
struct World {
    /// Keeps the directory alive for the test.
    _home: TempDir,
    store: PrefsStore,
    env: Env,
}

impl World {
    fn new() -> Self {
        let home = TempDir::new().unwrap();
        let root = dunce::canonicalize(home.path()).unwrap();
        let store = PrefsStore::new(root.join("config"));
        let env = Env::for_tests(&root);

        Self {
            _home: home,
            store,
            env,
        }
    }

    fn projects(&self) -> Projects<'_> {
        Projects::new(&self.store, &self.env)
    }

    /// Creates `<home>/<name>` and returns its canonical path.
    fn dir(&self, name: &str) -> PathBuf {
        let path = self.env.home.join(name);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn prefs(&self) -> AppPrefs {
        self.store.load()
    }
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[test]
fn inspect_reports_name_existence_and_git_marker() {
    let world = World::new();
    let plain = world.dir("plain");
    let repo = world.dir("repo");
    fs::create_dir(repo.join(".git")).unwrap();
    let worktree = world.dir("worktree");
    fs::write(worktree.join(".git"), "gitdir: /elsewhere\n").unwrap();
    let gone = world.env.home.join("gone");

    assert_eq!(
        ProjectInfo::inspect(&plain),
        ProjectInfo {
            path: text(&plain),
            name: "plain".to_owned(),
            exists: true,
            is_git_repo: false,
        }
    );
    assert!(ProjectInfo::inspect(&repo).is_git_repo);
    assert!(ProjectInfo::inspect(&worktree).is_git_repo);

    let missing = ProjectInfo::inspect(&gone);
    assert_eq!(
        (missing.name.as_str(), missing.exists, missing.is_git_repo),
        ("gone", false, false)
    );
}

#[test]
fn open_sets_the_active_project_and_moves_it_to_the_front_of_recents() {
    let world = World::new();
    let first = world.dir("first");
    let second = world.dir("second");

    world.projects().open(&first).unwrap();
    world.projects().open(&second).unwrap();
    let reopened = world.projects().open(&first).unwrap();

    assert_eq!(reopened.path, text(&first));
    assert_eq!(world.prefs().active_project, Some(text(&first)));
    assert_eq!(world.prefs().recent_projects, [text(&first), text(&second)]);

    let recent: Vec<String> = world
        .projects()
        .recent()
        .into_iter()
        .map(|project| project.name)
        .collect();
    assert_eq!(recent, ["first", "second"]);
}

#[test]
fn open_stores_the_canonical_path() {
    let world = World::new();
    let project = world.dir("project");
    world.dir("other");

    let opened = world.projects().open(&world.env.home.join("other/../project")).unwrap();

    assert_eq!(opened.path, text(&project));
    assert_eq!(world.prefs().active_project, Some(text(&project)));
}

#[test]
fn open_validates_the_directory() {
    let world = World::new();
    let file = world.env.home.join("file.txt");
    fs::write(&file, "x").unwrap();

    let missing = world.projects().open(&world.env.home.join("missing"));
    let not_a_dir = world.projects().open(&file);
    let relative = world.projects().open(Path::new("relative/dir"));

    assert!(matches!(missing, Err(BusinessError::NotFound(_))), "{missing:?}");
    assert!(
        matches!(not_a_dir, Err(BusinessError::InvalidInput(_))),
        "{not_a_dir:?}"
    );
    assert!(matches!(relative, Err(BusinessError::InvalidInput(_))), "{relative:?}");
    assert_eq!(world.prefs(), AppPrefs::default());
}

#[test]
fn open_without_remembering_keeps_recents_empty() {
    let world = World::new();
    let project = world.dir("project");
    world
        .store
        .save(&AppPrefs {
            remember_recent_projects: false,
            ..AppPrefs::default()
        })
        .unwrap();

    world.projects().open(&project).unwrap();

    assert_eq!(world.prefs().active_project, Some(text(&project)));
    assert!(world.prefs().recent_projects.is_empty());
}

#[test]
fn recents_are_capped() {
    let world = World::new();

    for index in 0..MAX_RECENT_PROJECTS + 2 {
        world.projects().open(&world.dir(&format!("p{index}"))).unwrap();
    }

    let recents = world.prefs().recent_projects;
    assert_eq!(recents.len(), MAX_RECENT_PROJECTS);
    assert_eq!(
        recents[0],
        text(&world.env.home.join(format!("p{}", MAX_RECENT_PROJECTS + 1)))
    );
}

#[test]
fn reopen_accepts_only_known_projects() {
    let world = World::new();
    let known = world.dir("known");
    let stranger = world.dir("stranger");
    world.projects().open(&known).unwrap();
    world.projects().close().unwrap();

    let reopened = world.projects().reopen(&text(&known)).unwrap();
    let refused = world.projects().reopen(&text(&stranger));

    assert_eq!(reopened.path, text(&known));
    assert_eq!(world.prefs().active_project, Some(text(&known)));
    assert!(matches!(refused, Err(BusinessError::Forbidden(_))), "{refused:?}");
}

#[test]
fn reopen_of_a_deleted_recent_project_fails_without_changing_prefs() {
    let world = World::new();
    let doomed = world.dir("doomed");
    world.projects().open(&doomed).unwrap();
    world.projects().close().unwrap();
    fs::remove_dir(&doomed).unwrap();
    let before = world.prefs();

    let result = world.projects().reopen(&text(&doomed));

    assert!(matches!(result, Err(BusinessError::NotFound(_))), "{result:?}");
    assert_eq!(world.prefs(), before);
    assert!(!world.projects().recent()[0].exists);
}

#[test]
fn close_clears_only_the_active_project() {
    let world = World::new();
    let project = world.dir("project");
    world.projects().open(&project).unwrap();

    world.projects().close().unwrap();

    assert_eq!(world.prefs().active_project, None);
    assert_eq!(world.projects().active(), None);
    assert_eq!(world.prefs().recent_projects, [text(&project)]);
}

#[test]
fn forget_removes_the_project_and_clears_it_when_active() {
    let world = World::new();
    let kept = world.dir("kept");
    let forgotten = world.dir("forgotten");
    world.projects().open(&kept).unwrap();
    world.projects().open(&forgotten).unwrap();

    let remaining = world.projects().forget(&text(&forgotten)).unwrap();

    assert_eq!(remaining, [ProjectInfo::inspect(&kept)]);
    assert_eq!(world.prefs().active_project, None);
    assert_eq!(world.prefs().recent_projects, [text(&kept)]);
}

#[test]
fn forget_keeps_a_different_active_project() {
    let world = World::new();
    let active = world.dir("active");
    let other = world.dir("other");
    world.projects().open(&other).unwrap();
    world.projects().open(&active).unwrap();

    world.projects().forget(&text(&other)).unwrap();

    assert_eq!(world.projects().active(), Some(active));
}

#[test]
fn authorize_accepts_none_and_known_projects_only() {
    let world = World::new();
    let active = world.dir("active");
    let recent = world.dir("recent");
    let stranger = world.dir("stranger");
    world.projects().open(&recent).unwrap();
    world.projects().open(&active).unwrap();

    assert_eq!(world.projects().authorize(None).unwrap(), None);
    assert_eq!(
        world.projects().authorize(Some(&text(&active))).unwrap(),
        Some(active.clone())
    );
    assert_eq!(world.projects().authorize(Some(&text(&recent))).unwrap(), Some(recent));

    for refused in [
        text(&stranger),
        text(&world.env.home),
        format!("{}/../stranger", text(&active)),
    ] {
        let result = world.projects().authorize(Some(&refused));
        assert!(
            matches!(result, Err(BusinessError::Forbidden(_))),
            "{refused}: {result:?}"
        );
    }
}

#[test]
fn authorize_still_validates_known_projects() {
    let world = World::new();
    let project = world.dir("project");
    world.projects().open(&project).unwrap();
    fs::remove_dir(&project).unwrap();

    let result = world.projects().authorize(Some(&text(&project)));

    assert!(matches!(result, Err(BusinessError::NotFound(_))), "{result:?}");
}
