//! Team routines in the app window: a `harness-memory` stand-in (a copy of `fake_agent`, see its
//! `memory_hooks` docs) shares routines, a refresh writes the copies, and the runtime schedules
//! and runs the ones turned on here with real shell commands.

use std::path::PathBuf;

use harness_business::ecosystem::{MemoryRemoteChange, memory_config_path};
use harness_business::model::{
    HookAuthor, HookOrigin, IpcErrorCode, MemoryNotesRouting, MemoryRemoteKind, RoutineDraft, RoutineFilter,
    RoutineStatus, RoutineView, RoutineWhen, RunStatus,
};
use harness_business::routines::sha256_hex;
use harness_business::routines::team::{TeamFolder, TeamRow};
use harness_business::routines::view::on_schedule;
use harness_business::signed_docs::project_identity;
use serde_json::{Value, json};
use tauri::async_runtime::block_on;

use super::tests::{command, draft, ended, save};
use crate::commands::memory_remote;
use crate::commands::routines::{self as commands, team as team_commands};
use crate::error::CommandError;
use crate::test_support::{ProjectHome, place_memory_server};

/// The ids of the team routines the stand-in lists.
pub(super) const TEAM_ID: &str = "7eaa0000000000000000000000000001";
const OTHER_ID: &str = "7eaa0000000000000000000000000002";

/// The repository key the stand-in gives every project.
const REPO_KEY: &str = "git:github.com/o/r";

/// An app state with an open project and an installed `harness-memory`; `None` (the test skips)
/// when `fake_agent` is not built.
pub(super) struct Team {
    pub(super) setup: ProjectHome,
    server: PathBuf,
}

pub(super) fn team(name: &str) -> Option<Team> {
    let setup = ProjectHome::new(name);
    let server = place_memory_server(&setup.home)?;

    Some(Team { setup, server })
}

impl Team {
    pub(super) fn state(&self) -> &crate::state::AppState {
        &self.setup.state
    }

    /// What the stand-in's `routines config` answers; `None` makes it an older server. The config
    /// file changes with it, as a save in Settings changes it, so the answer is asked again.
    pub(super) fn sharing(&self, config: Option<(bool, bool)>) {
        let answer = self.beside("routines-config.json");
        let file = memory_config_path(&self.state().env).expect("a config path");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();

        match config {
            Some((remote, trust)) => {
                let routines = if remote { "remote" } else { "local" };
                let text = json!({"routines": routines, "trustTeamRoutines": trust}).to_string();
                std::fs::write(&answer, text).unwrap();
                std::fs::write(
                    &file,
                    format!("routines = \"{routines}\"\ntrust_team_routines = {trust}\n"),
                )
                .unwrap();
            }
            None => {
                std::fs::remove_file(&answer).ok();
                std::fs::write(&file, "").unwrap();
            }
        }
    }

    fn beside(&self, name: &str) -> PathBuf {
        self.server.parent().unwrap().join(name)
    }

    /// The team's routines, as the stand-in keeps them.
    pub(super) fn rows(&self, rows: &[Value]) {
        std::fs::write(
            self.beside("routines-rows.json"),
            Value::from(rows.to_vec()).to_string(),
        )
        .unwrap();
    }

    fn stored_rows(&self) -> Vec<Value> {
        let text = std::fs::read_to_string(self.beside("routines-rows.json")).unwrap_or_default();

        serde_json::from_str(&text).unwrap_or_default()
    }

    pub(super) fn refresh(&self) {
        block_on(crate::team_sync::refresh(&*self.state().routines));
    }

    fn project(&self) -> String {
        project_identity(&self.setup.project)
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }

    fn listed(&self) -> Vec<RoutineView> {
        let project = Some(self.project());

        block_on(commands::list(self.state(), project, RoutineFilter::default()))
            .expect("listed")
            .routines
    }

    fn team_view(&self, id: &str) -> RoutineView {
        self.listed()
            .into_iter()
            .find(|view| view.origin == HookOrigin::Team && view.id == id)
            .expect("the team routine is listed")
    }

    pub(super) fn enable(&self, id: &str, enabled: bool, project: Option<String>) -> Result<RoutineView, CommandError> {
        block_on(team_commands::enable(self.state(), id.to_owned(), enabled, project))
    }

    fn approve(&self, view: &RoutineView) -> RoutineView {
        block_on(commands::approve(self.state(), view.id.clone(), view.sha256.clone())).expect("approved")
    }

    /// The ids the scheduler plans, with their origin.
    fn scheduled(&self) -> Vec<(String, HookOrigin)> {
        block_on(self.state().routines.schedule_views())
            .expect("read")
            .expect("a complete listing")
            .into_iter()
            .filter(on_schedule)
            .map(|view| (view.id, view.origin))
            .collect()
    }

