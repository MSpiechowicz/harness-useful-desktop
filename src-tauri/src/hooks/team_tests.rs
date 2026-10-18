//! Team hooks in chats: a `harness-memory` stand-in (a copy of `fake_agent`, see its
//! `memory_hooks` docs) shares hooks, a refresh writes the copies, and the scripted agent checks
//! the exact prompt it receives.

use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

use harness_business::ecosystem::memory_config_path;
use harness_business::hooks::sha256_hex;
use harness_business::hooks::team::TeamFolder;
use harness_business::hooks::{HookCapabilities, HookStore};
use harness_business::model::{
    AgentAction, HookAction, HookAuthor, HookDraft, HookEvent, HookOrigin, HookScope, HookStatus, HookView,
};
use serde_json::json;
use tauri::async_runtime::block_on;

use crate::commands::{agent, hooks as hook_commands};
use crate::test_support::{
    AgentSetup, agent_setup, collecting_channel, fake_agent, place_memory_server, start_request,
};

/// The id of the team hook the stand-in lists.
const TEAM_ID: &str = "7eaa0000000000000000000000000001";

/// A chat app state with an installed `harness-memory` that shares hooks; `None` (the test skips)
/// when `fake_agent` is not built.
struct Team {
    setup: AgentSetup,
    server: PathBuf,
}

fn team(scenario: &str) -> Option<Team> {
    let binary = fake_agent()?;
    let setup = agent_setup(&binary, scenario);
    let server = place_memory_server(&setup.state.env.home)?;

    Some(Team { setup, server })
}

impl Team {
    fn store(&self) -> HookStore {
        self.setup.state.hooks.store().expect("a hooks folder")
    }

    /// What the stand-in's `hooks config` answers; `None` makes it an older server. The config
    /// file changes with it, as a save in Settings changes it.
    fn sharing(&self, config: Option<(bool, bool)>) {
        let dir = self.server.parent().unwrap();
        let answer = dir.join("hooks-config.json");
        let file = memory_config_path(&self.setup.state.env).expect("a config path");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();

        match config {
            Some((remote, trust)) => {
                let hooks = if remote { "remote" } else { "local" };
                std::fs::write(&answer, json!({"hooks": hooks, "trustTeamHooks": trust}).to_string()).unwrap();
                std::fs::write(&file, format!("hooks = \"{hooks}\"\ntrust_team_hooks = {trust}\n")).unwrap();
            }
            None => {
                std::fs::remove_file(&answer).ok();
                std::fs::write(&file, "").unwrap();
            }
        }
    }

    /// The team's hooks, as the stand-in lists them.
    fn rows(&self, rows: &[serde_json::Value]) {
        let path = self.server.parent().unwrap().join("hooks-rows.json");
        std::fs::write(path, serde_json::to_string(rows).unwrap()).unwrap();
    }

    fn refresh(&self) {
        block_on(self.setup.state.hooks.refresh_team());
    }

    fn listed(&self) -> Vec<HookView> {
        let project = Some(self.setup.project.to_string_lossy().into_owned());

        block_on(hook_commands::list(&self.setup.state, project))
            .expect("listed")
            .hooks
    }

    fn team_view(&self) -> HookView {
        self.listed()
            .into_iter()
            .find(|view| view.origin == HookOrigin::Team)
            .expect("a team hook is listed")
    }

    fn prompt(&self, key: &str, message: &str) {
        let action = AgentAction::Prompt {
            message: message.to_owned(),
            images: Vec::new(),
            streaming_behavior: None,
        };

        block_on(super::chat::send(&self.setup.state, key, action)).expect("the agent accepts the prompt");
    }

    fn start(&self) -> String {
        let (channel, _) = collecting_channel();

        block_on(agent::start(
            &self.setup.state,
            start_request(&self.setup.project),
            channel,
        ))
        .expect("the chat starts")
        .key
    }
}

impl Drop for Team {
    fn drop(&mut self) {
        block_on(self.setup.state.agents.stop_all());
    }
}

/// A global team hook row shared by "ana", `action` its action.
fn row(name: &str, action: serde_json::Value) -> serde_json::Value {
    row_in(None, name, action)
}

