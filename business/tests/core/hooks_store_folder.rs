//! What the hooks folder holds: run order, the per-scope limit, which files are ignored, project
//! identity, and the seeded defaults (created once, never again after a delete).

use crate::hook_store_support;

mod hooks_store_folder {
    use std::path::Path;

    use harness_business::BusinessError;
    use harness_business::hooks::{MAX_HOOKS_PER_SCOPE, SEED_MEMORY_SAVE_TEXT, project_identity, seed_defaults};
    use harness_business::model::{HookAction, HookAuthor, HookRequirement, HookScope, HookSeed, HookStatus, HookView};
    use pretty_assertions::assert_eq;
    use serde_json::{Value, json};

    use super::hook_store_support::{
        DESKTOP, MEMORY, NO_MEMORY, agent_file, at, draft, file_of, folder, hex_id, saved_with_bytes, view,
    };

    #[test]
    fn hooks_run_global_first_then_by_creation_then_id() {
        let folder = folder();
        let project = tempfile::tempdir().unwrap();
        let root = project_identity(project.path()).unwrap().to_string_lossy().into_owned();

        agent_file(
            &folder.store,
            &hex_id(3),
            "2026-10-17T10:00:00Z",
            json!("project"),
            json!(root),
        );
        agent_file(
            &folder.store,
            &hex_id(2),
            "2026-10-17T10:00:01Z",
            json!("global"),
            Value::Null,
        );
        agent_file(
            &folder.store,
            &hex_id(5),
            "2026-10-17T10:00:00.5Z",
            json!("global"),
            Value::Null,
        );
        agent_file(
            &folder.store,
            &hex_id(4),
            "2026-10-17T10:00:00Z",
            json!("global"),
            Value::Null,
        );
        agent_file(
            &folder.store,
            &hex_id(1),
            "2026-10-17T10:00:00Z",
            json!("global"),
            Value::Null,
        );

        let order: Vec<String> = folder
            .store
            .list(NO_MEMORY)
            .hooks
            .into_iter()
            .map(|view| view.id)
            .collect();

        assert_eq!(order, vec![hex_id(1), hex_id(4), hex_id(5), hex_id(2), hex_id(3)]);
    }

    #[test]
    fn each_scope_holds_at_most_64_hooks() {
        let folder = folder();
        let project = tempfile::tempdir().unwrap();
        let root = project_identity(project.path()).unwrap().to_string_lossy().into_owned();

        for number in 0..=MAX_HOOKS_PER_SCOPE {
            let created = format!("2026-10-17T10:{:02}:{:02}Z", number / 60, number % 60);
            agent_file(
                &folder.store,
                &hex_id(number + 1),
                &created,
                json!("global"),
                Value::Null,
            );
        }

        agent_file(
            &folder.store,
            &hex_id(500),
            "2026-10-17T09:00:00Z",
            json!("project"),
            json!(root),
        );

        let listed = folder.store.list(NO_MEMORY);
        let over: Vec<&HookView> = listed
            .hooks
            .iter()
            .filter(|view| view.status == HookStatus::Invalid)
            .collect();

        // The newest global hook is over the limit; the project scope counts separately.
        assert_eq!(over.len(), 1);
        assert_eq!(over[0].id, hex_id(MAX_HOOKS_PER_SCOPE + 1));
        assert!(over[0].problem.as_deref().unwrap().contains("limit"));
        assert_eq!(view(&listed.hooks, &hex_id(500)).status, HookStatus::Pending);

        // An agent's add counts every valid hook.
        let refused = folder
            .store
            .save(&draft("One more"), HookAuthor::Agent, NO_MEMORY, at(0));
        assert!(matches!(refused, Err(BusinessError::InvalidInput(_))), "{refused:?}");

        let mut project_hook = draft("Project rules");
        project_hook.scope = HookScope::Project;
        project_hook.project = Some(project.path().to_string_lossy().into_owned());
        let saved = folder.store.save(&project_hook, DESKTOP, NO_MEMORY, at(0)).unwrap();
        assert_eq!(saved.hook.unwrap().project, Some(root));
    }