    /// Runs routine `id` now and returns how its newest run ended.
    pub(super) fn run(&self, id: &str, count: usize) -> harness_business::model::RoutineRun {
        block_on(self.state().routines.run_now(id))
            .expect("ran")
            .expect("a free place");

        ended(self.state(), id, count).remove(0)
    }
}

/// A team routine row shared by "ana": `id`, global or of `repo`, running `action`.
pub(super) fn row(id: &str, repo: Option<&str>, action: Value) -> Value {
    let body = format!(
        "{}\n",
        serde_json::to_string_pretty(&json!({
            "version": 1,
            "id": id,
            "name": "Team check",
            "description": null,
            "scope": if repo.is_some() { "project" } else { "global" },
            "project": repo,
            "when": {"type": "daily", "at": "07:00"},
            "action": action,
            "enabled": true,
            "createdAt": "2026-10-01T10:00:00Z",
            "createdBy": "desktop",
            "updatedAt": "2026-10-01T10:00:00Z",
            "updatedBy": "desktop"
        }))
        .unwrap()
    );

    json!({
        "id": id,
        "repo": repo,
        "sha256": sha256_hex(body.as_bytes()),
        "author": "ana",
        "createdAt": "2026-10-01T10:00:00Z",
        "updatedAt": "2026-10-01T10:00:00Z",
        "body": body,
    })
}

pub(super) fn echo(text: &str) -> Value {
    json!({"type": "command", "command": format!("echo {text}"), "timeoutSecs": null})
}

fn yolo() -> Value {
    json!({"type": "agent", "harness": "omp", "model": null, "prompt": "Tidy up.", "approvalMode": "yolo",
        "trustOnce": false, "maxMinutes": null})
}

fn code(result: Result<impl std::fmt::Debug, CommandError>) -> IpcErrorCode {
    result.expect_err("refused").code()
}

#[test]
fn a_team_routine_arrives_off_and_runs_once_turned_on_and_approved() {
    let Some(team) = team("team-routines-off") else {
        return;
    };
    let runtime = &team.state().routines;

    team.sharing(Some((true, false)));
    team.rows(&[row(TEAM_ID, None, echo("team"))]);
    team.refresh();

    let off = team.team_view(TEAM_ID);
    assert_eq!((off.status, off.team_on), (RoutineStatus::Off, false));
    assert!(team.scheduled().is_empty());
    let refused = block_on(runtime.run_now(TEAM_ID)).map_err(CommandError::from);
    assert_eq!(code(refused), IpcErrorCode::InvalidInput);

    // Turning on is not approval: without trust it waits.
    let pending = team.enable(TEAM_ID, true, None).expect("turned on");
    assert_eq!((pending.status, pending.team_on), (RoutineStatus::Pending, true));
    assert!(team.scheduled().is_empty());

    let active = team.approve(&pending);
    assert_eq!(
        (active.status, active.origin),
        (RoutineStatus::Active, HookOrigin::Team)
    );
    assert_eq!(team.scheduled(), [(TEAM_ID.to_owned(), HookOrigin::Team)]);

    let run = team.run(TEAM_ID, 1);
    assert_eq!(run.record.status, RunStatus::Ok);
    assert_eq!(run.record.origin, HookOrigin::Team);
    assert_eq!(
        (run.record.project, run.record.excerpt.as_deref()),
        (None, Some("team"))
    );

    let off = team.enable(TEAM_ID, false, None).expect("turned off");
    assert_eq!(off.status, RoutineStatus::Off);
    assert!(team.scheduled().is_empty());
}

#[test]
fn trust_runs_every_team_routine_even_one_that_runs_tools_without_asking() {
    let Some(team) = team("team-routines-trust") else {
        return;
    };

    team.sharing(Some((true, true)));
    team.rows(&[row(TEAM_ID, None, echo("trusted")), row(OTHER_ID, None, yolo())]);
    team.refresh();

    let trusted = team.enable(TEAM_ID, true, None).expect("turned on");
    assert_eq!(trusted.status, RoutineStatus::Active);

    let unattended = team.enable(OTHER_ID, true, None).expect("turned on");
    assert_eq!(unattended.status, RoutineStatus::Active, "trust covers yolo too");
    assert!(unattended.runs_unattended);
    assert_eq!(team.scheduled().len(), 2);

    // Without trust each one waits until its exact bytes are approved here.
    team.sharing(Some((true, false)));
    team.refresh();
    assert_eq!(team.team_view(OTHER_ID).status, RoutineStatus::Pending);
    assert!(team.scheduled().is_empty());

    assert_eq!(team.approve(&team.team_view(OTHER_ID)).status, RoutineStatus::Active);
    assert_eq!(team.scheduled(), [(OTHER_ID.to_owned(), HookOrigin::Team)]);
}

