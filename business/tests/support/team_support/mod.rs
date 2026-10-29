//! Team hooks for the tests (`hooks_team.rs`, `hooks_team_actions.rs`, `memory_hooks.rs`): team
//! bodies and rows as `harness-memory hooks` answers them, a store with its team folder, and a
//! runner that answers `harness-memory` commands from rules.
//! Each test binary uses only some of these helpers.
#![allow(dead_code)]

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Mutex;

use harness_business::Result;
use harness_business::hooks::sha256_hex;
use harness_business::hooks::team::{ProjectKey, TeamFolder, TeamRow};
use harness_business::hooks::{HookCapabilities, HookStore};
use harness_business::model::{HookAction, HookAuthor, HookDraft, HookEvent, HookScope, HookView};
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ProcessRunner};
use serde_json::{Value, json};
use tempfile::TempDir;

pub const NONE: HookCapabilities = HookCapabilities {
    memory_available: false,
    dashboard_available: false,
    code_graph_available: false,
};

/// The repository key of the tests' project.
pub const REPO: &str = "git:github.com/o/r";

/// A hook id made of `n`.
pub fn id(n: u8) -> String {
    format!("{n:032x}")
}

/// A hooks root with its team folder, and a project root.
pub struct Team {
    pub temp: TempDir,
    pub store: HookStore,
    pub folder: TeamFolder,
    pub project: String,
}

pub fn team() -> Team {
    let temp = tempfile::tempdir().unwrap();
    let store = HookStore::new(temp.path().join("harness-hooks"), temp.path().join("config"));
    let folder = TeamFolder::of(&store);

    let project = temp.path().join("demo");
    std::fs::create_dir_all(&project).unwrap();
    let project = dunce::canonicalize(project).unwrap().to_string_lossy().into_owned();

    Team {
        temp,
        store,
        folder,
        project,
    }
}

impl Team {
    /// Writes `rows` as the team copies.
    pub fn copies(&self, rows: &[TeamRow]) {
        for row in rows {
            self.folder.write_row(row).unwrap();
        }
    }

    /// Records that the project root has the repository key [`REPO`].
    pub fn map_project(&self) {
        let mut projects = std::collections::BTreeMap::new();
        projects.insert(
            self.project.clone(),
            ProjectKey {
                key: REPO.to_owned(),
                shareable: true,
            },
        );

        self.folder.save_projects(&projects).unwrap();
    }
}

/// The hook object of team hook `id`: global, or of the repository `repo`.
pub fn hook_json(id: &str, name: &str, repo: Option<&str>) -> Value {
    json!({
        "version": 1,
        "id": id,
        "name": name,
        "description": null,
        "scope": if repo.is_some() { "project" } else { "global" },
        "project": repo,
        "event": "promptSubmit",
        "matcher": null,
        "action": {"type": "addContext", "text": format!("{name} text")},
        "enabled": true,
        "createdAt": "2026-10-01T10:00:00Z",
        "createdBy": "desktop",
        "updatedAt": "2026-10-01T10:00:00Z",
        "updatedBy": "desktop"
    })
}

/// `value` as the apps write a hook file.
pub fn bytes_of(value: &Value) -> String {
    format!("{}\n", serde_json::to_string_pretty(value).unwrap())
}

/// The row of `body` for hook `id` (global, or of `repo`), shared by "ana".
pub fn row_of(id: &str, repo: Option<&str>, body: String) -> TeamRow {
    TeamRow {
        id: id.to_owned(),
        repo: repo.map(str::to_owned),
        sha256: sha256_hex(body.as_bytes()),
        author: "ana".to_owned(),
        created_at: "2026-10-01T10:00:00Z".to_owned(),
        updated_at: "2026-10-02T08:30:00Z".to_owned(),
        body,
    }
}

/// A valid team hook row: global, or of `repo`.
pub fn row(id: &str, name: &str, repo: Option<&str>) -> TeamRow {
    row_of(id, repo, bytes_of(&hook_json(id, name, repo)))
}

/// `row` as JSON, as `hooks list` prints it.
pub fn row_json(row: &TeamRow) -> Value {
    serde_json::to_value(row).unwrap()
}