    #[test]
    fn approved_hooks_keep_their_place_when_pending_files_fill_a_scope() {
        let folder = folder();

        for number in 0..=MAX_HOOKS_PER_SCOPE {
            let created = format!("2026-10-17T10:{:02}:{:02}Z", number / 60, number % 60);
            agent_file(
                &folder.store,
                &hex_id(number + 1),
                &created,
                json!("global"),
                Value::Null,
            );
        }

        // Files waiting for review do not count against the user's own hooks.
        let saved = folder.store.save(&draft("Mine"), DESKTOP, NO_MEMORY, at(0)).unwrap();
        assert_eq!(saved.status, HookStatus::Active);

        let listed = folder.store.list(NO_MEMORY);
        assert_eq!(view(&listed.hooks, &saved.id).status, HookStatus::Active);

        let over: Vec<&str> = listed
            .hooks
            .iter()
            .filter(|view| view.status == HookStatus::Invalid)
            .map(|view| view.id.as_str())
            .collect();
        assert_eq!(over, vec![hex_id(MAX_HOOKS_PER_SCOPE), hex_id(MAX_HOOKS_PER_SCOPE + 1)]);
    }

    #[test]
    fn the_desktop_refuses_a_hook_over_the_limit_of_approved_ones() {
        let folder = folder();

        for number in 0..MAX_HOOKS_PER_SCOPE {
            folder
                .store
                .save(&draft(&format!("Hook {number}")), DESKTOP, NO_MEMORY, at(0))
                .unwrap();
        }

        let refused = folder.store.save(&draft("One more"), DESKTOP, NO_MEMORY, at(1));
        assert!(matches!(refused, Err(BusinessError::InvalidInput(_))), "{refused:?}");
    }

    #[test]
    fn only_regular_id_files_are_read() {
        let folder = folder();
        let (saved, bytes) = saved_with_bytes(&folder, "Answer style");
        let dir = folder.store.hooks_dir();

        for name in [
            format!(".{}.json.tmp-1-2", saved.id),
            format!("{}.json.bak", saved.id),
            format!("{}.json~", saved.id),
            "notes.json".to_owned(),
            format!("{}.json", saved.id.to_uppercase()),
        ] {
            std::fs::write(dir.join(name), &bytes).unwrap();
        }

        std::fs::create_dir(dir.join(format!("{}.json", hex_id(9)))).unwrap();

        #[cfg(unix)]
        std::os::unix::fs::symlink(
            file_of(&folder.store, &saved.id),
            dir.join(format!("{}.json", hex_id(8))),
        )
        .unwrap();

        let listed = folder.store.list(NO_MEMORY);
        let ids: Vec<&str> = listed.hooks.iter().map(|view| view.id.as_str()).collect();
        assert_eq!(ids, vec![saved.id.as_str()]);
    }

