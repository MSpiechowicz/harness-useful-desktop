//! Saving is opt-in: a chat's project is off until the user turned saving on for every project or
//! for its repository (a git worktree counts as its main repository). The first message of a chat
//! says so, a chat where saving is on hears nothing.

use crate::hook_support;
use crate::hooks_memory_notice;

pub(crate) mod hooks_memory_opt_in {
    use std::fs;

    use harness_business::ecosystem::{FeaturesSupport, Pin, ensure_global, update_global};
    use harness_business::hooks::{MemoryGuardState, SaveRule, read_memory_guards, takes_part};
    use harness_business::model::{MemoryGuardKind, MemoryGuardsPatch};
    use pretty_assertions::assert_eq;

    use super::hook_support::chat;
    use super::hooks_memory_notice::hooks_memory_notice::told;

    const KEY: &str = "3b2f6c1e-8a4d-4e5f-9a0b-1c2d3e4f5a6b";

    fn guards(notes: SaveRule, scripts: SaveRule) -> MemoryGuardState {
        MemoryGuardState { notes, scripts }
    }

    #[test]
    fn the_first_message_of_a_chat_says_saving_is_off_for_a_project_not_turned_on() {
        let data = tempfile::tempdir().unwrap();
        let dir = data.path();
        let repo = tempfile::tempdir().unwrap();
        ensure_global(dir).unwrap();

        let mut session = chat(true, false);
        session.set_memory_guards(read_memory_guards(dir, Some(KEY), Some(repo.path())));
        assert_eq!(
            told(&mut session).as_deref(),
            Some("Harness Desktop: saving notes and scripts is off for this project.")
        );
        assert_eq!(told(&mut session), None, "said once");

        let root = turn_notes_on_for(dir, repo.path());
        turn_every_project(dir, MemoryGuardKind::Scripts, true);

        let mut on = chat(true, false);
        on.set_memory_guards(read_memory_guards(dir, Some(KEY), Some(&root)));
        assert_eq!(told(&mut on), None, "a chat where saving is on hears nothing");
    }

    fn listed(save_guards: bool, save_opt_in: bool) -> FeaturesSupport {
        FeaturesSupport::Listed {
            notes_remote_local: false,
            notes_hide: false,
            push_json: false,
            scripts_remote_local: false,
            scripts_hide: false,
            scripts_push_json: false,
            save_guards,
            save_opt_in,
            embedder: false,
            settings: false,
            notes_lang: false,
            notes_useful: false,
            notes_authors: false,
            notes_author_filter: false,
        }
    }

    #[test]
    fn the_guards_speak_only_where_harness_memory_takes_part() {
        for off_pref in [listed(true, true), listed(true, false), FeaturesSupport::Unknown] {
            assert!(!takes_part(false, off_pref), "Project memory is off: {off_pref:?}");
        }

        assert!(!takes_part(true, FeaturesSupport::NotInstalled), "not installed");
        assert!(!takes_part(true, FeaturesSupport::Unsupported), "predates features");
        assert!(
            !takes_part(true, listed(false, false)),
            "no save guards: saves as before"
        );
        assert!(takes_part(true, listed(true, true)));
        assert!(
            takes_part(true, listed(true, false)),
            "a server without opt-in refuses the file, so the notice is true"
        );
        assert!(
            takes_part(true, FeaturesSupport::Unknown),
            "could not ask: off is the safe answer"
        );
    }

    #[test]
    fn an_entry_with_no_pin_does_not_say_saving_is_on() {
        let data = tempfile::tempdir().unwrap();
        let dir = data.path();
        let repo = tempfile::tempdir().unwrap();
        let root = turn_notes_on_for(dir, repo.path());
        let on = guards(SaveRule::Saved, SaveRule::ProjectOff);
        assert_eq!(read_memory_guards(dir, Some(KEY), Some(&root)), on);

        // A file from before the pins lists the entry and pins nothing: Harness Memory takes it as
        // naming nothing, so the chat is not told saving is on.
        let file = dir.join("guards").join("global.json");
        let unpinned = fs::read_to_string(&file).unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&unpinned).unwrap();
        value["notes"].as_object_mut().unwrap().remove("repoKeys");
        value["scripts"].as_object_mut().unwrap().remove("repoKeys");
        fs::write(&file, value.to_string()).unwrap();