/// A rule: the answer to a command's arguments, `None` when it does not take them.
type Rule = Box<dyn Fn(&[String]) -> Option<ExecOutput> + Send + Sync>;

/// A `harness-memory` stand-in: the first rule that takes the arguments answers.
#[derive(Default)]
pub struct Cli {
    rules: Mutex<Vec<Rule>>,
    pub runs: Mutex<Vec<ExecSpec>>,
}

impl Cli {
    /// Answers `output` to the commands `test` takes.
    pub fn on(self, test: impl Fn(&[String]) -> bool + Send + Sync + 'static, output: ExecOutput) -> Self {
        self.answering(move |args| test(args).then(|| output.clone()))
    }

    /// Answers what `answer` gives for a command's arguments.
    pub fn answering(self, answer: impl Fn(&[String]) -> Option<ExecOutput> + Send + Sync + 'static) -> Self {
        self.rules.lock().unwrap().push(Box::new(answer));
        self
    }

    /// Answers `output` to commands starting with `prefix`.
    pub fn starting(self, prefix: &[&str], output: ExecOutput) -> Self {
        let prefix: Vec<String> = prefix.iter().map(|part| (*part).to_owned()).collect();

        self.on(move |args| args.starts_with(&prefix), output)
    }

    /// The arguments of every run, in order.
    pub fn commands(&self) -> Vec<Vec<String>> {
        self.runs.lock().unwrap().iter().map(|spec| texts(&spec.args)).collect()
    }
}

impl ProcessRunner for Cli {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        self.runs.lock().unwrap().push(spec.clone());

        let args = texts(&spec.args);
        let output = self
            .rules
            .lock()
            .unwrap()
            .iter()
            .find_map(|rule| rule(&args))
            .unwrap_or_else(|| exits(2, "", "usage: harness-memory …"));

        Box::pin(async move { Ok(output) })
    }
}

/// True when `args` hold `--project <project>`.
pub fn with_project(args: &[String], project: &str) -> bool {
    args.windows(2).any(|pair| pair[0] == "--project" && pair[1] == project)
}

pub fn texts(args: &[OsString]) -> Vec<String> {
    args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect()
}

/// A run that printed `value` and exited 0.
pub fn answers(value: Value) -> ExecOutput {
    ExecOutput {
        status: Some(0),
        stdout: value.to_string(),
        ..ExecOutput::default()
    }
}

/// A run that exited `code` with `stdout` and `stderr`.
pub fn exits(code: i32, stdout: &str, stderr: &str) -> ExecOutput {
    ExecOutput {
        status: Some(code),
        stdout: stdout.to_owned(),
        stderr: stderr.to_owned(),
        ..ExecOutput::default()
    }
}

/// `hooks list` of `rows`, with `project` as the identity asked for.
pub fn listed(rows: &[TeamRow], project: Option<(&str, bool)>) -> ExecOutput {
    answers(json!({
        "total": rows.len(),
        "project": project.map(|(key, shareable)| json!({"key": key, "shareable": shareable})),
        "hooks": rows.iter().map(row_json).collect::<Vec<_>>(),
    }))
}

/// Runs `future` to completion.
pub fn wait<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Runtime::new().unwrap().block_on(future)
}

/// The path of the server the tests name (never run: the [`Cli`] answers).
pub fn server() -> PathBuf {
    PathBuf::from(if cfg!(windows) {
        "C:\\bin\\harness-memory.exe"
    } else {
        "/bin/harness-memory"
    })
}

/// The value after `option` in `args`, as a number.
pub fn number_after(args: &[String], option: &str) -> Option<usize> {
    let index = args.iter().position(|arg| arg == option)?;

    args.get(index + 1)?.parse().ok()
}

/// A global hook `name` saved in the app window (approved).
pub fn local(store: &HookStore, name: &str) -> HookView {
    let draft = HookDraft {
        id: None,
        expected_sha256: None,
        name: name.to_owned(),
        description: None,
        scope: HookScope::Global,
        project: None,
        event: HookEvent::PromptSubmit,
        matcher: None,
        action: HookAction::AddContext {
            text: "Local text".to_owned(),
        },
        enabled: true,
    };

    store
        .save(&draft, HookAuthor::Desktop, NONE, std::time::UNIX_EPOCH)
        .unwrap()
}
