//! Opt-in saving pins each repository to the project key the installed server gave its folder when
//! the user turned it on (`repoKeys`), and tells which entries have another key now
//! (`changedRepos`). Run against a stand-in for `harness-memory features` and `project-key`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::memory_cli_support::installed;
use crate::team_support::{exits, texts, wait};

use harness_business::BusinessError;
use harness_business::Result;
use harness_business::ecosystem::{MemoryFeatures, read_global, read_guards, update_guards};
use harness_business::env::Env;
use harness_business::model::{MemoryGuardKind, MemoryGuards, MemoryGuardsPatch};
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ProcessRunner};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// A server with `save-guards` and `save-opt-in` that gives each folder in `keys` the key it maps
/// to (`None`: no project) and refuses the others, or fails every `project-key` while `broken`.
#[derive(Default)]
struct Server {
    keys: Mutex<HashMap<String, Option<String>>>,
    broken: Mutex<bool>,
    asked: Mutex<Vec<Vec<String>>>,
}

impl Server {
    fn key(&self, folder: &Path, key: Option<&str>) {
        self.keys
            .lock()
            .unwrap()
            .insert(folder.to_string_lossy().into_owned(), key.map(str::to_owned));
    }

    fn break_it(&self) {
        *self.broken.lock().unwrap() = true;
    }

    /// The folders of each `project-key` run, in order.
    fn asked(&self) -> Vec<Vec<String>> {
        self.asked.lock().unwrap().clone()
    }
}

impl ProcessRunner for Server {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        let output = match texts(&spec.args).as_slice() {
            [command] if command == "features" => exits(0, "save-guards\nsave-opt-in\n", ""),
            [command, json] if command == "project-key" && json == "--json" => self.project_key(spec),
            _ => exits(2, "", "usage: harness-memory …"),
        };

        Box::pin(async move { Ok(output) })
    }
}

impl Server {
    fn project_key(&self, spec: &ExecSpec) -> ExecOutput {
        let folders: Vec<String> = serde_json::from_slice(spec.stdin.as_deref().unwrap()).unwrap();
        self.asked.lock().unwrap().push(folders.clone());

        if *self.broken.lock().unwrap() {
            return exits(1, "", "harness-memory: the project key could not be read");
        }

        let keys = self.keys.lock().unwrap();
        let answers: Vec<Value> = folders
            .iter()
            .map(|folder| match keys.get(folder) {
                Some(key) => json!({ "path": folder, "root": folder, "key": key }),
                None => json!({ "path": folder, "error": "no such folder" }),
            })
            .collect();

        exits(0, &Value::Array(answers).to_string(), "")
    }
}

/// A home with the installed server, a data folder and repositories `one` and `two`.
struct Lab {
    _dir: tempfile::TempDir,
    env: Env,
    data: PathBuf,
    one: PathBuf,
    two: PathBuf,
    server: Server,
    features: MemoryFeatures,
}