#[test]
fn a_trusted_team_routine_runs_only_while_the_team_database_holds_its_bytes() {
    let Some(team) = team("team-routines-confirm") else {
        return;
    };

    team.sharing(Some((true, true)));
    team.rows(&[row(TEAM_ID, None, echo("trusted"))]);
    team.refresh();
    team.enable(TEAM_ID, true, None).expect("turned on");
    assert_eq!(team.run(TEAM_ID, 1).record.status, RunStatus::Ok);

    // The team copy changed in the database and this machine has not refreshed yet.
    team.rows(&[row(TEAM_ID, None, echo("changed"))]);
    let stale = team.run(TEAM_ID, 2).record;
    assert_eq!(stale.status, RunStatus::Failed);
    assert!(stale.message.unwrap_or_default().contains("changed"));
}

#[test]
fn a_team_project_routine_runs_in_the_folder_chosen_for_its_repository() {
    let Some(team) = team("team-routines-project") else {
        return;
    };
    let root = team.project();

    team.sharing(Some((true, true)));
    team.rows(&[row(TEAM_ID, Some(REPO_KEY), echo("here"))]);
    team.state().routines.team().note_root(&team.state().env, &root);
    team.refresh();

    assert_eq!(code(team.enable(TEAM_ID, true, None)), IpcErrorCode::InvalidInput);
    let stranger = team.setup.stranger.to_string_lossy().into_owned();
    assert_eq!(
        code(team.enable(TEAM_ID, true, Some(stranger))),
        IpcErrorCode::Forbidden
    );

    let on = team.enable(TEAM_ID, true, Some(root.clone())).expect("turned on");
    assert_eq!(on.status, RoutineStatus::Active);
    assert_eq!(on.team_folder.as_deref(), Some(root.as_str()));

    let run = team.run(TEAM_ID, 1);
    assert_eq!(run.record.status, RunStatus::Ok);
    assert_eq!(run.record.project.as_deref(), Some(root.as_str()));

    // A copy of another repository is never turned on for this folder.
    let store = team.state().routines.store().unwrap();
    let other: TeamRow = serde_json::from_value(row(OTHER_ID, Some("git:github.com/o/other"), echo("x"))).unwrap();
    TeamFolder::of(&store).write_row(&other).unwrap();
    assert_eq!(
        code(team.enable(OTHER_ID, true, Some(root))),
        IpcErrorCode::InvalidInput
    );
}

#[test]
fn a_local_routine_with_the_same_id_wins_and_copies_need_trust_or_approval() {
    let Some(team) = team("team-routines-copy") else {
        return;
    };
    let state = team.state();

    team.sharing(Some((true, false)));
    team.rows(&[row(TEAM_ID, None, echo("team"))]);
    team.refresh();

    let pending = team.enable(TEAM_ID, true, None).expect("turned on");
    let copy = |view: &RoutineView| block_on(team_commands::copy(state, view.id.clone(), view.sha256.clone(), None));
    assert_eq!(
        code(copy(&pending)),
        IpcErrorCode::InvalidInput,
        "a pending one is not copied"
    );

    let approved = team.approve(&pending);
    let local = copy(&approved).expect("copied");
    assert_eq!((local.origin, local.status), (HookOrigin::Local, RoutineStatus::Active));

    assert_eq!(team.team_view(TEAM_ID).status, RoutineStatus::Duplicate);
    assert_eq!(team.scheduled(), [(TEAM_ID.to_owned(), HookOrigin::Local)]);
    assert_eq!(team.run(TEAM_ID, 1).record.origin, HookOrigin::Local);
}

#[test]
fn sharing_off_removes_the_team_routines_at_once() {
    let Some(team) = team("team-routines-off-switch") else {
        return;
    };

    team.sharing(Some((true, true)));
    team.rows(&[row(TEAM_ID, None, echo("team"))]);
    team.refresh();
    team.enable(TEAM_ID, true, None).expect("turned on");
    assert_eq!(team.scheduled().len(), 1);

    team.sharing(Some((false, false)));
    team.refresh();

    let store = team.state().routines.store().unwrap();
    assert!(!TeamFolder::of(&store).dir().exists());
    assert!(team.listed().iter().all(|view| view.origin == HookOrigin::Local));
    assert!(team.state().routines.team_settings().is_none());
    assert!(team.scheduled().is_empty());

    let gone = block_on(team.state().routines.run_now(TEAM_ID)).map_err(CommandError::from);
    assert_eq!(code(gone), IpcErrorCode::NotFound);
}

