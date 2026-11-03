//! Installing the Harness Skills seeds their hook: through the app's own installers, and when a
//! chat finds skills that a harness's own plugin or package action installed.

use harness_business::ecosystem::Companion;
use harness_business::hooks::HookCapabilities;
use harness_business::model::{HarnessId, HookAuthor, HookSeed};
use tauri::async_runtime::block_on;

use super::after_member_install;
use crate::state::AppState;
use crate::test_support::{TempDir, home_state};

/// The seeds of the hook files in the folder.
fn seeded(state: &AppState) -> Vec<HookSeed> {
    let store = state.hooks.store().expect("a hooks folder");

    store
        .list(HookCapabilities::default())
        .hooks
        .into_iter()
        .filter_map(|view| view.hook.and_then(|hook| hook.seed))
        .collect()
}

/// PI's user settings name the skills package: they count as installed for PI.
fn install_pi_skills(state: &AppState) {
    let agent = state.env.home.join(".pi").join("agent");
    std::fs::create_dir_all(&agent).expect("create the agent folder");
    std::fs::write(
        agent.join("settings.json"),
        r#"{"packages": ["git:github.com/MSpiechowicz/harness-useful-skills"]}"#,
    )
    .expect("write settings");
}

#[test]
fn installing_the_skills_for_opencode_or_codex_brings_their_hook_back() {
    for companion in [Companion::OpenCodeSkills, Companion::CodexSkills] {
        let dir = TempDir::new("skills-install-seeds");
        let state = home_state(&dir);
        assert_eq!(seeded(&state), Vec::<HookSeed>::new());

        block_on(after_member_install(&state.hooks, companion));

        assert_eq!(seeded(&state), [HookSeed::UsefulSkills], "{companion:?}");
    }
}

#[test]
fn installing_the_skills_brings_back_a_hook_the_user_deleted() {
    let dir = TempDir::new("skills-install-restores");
    let state = home_state(&dir);

    block_on(after_member_install(&state.hooks, Companion::CodexSkills));
    let store = state.hooks.store().expect("a hooks folder");
    let listed = store.list(HookCapabilities::default()).hooks;
    let id = listed[0].id.clone();
    store.delete(&id, HookAuthor::Desktop, None).expect("delete the hook");
    assert_eq!(seeded(&state), Vec::<HookSeed>::new());

    block_on(after_member_install(&state.hooks, Companion::CodexSkills));

    assert_eq!(seeded(&state), [HookSeed::UsefulSkills]);
}

#[test]
fn a_chat_that_finds_skills_installed_outside_the_app_seeds_their_hook_once() {
    let dir = TempDir::new("skills-found-seeds");
    let state = home_state(&dir);

    assert!(!block_on(state.hooks.useful_skills_for(HarnessId::Pi)));
    assert_eq!(seeded(&state), Vec::<HookSeed>::new());

    install_pi_skills(&state);
    state.hooks.refresh_members();

    assert!(block_on(state.hooks.useful_skills_for(HarnessId::Pi)));
    assert_eq!(seeded(&state), [HookSeed::UsefulSkills]);

    // The user deletes it: a later lookup does not put it back.
    let store = state.hooks.store().expect("a hooks folder");
    let id = store.list(HookCapabilities::default()).hooks[0].id.clone();
    store.delete(&id, HookAuthor::Desktop, None).expect("delete the hook");
    state.hooks.refresh_members();

    assert!(block_on(state.hooks.useful_skills_for(HarnessId::Pi)));
    assert_eq!(seeded(&state), Vec::<HookSeed>::new());
}