fn lab() -> Lab {
    let (dir, env) = installed();
    let base = dunce::canonicalize(dir.path()).unwrap();
    let make = |name: &str| {
        let repo = base.join(name);
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir_all(repo.join("src")).unwrap();
        dunce::canonicalize(repo).unwrap()
    };
    let (one, two) = (make("one"), make("two"));

    let server = Server::default();
    server.key(&one, Some("git:example.invalid/acme/one"));
    server.key(&one.join("src"), Some("git:example.invalid/acme/one"));
    server.key(&two, Some("git:example.invalid/acme/two"));

    Lab {
        data: base.join("data"),
        _dir: dir,
        env,
        one,
        two,
        server,
        features: MemoryFeatures::new(),
    }
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn patch(kind: MemoryGuardKind) -> MemoryGuardsPatch {
    MemoryGuardsPatch {
        kind,
        paused: None,
        every_project: None,
        add_repo: None,
        remove_repo: None,
    }
}

impl Lab {
    fn add(&self, kind: MemoryGuardKind, folder: &Path) -> Result<MemoryGuards> {
        let patch = MemoryGuardsPatch {
            add_repo: Some(text(folder)),
            ..patch(kind)
        };

        wait(update_guards(
            &self.env,
            &self.server,
            &self.features,
            self.data.clone(),
            patch,
        ))
    }

    fn remove(&self, kind: MemoryGuardKind, folder: &Path) -> MemoryGuards {
        let patch = MemoryGuardsPatch {
            remove_repo: Some(text(folder)),
            ..patch(kind)
        };

        wait(update_guards(
            &self.env,
            &self.server,
            &self.features,
            self.data.clone(),
            patch,
        ))
        .unwrap()
    }

    fn read(&self) -> MemoryGuards {
        wait(read_guards(&self.env, &self.server, &self.features, self.data.clone())).unwrap()
    }

    /// `guards/global.json` as written.
    fn file(&self) -> Value {
        let text = std::fs::read_to_string(self.data.join("guards").join("global.json")).unwrap();

        serde_json::from_str(&text).unwrap()
    }
}

#[test]
fn turning_a_repository_on_pins_the_key_of_the_folder_given_and_not_of_its_saving_root() {
    let lab = lab();

    // The user picked a folder inside the repository: the entry is the repository, the key the
    // server gave the folder picked.
    let guards = lab.add(MemoryGuardKind::Notes, &lab.one.join("src")).unwrap();
    assert_eq!(guards.notes.on_repos, [text(&lab.one)]);
    assert_eq!(
        guards.notes.repo_keys.get(&text(&lab.one)),
        Some(&Some("git:example.invalid/acme/one".to_owned()))
    );
    assert!(guards.notes.dead_repos.is_empty() && guards.notes.changed_repos.is_empty());
    assert!(guards.scripts.repo_keys.is_empty());
    assert_eq!(lab.server.asked()[0], [text(&lab.one.join("src"))]);

    let file = lab.file();
    assert_eq!(
        file["notes"]["repoKeys"],
        json!({ text(&lab.one): "git:example.invalid/acme/one" })
    );
    assert_eq!(file["scripts"]["repoKeys"], json!({}));
}

#[test]
fn a_folder_with_no_project_is_pinned_with_null() {
    let lab = lab();
    let home = lab.one.join("src");
    lab.server.key(&home, None);

    let guards = lab.add(MemoryGuardKind::Scripts, &home).unwrap();

    assert_eq!(guards.scripts.repo_keys.get(&text(&lab.one)), Some(&None));
    assert_eq!(lab.file()["scripts"]["repoKeys"][text(&lab.one)], Value::Null);
}

#[test]
fn a_server_that_cannot_be_asked_or_cannot_use_the_folder_turns_nothing_on_and_writes_nothing() {
    let lab = lab();
    lab.add(MemoryGuardKind::Notes, &lab.one).unwrap();
    let before = std::fs::read(lab.data.join("guards").join("global.json")).unwrap();

    // A folder the server cannot use: the reason reaches the user.
    let gone = lab.two.join("gone");
    let error = lab.add(MemoryGuardKind::Notes, &gone).unwrap_err();
    assert!(
        matches!(&error, BusinessError::InvalidInput(message) if message.contains("no such folder")),
        "{error:?}"
    );

    // A server that fails.
    lab.server.break_it();
    let error = lab.add(MemoryGuardKind::Notes, &lab.two).unwrap_err();
    assert!(
        matches!(&error, BusinessError::Process(message) if message.contains("could not be read")),
        "{error:?}"
    );

    assert_eq!(
        std::fs::read(lab.data.join("guards").join("global.json")).unwrap(),
        before,
        "nothing was written"
    );
}

#[test]
fn turning_a_listed_repository_on_again_replaces_its_pin_and_turning_it_off_drops_it() {
    let lab = lab();
    lab.add(MemoryGuardKind::Notes, &lab.one).unwrap();
    lab.add(MemoryGuardKind::Notes, &lab.two).unwrap();

    // Its remote changed: the server gives the folder another key, and turning it on again pins it.
    lab.server.key(&lab.one, Some("git:example.invalid/acme/moved"));
    let again = lab.add(MemoryGuardKind::Notes, &lab.one).unwrap();
    assert_eq!(again.notes.on_repos.len(), 2, "listed once");
    assert_eq!(
        again.notes.repo_keys.get(&text(&lab.one)),
        Some(&Some("git:example.invalid/acme/moved".to_owned()))
    );
    assert_eq!(
        again.notes.repo_keys.get(&text(&lab.two)),
        Some(&Some("git:example.invalid/acme/two".to_owned())),
        "the other pin stays"
    );

    // Turning it off drops its pin, and only its pin.
    let off = lab.remove(MemoryGuardKind::Notes, &lab.one);
    assert_eq!(off.notes.on_repos, [text(&lab.two)]);
    assert_eq!(
        off.notes.repo_keys.keys().cloned().collect::<Vec<_>>(),
        [text(&lab.two)]
    );
    assert_eq!(
        lab.file()["notes"]["repoKeys"],
        json!({ text(&lab.two): "git:example.invalid/acme/two" })
    );
}

#[test]
fn changed_repos_names_the_entries_whose_key_differs_and_is_never_written() {
    let lab = lab();
    lab.add(MemoryGuardKind::Notes, &lab.one).unwrap();
    lab.add(MemoryGuardKind::Notes, &lab.two).unwrap();
    lab.add(MemoryGuardKind::Scripts, &lab.one).unwrap();
    let asked_before = lab.server.asked().len();

    // Nothing changed: nothing named.
    let read = lab.read();
    assert!(read.notes.changed_repos.is_empty() && read.scripts.changed_repos.is_empty());

    // `one` got another remote.
    lab.server.key(&lab.one, Some("git:example.invalid/acme/off"));
    let read = lab.read();
    assert_eq!(read.notes.changed_repos, [text(&lab.one)]);
    assert_eq!(read.scripts.changed_repos, [text(&lab.one)]);
    assert!(read.notes.dead_repos.is_empty(), "a changed entry is not a dead one");

    // One question for the entries of both kinds, and a change returns the same.
    let asked = lab.server.asked();
    assert_eq!(asked.len(), asked_before + 2, "one question per read");
    assert_eq!(asked.last().unwrap(), &[text(&lab.one), text(&lab.two)]);
    let paused = wait(update_guards(
        &lab.env,
        &lab.server,
        &lab.features,
        lab.data.clone(),
        MemoryGuardsPatch {
            paused: Some(true),
            ..patch(MemoryGuardKind::Notes)
        },
    ))
    .unwrap();
    assert_eq!(paused.notes.changed_repos, [text(&lab.one)]);

    // Neither it nor the dead entries reach the file, and reading it back has none.
    let file = lab.file();
    for kind in ["notes", "scripts"] {
        assert!(file[kind].get("changedRepos").is_none() && file[kind].get("deadRepos").is_none());
    }
    assert!(read_global(&lab.data).unwrap().notes.changed_repos.is_empty());

    // Turning it on again accepts the new key.
    let accepted = lab.add(MemoryGuardKind::Notes, &lab.one).unwrap();
    assert!(accepted.notes.changed_repos.is_empty());
    assert_eq!(
        lab.read().scripts.changed_repos,
        [text(&lab.one)],
        "the scripts entry is its own"
    );
}

#[test]
fn a_key_made_from_a_path_is_not_a_change_and_a_server_that_fails_names_nothing() {
    let lab = lab();
    lab.server.key(&lab.two, Some(&format!("path:{}", lab.two.display())));
    lab.add(MemoryGuardKind::Notes, &lab.two).unwrap();

    // A path key stays a path key whatever the path (a worktree of a repository with no remote).
    lab.server.key(&lab.two, Some("path:/somewhere/else"));
    assert!(lab.read().notes.changed_repos.is_empty());

    // A remote appeared: another key.
    lab.server.key(&lab.two, Some("git:example.invalid/acme/two"));
    assert_eq!(lab.read().notes.changed_repos, [text(&lab.two)]);

    // The server cannot be asked: the guards still read, and nothing is named.
    lab.server.break_it();
    let read = lab.read();
    assert_eq!(read.notes.on_repos, [text(&lab.two)]);
    assert!(read.notes.changed_repos.is_empty());
}