#[test]
fn sharing_updating_and_removing_go_through_harness_memory() {
    let Some(team) = team("team-routines-share") else {
        return;
    };
    let state = team.state();

    team.sharing(Some((true, false)));
    team.rows(&[]);
    team.refresh();

    let first = save(
        state,
        &draft("Mine", RoutineWhen::AppStart, command("echo one")),
        HookAuthor::Desktop,
    );
    let second = save(
        state,
        &draft("Also mine", RoutineWhen::AppStart, command("echo two")),
        HookAuthor::Desktop,
    );

    let shared = block_on(team_commands::share(state, first.id.clone(), first.sha256.clone())).expect("shared");
    assert_eq!(
        (shared.origin, shared.status),
        (HookOrigin::Team, RoutineStatus::Duplicate)
    );
    block_on(team_commands::share(state, second.id.clone(), second.sha256.clone())).expect("shared");
    assert_eq!(team.stored_rows().len(), 2);
    assert!(team.stored_rows().iter().all(|row| row["author"] == "me"));

    // The local original changes: the team copy offers an update, which replaces it.
    let edited = save(
        state,
        &RoutineDraft {
            id: Some(first.id.clone()),
            expected_sha256: Some(first.sha256.clone()),
            ..draft("Mine, renamed", RoutineWhen::AppStart, command("echo one"))
        },
        HookAuthor::Desktop,
    );
    assert!(team.team_view(&first.id).team_changed);

    let updated = block_on(team_commands::share(state, edited.id.clone(), edited.sha256.clone())).expect("updated");
    assert!(!updated.team_changed);
    assert_eq!(updated.sha256, team.stored_rows()[0]["sha256"]);

    let stale = block_on(team_commands::remove(state, first.id.clone(), "0".repeat(64)));
    assert_eq!(code(stale), IpcErrorCode::Conflict);
    block_on(team_commands::remove(state, first.id.clone(), updated.sha256.clone())).expect("removed");
    assert_eq!(team.stored_rows().len(), 1);

    let removed = block_on(team_commands::remove_mine(state)).expect("removed mine");
    assert_eq!((removed.deleted, removed.ids), (1, vec![second.id.clone()]));
    assert!(team.stored_rows().is_empty());

    let store = state.routines.store().unwrap();
    assert!(TeamFolder::of(&store).ids().is_empty());
    assert!(
        team.listed()
            .iter()
            .all(|view| view.origin == HookOrigin::Local && !view.team_changed),
        "the local routines stay"
    );
}

/// The team database settings with these team routines choices.
fn database(share: Option<bool>, trust: Option<bool>) -> MemoryRemoteChange {
    MemoryRemoteChange {
        enabled: true,
        kind: Some(MemoryRemoteKind::Postgres),
        url: Some("postgres://team@db.example.com/memory".to_owned()),
        token: None,
        repository_notes: MemoryNotesRouting::Remote,
        global_notes: MemoryNotesRouting::Remote,
        scripts: MemoryNotesRouting::Remote,
        trust_team_scripts: false,
        share_hooks: None,
        trust_team_hooks: None,
        share_routines: share,
        trust_team_routines: trust,
    }
}

#[test]
fn settings_save_the_routines_keys_only_for_a_server_that_keeps_team_routines() {
    let Some(team) = team("team-routines-settings") else {
        return;
    };
    let state = team.state();
    let file = memory_config_path(&state.env).unwrap();

    team.sharing(Some((false, false)));
    let saved = block_on(memory_remote::set(state, database(Some(true), Some(true)))).expect("saved");
    assert!(saved.routines_supported && saved.share_routines && saved.trust_team_routines);
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("routines = \"remote\"\ntrust_team_routines = true\n"),
        "{text}"
    );

    let read = block_on(memory_remote::get(state)).expect("read");
    assert_eq!((read.share_routines, read.trust_team_routines), (true, true));

    // An older server: the keys are not written, and it has no team routines.
    team.sharing(None);
    let older = block_on(memory_remote::set(state, database(Some(true), Some(true)))).expect("saved");
    assert!(!older.routines_supported && !older.share_routines);
    assert!(!std::fs::read_to_string(&file).unwrap().contains("routines"));
    assert!(state.routines.team_settings().is_none());

    team.refresh();
    let listing = block_on(commands::list(state, None, RoutineFilter::default())).expect("listed");
    assert!(listing.team.is_none());
}