/// A team hook row of the repository `repo` (the key the stand-in gives every project), or a
/// global one.
fn row_in(repo: Option<&str>, name: &str, action: serde_json::Value) -> serde_json::Value {
    let body = format!(
        "{}\n",
        serde_json::to_string_pretty(&json!({
            "version": 1,
            "id": TEAM_ID,
            "name": name,
            "description": null,
            "scope": if repo.is_some() { "project" } else { "global" },
            "project": repo,
            "event": "promptSubmit",
            "matcher": null,
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
        "id": TEAM_ID,
        "repo": repo,
        "sha256": sha256_hex(body.as_bytes()),
        "author": "ana",
        "createdAt": "2026-10-01T10:00:00Z",
        "updatedAt": "2026-10-01T10:00:00Z",
        "body": body,
    })
}

fn local_hook(store: &HookStore, name: &str, text: &str) {
    let draft = HookDraft {
        id: None,
        expected_sha256: None,
        name: name.to_owned(),
        description: None,
        scope: HookScope::Global,
        project: None,
        event: HookEvent::PromptSubmit,
        matcher: None,
        action: HookAction::AddContext { text: text.to_owned() },
        enabled: true,
    };

    store
        .save(
            &draft,
            HookAuthor::Desktop,
            HookCapabilities::default(),
            UNIX_EPOCH + Duration::from_secs(1_791_000_000),
        )
        .expect("hook saved");
}

#[cfg(unix)]
#[test]
fn a_trusted_team_hook_runs_after_the_local_hooks() {
    let Some(team) = team("omp_hooks_team.jsonl") else {
        return;
    };
    let store = team.store();
    local_hook(&store, "Local rule", "Be brief.");

    team.sharing(Some((true, true)));
    team.rows(&[row(
        "Team check",
        json!({"type": "command", "command": "printf 'team=%s' \"$HARNESS_HOOK_EVENT\""}),
    )]);
    team.refresh();

    assert!(TeamFolder::of(&store).dir().join(format!("{TEAM_ID}.json")).is_file());
    let view = team.team_view();
    assert_eq!(
        (view.status, view.team_author.as_deref()),
        (HookStatus::Active, Some("ana"))
    );

    // Agents never see team hooks.
    assert!(
        store
            .list(HookCapabilities::default())
            .hooks
            .iter()
            .all(|view| view.id != TEAM_ID)
    );

    let key = team.start();
    team.prompt(&key, "Hello");
}

#[test]
fn an_untrusted_team_hook_runs_once_approved_and_stops_when_sharing_is_off() {
    let Some(team) = team("omp_hooks_pending.jsonl") else {
        return;
    };
    let store = team.store();

    team.sharing(Some((true, false)));
    team.rows(&[row("Agent rule", json!({"type": "addContext", "text": "Use tabs."}))]);
    team.refresh();

    let pending = team.team_view();
    assert_eq!(pending.status, HookStatus::Pending);

    let key = team.start();
    team.prompt(&key, "Hello");

    let approved = block_on(hook_commands::approve(&team.setup.state, pending.id, pending.sha256)).expect("approved");
    assert_eq!(
        (approved.status, approved.origin),
        (HookStatus::Active, HookOrigin::Team)
    );
    team.prompt(&key, "Again");

    team.sharing(Some((false, false)));
    team.refresh();

    assert!(!TeamFolder::of(&store).dir().exists());
    assert!(team.listed().iter().all(|view| view.origin == HookOrigin::Local));
    assert!(team.setup.state.hooks.team_settings().is_none());
}

#[test]
fn an_older_memory_server_shows_no_team_hooks() {
    let Some(team) = team("omp_hooks_plain.jsonl") else {
        return;
    };

    team.sharing(None);
    team.refresh();

    let listing = block_on(hook_commands::list(&team.setup.state, None)).expect("listed");
    assert!(listing.team.is_none());

    let key = team.start();
    team.prompt(&key, "Hello");
}

#[test]
fn trust_is_dropped_while_harness_memory_cannot_be_asked() {
    let Some(team) = team("omp_hooks_plain.jsonl") else {
        return;
    };

    team.sharing(Some((true, true)));
    team.rows(&[row("Team rule", json!({"type": "addContext", "text": "Use tabs."}))]);
    team.refresh();
    assert_eq!(team.team_view().status, HookStatus::Active);

    // The server answers what this app cannot read (the config file changed, so it is asked).
    let dir = team.server.parent().unwrap();
    std::fs::write(dir.join("hooks-config.json"), "not json").unwrap();
    let file = memory_config_path(&team.setup.state.env).unwrap();
    std::fs::write(&file, "hooks = \"remote\"\n").unwrap();
    team.refresh();

    let view = team.team_view();
    assert_eq!(
        view.status,
        HookStatus::Pending,
        "the copy stays and waits for approval"
    );
    let settings = team.setup.state.hooks.team_settings().expect("still shared");
    assert!(!settings.trust && settings.problem.is_some());
}

#[test]
fn a_roots_repository_is_used_only_once_this_session_confirmed_it() {
    let Some(team) = team("omp_hooks_plain.jsonl") else {
        return;
    };
    let hooks = &team.setup.state.hooks;
    let root = harness_business::hooks::project_identity(&team.setup.project)
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let has_team_hook = || !block_on(hooks.hook_set(Some(&root))).is_empty();

    team.sharing(Some((true, true)));
    team.rows(&[row_in(
        Some("git:github.com/o/r"),
        "Repo rule",
        json!({"type": "addContext", "text": "Use tabs."}),
    )]);
    team.refresh();

    // Not asked for yet: the view shows no team hook of the project, and a chat runs none.
    assert!(team.listed().iter().all(|view| view.origin == HookOrigin::Local));
    assert!(!has_team_hook());
    assert!(!hooks.root_verified(&root));

    // The next round asks for the root the view and the chat used.
    team.refresh();
    assert!(hooks.root_verified(&root));
    assert_eq!(team.team_view().status, HookStatus::Active);
    assert!(has_team_hook());

    // Unmounted: the copies stay, but the root's mapping is not used until it is asked again.
    let parked = team.setup.project.with_file_name("parked");
    std::fs::rename(&team.setup.project, &parked).unwrap();
    team.refresh();
    assert!(!hooks.root_verified(&root));
    assert!(!has_team_hook());
    assert!(
        TeamFolder::of(&team.store())
            .dir()
            .join(format!("{TEAM_ID}.json"))
            .is_file()
    );

    std::fs::rename(&parked, &team.setup.project).unwrap();
    team.refresh();
    assert!(has_team_hook());
}
