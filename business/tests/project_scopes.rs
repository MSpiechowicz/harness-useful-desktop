//! The projects the views scope their tables by (`project_scopes`): the active project, then the
//! recent ones, each with its project root and its Harness Memory repository key, asked once per
//! session (until the project is opened or forgotten again) and `None` whenever the server cannot
//! give it.

mod common;
mod memory_cli_support;
mod team_support;

use std::path::Path;

use harness_business::env::Env;
use harness_business::model::ProjectScope;
use harness_business::prefs::PrefsStore;
use harness_business::project::{ProjectKeys, Projects, with_keys};
use memory_cli_support::installed;
use pretty_assertions::assert_eq;
use serde_json::json;
use team_support::{Cli, answers, wait};

/// A home with a prefs store, where `harness-memory` may be installed.
struct World {
    _home: tempfile::TempDir,
    store: PrefsStore,
    env: Env,
}

impl World {
    fn new(with_memory: bool) -> Self {
        let (home, env) = if with_memory {
            installed()
        } else {
            let home = tempfile::tempdir().unwrap();
            let env = Env::for_tests(home.path());
            (home, env)
        };

        Self {
            store: PrefsStore::new(home.path().join("config")),
            _home: home,
            env,
        }
    }

    fn projects(&self) -> Projects<'_> {
        Projects::new(&self.store, &self.env)
    }

    /// Opens `<home>/<name>` (a git repository when `git`) and returns its stored path.
    fn open(&self, name: &str, git: bool) -> String {
        let dir = self.env.home.join(name);
        std::fs::create_dir_all(if git { dir.join(".git") } else { dir.clone() }).unwrap();

        self.projects().open(&dir).unwrap().path
    }

    fn scopes(&self, keys: &ProjectKeys, cli: &Cli) -> Vec<ProjectScope> {
        wait(with_keys(&self.env, cli, keys, self.projects().scopes()))
    }
}

/// Answers `hooks list --project <root>` with the key `git:host/<folder name>`.
fn keys_by_folder() -> Cli {
    Cli::default().answering(|args| {
        let index = args.iter().position(|arg| arg == "--project")?;
        let name = Path::new(args.get(index + 1)?)
            .file_name()?
            .to_string_lossy()
            .into_owned();

        args.starts_with(&["hooks".to_owned(), "list".to_owned()]).then(|| {
            answers(json!({
                "total": 0,
                "nextOffset": null,
                "project": {"key": format!("git:host/{name}"), "shareable": true},
                "hooks": []
            }))
        })
    })
}

fn keys(scopes: &[ProjectScope]) -> Vec<(&str, Option<&str>)> {
    scopes
        .iter()
        .map(|scope| (scope.name.as_str(), scope.key.as_deref()))
        .collect()
}

#[test]
fn the_active_project_comes_first_then_the_recent_ones_with_their_roots() {
    let world = World::new(false);
    let plain = world.open("plain", false);
    let repo = world.open("repo", true);
    let nested = world.env.home.join("repo").join("src");
    std::fs::create_dir_all(&nested).unwrap();
    let nested = world.projects().open(&nested).unwrap().path;
    world.open("gone", false);
    std::fs::remove_dir(world.env.home.join("gone")).unwrap();
    world.projects().reopen(&plain).unwrap();

    let cli = Cli::default();
    let scopes = world.scopes(&ProjectKeys::default(), &cli);

    let listed: Vec<(&str, Option<&str>)> = scopes
        .iter()
        .map(|scope| (scope.name.as_str(), scope.root.as_deref()))
        .collect();
    assert_eq!(
        listed,
        [
            ("plain", Some(plain.as_str())),
            ("gone", None),
            ("src", Some(repo.as_str())),
            ("repo", Some(repo.as_str())),
        ]
    );
    assert_eq!(scopes[2].path, nested);

    // No Harness Memory: no key, and nothing is run.
    assert!(scopes.iter().all(|scope| scope.key.is_none()));
    assert!(cli.commands().is_empty());
}

#[test]
fn keys_are_asked_once_per_session_until_the_project_is_opened_or_forgotten_again() {
    let world = World::new(true);
    let web = world.open("web", true);
    world.open("api", true);

    let cli = keys_by_folder();
    let kept = ProjectKeys::default();

    let scopes = world.scopes(&kept, &cli);
    assert_eq!(
        keys(&scopes),
        [("api", Some("git:host/api")), ("web", Some("git:host/web"))]
    );
    assert_eq!(cli.commands().len(), 2);

    world.scopes(&kept, &cli);
    assert_eq!(cli.commands().len(), 2, "kept for the session");

    kept.forget(&web);
    let again = world.scopes(&kept, &cli);
    assert_eq!(keys(&again), keys(&scopes));
    let asked = cli.commands();
    assert_eq!(asked.len(), 3);
    assert!(asked[2].windows(2).any(|pair| pair == ["--project", web.as_str()]));
}

#[test]
fn a_server_that_gives_no_key_leaves_it_null_without_failing_the_list() {
    let world = World::new(true);
    world.open("web", true);

    // The stand-in answers a usage error: a server that predates `hooks list`.
    let old = Cli::default();
    let kept = ProjectKeys::default();

    let scopes = world.scopes(&kept, &old);
    assert_eq!(keys(&scopes), [("web", None)]);
    assert_eq!(old.commands().len(), 1);

    // Not asked again right away, even of a server that would answer.
    let current = keys_by_folder();
    assert_eq!(keys(&world.scopes(&kept, &current)), [("web", None)]);
    assert!(current.commands().is_empty());

    // A key that is not a repository key is no key.
    let odd = Cli::default().answering(|args| {
        args.starts_with(&["hooks".to_owned()])
            .then(|| answers(json!({"project": {"key": "bad\nkey", "shareable": false}, "hooks": []})))
    });
    assert_eq!(keys(&world.scopes(&ProjectKeys::default(), &odd)), [("web", None)]);
}