    #[test]
    fn an_oversized_file_is_invalid() {
        let folder = folder();
        let id = hex_id(1);
        std::fs::create_dir_all(folder.store.hooks_dir()).unwrap();
        std::fs::write(file_of(&folder.store, &id), vec![b' '; 16 * 1024 + 1]).unwrap();

        let listed = folder.store.list(NO_MEMORY);
        assert_eq!(listed.hooks[0].status, HookStatus::Invalid);
        assert!(listed.hooks[0].problem.as_deref().unwrap().contains("16 KiB"));
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_others_can_write_is_ignored() {
        use std::os::unix::fs::PermissionsExt;

        let folder = folder();
        folder
            .store
            .save(&draft("Answer style"), DESKTOP, NO_MEMORY, at(0))
            .unwrap();
        std::fs::set_permissions(folder.store.hooks_dir(), std::fs::Permissions::from_mode(0o772)).unwrap();

        let listed = folder.store.list(NO_MEMORY);
        assert!(listed.hooks.is_empty());
        assert!(listed.folder_problem.is_some());

        let refused = folder.store.save(&draft("Another"), DESKTOP, NO_MEMORY, at(1));
        assert!(matches!(refused, Err(BusinessError::Forbidden(_))), "{refused:?}");
    }

    #[cfg(unix)]
    #[test]
    fn a_root_of_the_user_others_can_write_to_loses_only_their_write_access() {
        use std::os::unix::fs::PermissionsExt;

        let folder = folder();
        let saved = folder
            .store
            .save(&draft("Answer style"), DESKTOP, NO_MEMORY, at(0))
            .unwrap();
        let mode = || std::fs::metadata(folder.store.root()).unwrap().permissions().mode() & 0o777;

        for (loose, tightened) in [(0o775, 0o755), (0o777, 0o755), (0o720, 0o700)] {
            std::fs::set_permissions(folder.store.root(), std::fs::Permissions::from_mode(loose)).unwrap();

            let listed = folder.store.list(NO_MEMORY);
            assert_eq!(listed.folder_problem, None);
            assert_eq!(listed.hooks[0].id, saved.id);
            assert_eq!(mode(), tightened, "{loose:o}");
        }
    }

    #[test]
    fn the_project_root_is_the_nearest_git_ancestor() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        let nested = repo.join("src/deep");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::create_dir(repo.join(".git")).unwrap();

        let worktree = temp.path().join("worktree");
        std::fs::create_dir_all(worktree.join("sub")).unwrap();
        std::fs::write(worktree.join(".git"), "gitdir: elsewhere\n").unwrap();

        let plain = temp.path().join("plain");
        std::fs::create_dir(&plain).unwrap();

        let real = |path: &Path| dunce::canonicalize(path).unwrap();

        assert_eq!(project_identity(&nested).unwrap(), real(&repo));
        assert_eq!(project_identity(&worktree.join("sub")).unwrap(), real(&worktree));
        assert_eq!(project_identity(&plain).unwrap(), real(&plain));
        assert!(project_identity(&temp.path().join("missing")).is_err());
    }

    #[test]
    fn the_defaults_are_seeded_once_and_never_again() {
        let folder = folder();

        assert_eq!(seed_defaults(&folder.store, NO_MEMORY, at(0)).unwrap(), vec![]);
        assert!(folder.store.list(MEMORY).hooks.is_empty());

        let created = seed_defaults(&folder.store, MEMORY, at(0)).unwrap();
        assert_eq!(
            created,
            vec![
                HookSeed::MemorySearch,
                HookSeed::MemorySave,
                HookSeed::UsefulSkills,
                HookSeed::BranchSync,
                HookSeed::MemoryFirst
            ]
        );

        let listed = folder.store.list(MEMORY);
        assert_eq!(listed.hooks.len(), 5);

        for view in &listed.hooks {
            let hook = view.hook.as_ref().unwrap();
            assert_eq!(view.status, HookStatus::Active);
            assert_eq!(hook.scope, HookScope::Global);
            assert_eq!(hook.requires, vec![HookRequirement::HarnessMemory]);
            assert_eq!(
                (hook.created_by, hook.updated_by),
                (HookAuthor::Desktop, HookAuthor::Desktop)
            );
        }

        let save = listed
            .hooks
            .iter()
            .find(|view| view.hook.as_ref().unwrap().seed == Some(HookSeed::MemorySave))
            .unwrap();
        assert_eq!(save.name.as_deref(), Some("Save what you learned"));
        assert_eq!(
            save.hook.as_ref().unwrap().action,
            HookAction::FollowUp {
                text: SEED_MEMORY_SAVE_TEXT.to_owned()
            }
        );

        assert_eq!(seed_defaults(&folder.store, MEMORY, at(1)).unwrap(), vec![]);
        assert_eq!(folder.store.list(MEMORY).hooks.len(), 5);

        for view in &listed.hooks {
            folder.store.delete(&view.id, DESKTOP, None).unwrap();
        }

        assert_eq!(seed_defaults(&folder.store, MEMORY, at(2)).unwrap(), vec![]);
        assert!(folder.store.list(MEMORY).hooks.is_empty());
    }

    #[test]
    fn a_seed_already_in_the_folder_is_not_duplicated() {
        let folder = folder();
        seed_defaults(&folder.store, MEMORY, at(0)).unwrap();
        std::fs::remove_file(folder.store.config_dir().join("hooks-state.json")).unwrap();

        assert_eq!(seed_defaults(&folder.store, MEMORY, at(1)).unwrap(), vec![]);
        assert_eq!(folder.store.list(MEMORY).hooks.len(), 5);
    }
}