        assert_eq!(
            read_memory_guards(dir, Some(KEY), Some(&root)),
            guards(SaveRule::ProjectOff, SaveRule::ProjectOff)
        );

        // A pin whose key changed since still reads as on here: only Harness Memory can compute keys.
        fs::write(&file, unpinned.replace("null", "\"git:example.invalid/acme/old\"")).unwrap();
        assert_eq!(read_memory_guards(dir, Some(KEY), Some(&root)), on);
    }

    #[test]
    fn saving_is_off_by_default_and_on_where_the_user_turned_it_on() {
        let data = tempfile::tempdir().unwrap();
        let dir = data.path();
        let repo = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let off = guards(SaveRule::ProjectOff, SaveRule::ProjectOff);

        assert_eq!(
            read_memory_guards(dir, Some(KEY), Some(repo.path())),
            off,
            "no guards file"
        );
        assert_eq!(read_memory_guards(dir, None, None), off, "no project");

        ensure_global(dir).unwrap();
        assert_eq!(
            read_memory_guards(dir, Some(KEY), Some(repo.path())),
            off,
            "a fresh file"
        );

        let root = turn_notes_on_for(dir, repo.path());
        assert_eq!(
            read_memory_guards(dir, Some(KEY), Some(&root)),
            guards(SaveRule::Saved, SaveRule::ProjectOff),
            "notes are on for the listed project"
        );
        assert_eq!(read_memory_guards(dir, Some(KEY), Some(elsewhere.path())), off);

        turn_every_project(dir, MemoryGuardKind::Scripts, true);
        assert_eq!(
            read_memory_guards(dir, Some(KEY), Some(elsewhere.path())),
            guards(SaveRule::ProjectOff, SaveRule::Saved),
            "scripts are on for every project"
        );

        update_global(
            dir,
            &MemoryGuardsPatch {
                paused: Some(true),
                ..patch(MemoryGuardKind::Notes)
            },
            None,
        )
        .unwrap();
        assert_eq!(
            read_memory_guards(dir, Some(KEY), Some(&root)),
            guards(SaveRule::PausedEverywhere, SaveRule::Saved),
            "a pause comes before the project"
        );

        update_global(
            dir,
            &MemoryGuardsPatch {
                remove_repo: Some(root.to_string_lossy().into_owned()),
                paused: Some(false),
                ..patch(MemoryGuardKind::Notes)
            },
            None,
        )
        .unwrap();
        assert_eq!(
            read_memory_guards(dir, Some(KEY), Some(&root)),
            guards(SaveRule::ProjectOff, SaveRule::Saved),
            "turned off again"
        );
    }

    #[test]
    fn a_worktree_of_a_listed_repository_is_on_and_a_separate_clone_is_not() {
        let data = tempfile::tempdir().unwrap();
        let dir = data.path();
        let folders = tempfile::tempdir().unwrap();
        let base = dunce::canonicalize(folders.path()).unwrap();

        // A repository, one of its linked worktrees and an unrelated checkout, as git lays them out.
        let main = base.join("main");
        let admin = main.join(".git").join("worktrees").join("w");
        let linked = base.join("w");
        let other = base.join("other");
        fs::create_dir_all(&admin).unwrap();
        fs::create_dir_all(&linked).unwrap();
        fs::create_dir_all(other.join(".git")).unwrap();
        fs::write(admin.join("commondir"), "../..\n").unwrap();
        fs::write(admin.join("gitdir"), format!("{}\n", linked.join(".git").display())).unwrap();
        fs::write(linked.join(".git"), format!("gitdir: {}\n", admin.display())).unwrap();

        turn_every_project(dir, MemoryGuardKind::Scripts, false);
        let listed = turn_notes_on_for(dir, &main);
        assert_eq!(listed, main);

        assert_eq!(
            read_memory_guards(dir, None, Some(&main)),
            guards(SaveRule::Saved, SaveRule::ProjectOff)
        );
        assert_eq!(
            read_memory_guards(dir, None, Some(&linked)),
            guards(SaveRule::Saved, SaveRule::ProjectOff),
            "the worktree is on with its repository"
        );
        assert_eq!(
            read_memory_guards(dir, None, Some(&other)),
            guards(SaveRule::ProjectOff, SaveRule::ProjectOff),
            "another checkout is not"
        );
    }

    const OFF: &str = "Harness Desktop: saving notes and scripts is off for this project.";

    /// Whether the memory-save hook fires in a run that settles now.
    fn memory_save_fires(session: &mut harness_business::hooks::HookSession) -> bool {
        use harness_business::hooks::EventOutcome;
        use harness_business::model::{HookEvent, HookSeed};

        use super::hook_support::{follow_up, set, user_run};

        let mut memory_save = follow_up(2, HookEvent::TurnEnd);
        memory_save.seed = Some(HookSeed::MemorySave);

        matches!(user_run(session, &set(vec![memory_save])), EventOutcome::FollowUp(_))
    }

    #[test]
    fn a_chat_is_told_nothing_while_harness_memory_takes_no_part_and_only_a_change_when_it_does_again() {
        let off = guards(SaveRule::ProjectOff, SaveRule::ProjectOff);
        let mut session = chat(true, false);

        session.set_memory_guards(off);
        assert_eq!(told(&mut session).as_deref(), Some(OFF));
        assert!(!memory_save_fires(&mut session), "skipped while both kinds are off");

        // Project memory is turned off mid-chat: no "on again" notice, and the hook is not held
        // back by guards Harness Memory no longer applies.
        session.set_memory_apart();
        assert_eq!(told(&mut session), None);
        assert!(memory_save_fires(&mut session));
        assert_eq!(told(&mut session), None, "and nothing on any later message");

        // Back on, the state unchanged since the agent was told: nothing to say.
        session.set_memory_guards(off);
        assert_eq!(told(&mut session), None);
        assert!(!memory_save_fires(&mut session), "skipped again");

        // Off once more and a change meanwhile: the change is told, compared with what the agent
        // heard last.
        session.set_memory_apart();
        assert_eq!(told(&mut session), None);

        session.set_memory_guards(guards(SaveRule::ChatOff, SaveRule::Saved));
        assert_eq!(
            told(&mut session).as_deref(),
            Some("Harness Desktop: saving notes is off for this chat. Scripts are saved.")
        );

        // The project is turned on while Harness Memory takes no part: the agent hears it is on.
        session.set_memory_apart();
        session.set_memory_guards(MemoryGuardState::OPEN);
        assert_eq!(
            told(&mut session).as_deref(),
            Some("Harness Desktop: saving notes and scripts is on again for this chat.")
        );
    }

    #[test]
    fn a_chat_that_started_apart_hears_the_state_when_harness_memory_takes_part() {
        let mut session = chat(true, false);

        session.set_memory_apart();
        assert_eq!(told(&mut session), None);

        session.set_memory_guards(guards(SaveRule::ProjectOff, SaveRule::ProjectOff));
        assert_eq!(told(&mut session).as_deref(), Some(OFF));
    }

    pub(crate) fn patch(kind: MemoryGuardKind) -> MemoryGuardsPatch {
        MemoryGuardsPatch {
            kind,
            paused: None,
            every_project: None,
            add_repo: None,
            remove_repo: None,
        }
    }

    /// Turns saving on for every project, notes and scripts.
    pub(crate) fn turn_on_everywhere(dir: &std::path::Path) {
        for kind in [MemoryGuardKind::Notes, MemoryGuardKind::Scripts] {
            turn_every_project(dir, kind, true);
        }
    }

    pub(crate) fn turn_every_project(dir: &std::path::Path, kind: MemoryGuardKind, on: bool) {
        let change = MemoryGuardsPatch {
            every_project: Some(on),
            ..patch(kind)
        };
        update_global(dir, &change, None).unwrap();
    }

    /// Turns notes on for the project at `folder` and returns the root it was stored as.
    pub(crate) fn turn_notes_on_for(dir: &std::path::Path, folder: &std::path::Path) -> std::path::PathBuf {
        let on = update_global(
            dir,
            &MemoryGuardsPatch {
                add_repo: Some(folder.to_string_lossy().into_owned()),
                ..patch(MemoryGuardKind::Notes)
            },
            // The hooks compare paths only: the key is the server's to give.
            Some(Pin(None)),
        )
        .unwrap();

        std::path::PathBuf::from(&on.notes.on_repos[0])
    }
}
