//! Writes by the MCP server on behalf of an agent: they are pending until the user approves them,
//! keep the file's origin, never touch the app config folder, and an agent removes only the
//! version it read of a hook an agent created. The app window keeps a copy of each approved
//! version for the pending diff, and rejecting a pending edit restores it.

mod hooks_agent {
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use harness_business::BusinessError;
    use harness_business::hooks::{HookCapabilities, HookStore};
    use harness_business::model::{
        HookAction, HookApproval, HookAuthor, HookDraft, HookEvent, HookScope, HookStatus, HookView,
    };
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    const CAPS: HookCapabilities = HookCapabilities {
        memory_available: false,
        dashboard_available: false,
    };

    struct Folder {
        _temp: TempDir,
        store: HookStore,
        config: PathBuf,
    }

    fn folder() -> Folder {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config");
        let store = HookStore::new(temp.path().join("harness-hooks"), &config);

        Folder {
            _temp: temp,
            store,
            config,
        }
    }

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_791_000_000 + secs)
    }

    fn draft(text: &str) -> HookDraft {
        HookDraft {
            id: None,
            expected_sha256: None,
            name: "Answer style".to_owned(),
            description: None,
            scope: HookScope::Global,
            project: None,
            event: HookEvent::PromptSubmit,
            matcher: None,
            action: HookAction::AddContext { text: text.to_owned() },
            enabled: true,
        }
    }

    fn edit_of(view: &HookView, text: &str) -> HookDraft {
        HookDraft {
            id: Some(view.id.clone()),
            expected_sha256: Some(view.sha256.clone()),
            ..draft(text)
        }
    }

    #[test]
    fn an_agent_hook_is_pending_and_leaves_the_config_folder_alone() {
        let folder = folder();

        let added = folder
            .store
            .save(&draft("Short answers."), HookAuthor::Agent, CAPS, at(0))
            .unwrap();
        let hook = added.hook.clone().unwrap();

        assert_eq!(
            (added.status, added.approval),
            (HookStatus::Pending, HookApproval::Never)
        );
        assert_eq!(
            (hook.created_by, hook.updated_by),
            (HookAuthor::Agent, HookAuthor::Agent)
        );
        assert!(
            !folder.config.exists(),
            "the MCP server never writes the app config folder"
        );

        let edited = folder
            .store
            .save(&edit_of(&added, "Shorter answers."), HookAuthor::Agent, CAPS, at(1))
            .unwrap();
        assert_eq!(edited.status, HookStatus::Pending);
        assert!(!folder.config.exists());
    }

    #[test]
    fn an_agent_edit_of_a_user_hook_is_pending_with_the_approved_version_kept() {
        let folder = folder();
        let mine = folder
            .store
            .save(&draft("Short answers."), HookAuthor::Desktop, CAPS, at(0))
            .unwrap();
        assert_eq!(mine.approved, None);

        let changed = folder
            .store
            .save(&edit_of(&mine, "Long answers."), HookAuthor::Agent, CAPS, at(1))
            .unwrap();
        let hook = changed.hook.clone().unwrap();

        assert_eq!(
            (changed.status, changed.approval),
            (HookStatus::Pending, HookApproval::Changed)
        );
        assert_eq!(
            (hook.created_by, hook.updated_by),
            (HookAuthor::Desktop, HookAuthor::Agent)
        );
        assert_eq!(changed.approved, mine.hook);
        assert_eq!(folder.store.approved_snapshot(&mine.id), mine.hook);

        let listed = folder.store.list(CAPS);
        assert_eq!(listed.hooks[0].approved, mine.hook);

        folder.store.approve(&changed.id, &changed.sha256).unwrap();

        let approved = folder.store.get(&changed.id, CAPS).unwrap();
        assert_eq!((approved.status, approved.approved.clone()), (HookStatus::Active, None));
        assert_eq!(folder.store.approved_snapshot(&changed.id), changed.hook);
    }

    #[test]
    fn an_agent_removes_only_the_read_version_of_an_agent_hook() {
        let folder = folder();
        let theirs = folder
            .store
            .save(&draft("Short answers."), HookAuthor::Agent, CAPS, at(0))
            .unwrap();
        let mine = folder
            .store
            .save(&draft("Mine."), HookAuthor::Desktop, CAPS, at(0))
            .unwrap();

        let unnamed = folder.store.delete(&theirs.id, HookAuthor::Agent, None);
        assert!(matches!(unnamed, Err(BusinessError::InvalidInput(_))), "{unnamed:?}");

        let stale = folder
            .store
            .delete(&theirs.id, HookAuthor::Agent, Some(&"0".repeat(64)));
        assert!(matches!(stale, Err(BusinessError::Conflict(_))), "{stale:?}");

        let users = folder.store.delete(&mine.id, HookAuthor::Agent, Some(&mine.sha256));
        assert!(matches!(users, Err(BusinessError::Forbidden(_))), "{users:?}");

        // An agent's edit of the user's hook does not make it removable by the agent.
        let edited = folder
            .store
            .save(&edit_of(&mine, "Edited."), HookAuthor::Agent, CAPS, at(1))
            .unwrap();
        let still_users = folder.store.delete(&mine.id, HookAuthor::Agent, Some(&edited.sha256));
        assert!(
            matches!(still_users, Err(BusinessError::Forbidden(_))),
            "{still_users:?}"
        );

        folder
            .store
            .delete(&theirs.id, HookAuthor::Agent, Some(&theirs.sha256))
            .unwrap();

        let ids: Vec<String> = folder.store.list(CAPS).hooks.into_iter().map(|view| view.id).collect();
        assert_eq!(ids, vec![mine.id.clone()]);

        let gone = folder.store.delete(&theirs.id, HookAuthor::Agent, Some(&theirs.sha256));
        assert!(matches!(gone, Err(BusinessError::NotFound(_))), "{gone:?}");
    }

    #[test]
    fn rejecting_a_pending_edit_restores_the_approved_version() {
        let folder = folder();
        let mine = folder
            .store
            .save(&draft("Short answers."), HookAuthor::Desktop, CAPS, at(0))
            .unwrap();
        let changed = folder
            .store
            .save(&edit_of(&mine, "Long answers."), HookAuthor::Agent, CAPS, at(1))
            .unwrap();

        let stale = folder.store.reject(&changed.id, &mine.sha256, CAPS);
        assert!(matches!(stale, Err(BusinessError::Conflict(_))), "{stale:?}");

        let restored = folder
            .store
            .reject(&changed.id, &changed.sha256, CAPS)
            .unwrap()
            .unwrap();

        assert_eq!(restored.status, HookStatus::Active);
        assert_eq!(restored.sha256, mine.sha256);
        assert_eq!(restored.hook, mine.hook);
    }

    #[test]
    fn rejecting_a_never_approved_hook_deletes_it() {
        let folder = folder();
        let theirs = folder
            .store
            .save(&draft("Short answers."), HookAuthor::Agent, CAPS, at(0))
            .unwrap();

        assert_eq!(folder.store.reject(&theirs.id, &theirs.sha256, CAPS).unwrap(), None);
        assert!(folder.store.list(CAPS).hooks.is_empty());
    }

    #[test]
    fn deleting_in_the_app_drops_the_approved_copy() {
        let folder = folder();
        let mine = folder
            .store
            .save(&draft("Short answers."), HookAuthor::Desktop, CAPS, at(0))
            .unwrap();
        assert!(folder.store.approved_snapshot(&mine.id).is_some());

        folder.store.delete(&mine.id, HookAuthor::Desktop, None).unwrap();

        assert_eq!(folder.store.approved_snapshot(&mine.id), None);
        assert!(
            !folder
                .config
                .join("hook-approved")
                .join(format!("{}.json", mine.id))
                .exists()
        );
    }
}
