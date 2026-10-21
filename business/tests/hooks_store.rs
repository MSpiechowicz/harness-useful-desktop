//! The hooks folder as the desktop sees and writes it: saving approves the written bytes, any
//! byte change makes a hook pending, approving checks the reviewed version, what the desktop keeps
//! on a rewrite, and deleting.

mod hook_store_support;

mod hooks_store {
    use std::path::Path;

    use harness_business::BusinessError;
    use harness_business::hooks::{APPROVALS_FILE, seed_defaults, sha256_hex};
    use harness_business::model::{
        HookAction, HookApproval, HookAuthor, HookRequirement, HookScope, HookSeed, HookStatus, HookView,
    };
    use pretty_assertions::assert_eq;
    use serde_json::{Value, json};

    use super::hook_store_support::{
        DESKTOP, MEMORY, NO_MEMORY, agent_file, at, draft, edit_of, file_of, folder, hex_id, rewrite_same_values,
        saved_with_bytes, view,
    };

    #[test]
    fn a_saved_hook_is_active_at_once_and_written_by_the_desktop() {
        let folder = folder();

        let saved = folder
            .store
            .save(&draft("Answer style"), DESKTOP, NO_MEMORY, at(0))
            .unwrap();

        assert_eq!(saved.status, HookStatus::Active);
        assert_eq!(saved.approval, HookApproval::Approved);

        let bytes = std::fs::read(file_of(&folder.store, &saved.id)).unwrap();
        assert_eq!(saved.sha256, sha256_hex(&bytes));
        assert!(bytes.ends_with(b"}\n"));

        let hook = saved.hook.unwrap();
        assert_eq!(
            (hook.created_by, hook.updated_by),
            (HookAuthor::Desktop, HookAuthor::Desktop)
        );
        assert_eq!(hook.created_at, hook.updated_at);

        let listed = folder.store.list(NO_MEMORY);
        assert_eq!(listed.hooks.len(), 1);
        assert_eq!(listed.hooks[0].status, HookStatus::Active);
    }

    #[cfg(unix)]
    #[test]
    fn files_and_folders_are_private() {
        use std::os::unix::fs::PermissionsExt;

        let folder = folder();
        let saved = folder.store.save(&draft("Private"), DESKTOP, NO_MEMORY, at(0)).unwrap();

        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;

        assert_eq!(mode(&file_of(&folder.store, &saved.id)), 0o600);
        assert_eq!(mode(&folder.store.hooks_dir()), 0o700);
        assert_eq!(mode(folder.store.root()), 0o700);
    }

    #[test]
    fn rewriting_the_same_values_makes_a_hook_pending() {
        let folder = folder();
        let saved = folder
            .store
            .save(&draft("Answer style"), DESKTOP, NO_MEMORY, at(0))
            .unwrap();

        rewrite_same_values(&folder.store, &saved.id);

        let listed = folder.store.list(NO_MEMORY);
        let after = view(&listed.hooks, &saved.id);
        assert_eq!(after.status, HookStatus::Pending);
        assert_eq!(after.approval, HookApproval::Changed);
        assert_eq!(after.hook, saved.hook);
    }

    #[test]
    fn approving_needs_the_reviewed_version() {
        let folder = folder();
        let saved = folder
            .store
            .save(&draft("Answer style"), DESKTOP, NO_MEMORY, at(0))
            .unwrap();
        rewrite_same_values(&folder.store, &saved.id);

        let stale = folder.store.approve(&saved.id, &saved.sha256);
        assert!(matches!(stale, Err(BusinessError::Conflict(_))), "{stale:?}");
        assert_eq!(
            view(&folder.store.list(NO_MEMORY).hooks, &saved.id).status,
            HookStatus::Pending
        );

        let pending = view(&folder.store.list(NO_MEMORY).hooks, &saved.id).clone();
        folder.store.approve(&saved.id, &pending.sha256).unwrap();

        assert_eq!(
            view(&folder.store.list(NO_MEMORY).hooks, &saved.id).status,
            HookStatus::Active
        );
    }

    #[test]
    fn an_agent_hook_is_pending_until_approved() {
        let folder = folder();
        let id = hex_id(1);
        agent_file(&folder.store, &id, "2026-10-17T10:00:00Z", json!("global"), Value::Null);

        let listed = folder.store.list(NO_MEMORY);
        let pending = view(&listed.hooks, &id);
        assert_eq!(
            (pending.status, pending.approval),
            (HookStatus::Pending, HookApproval::Never)
        );

        folder.store.approve(&id, &pending.sha256).unwrap();
        assert_eq!(
            view(&folder.store.list(NO_MEMORY).hooks, &id).status,
            HookStatus::Active
        );
    }

    #[test]
    fn status_shows_off_and_waiting_hooks() {
        let folder = folder();

        let mut off = draft("Off");
        off.enabled = false;
        let off = folder.store.save(&off, DESKTOP, NO_MEMORY, at(0)).unwrap();
        assert_eq!(off.status, HookStatus::Off);

        seed_defaults(&folder.store, MEMORY, at(1)).unwrap();

        let without = folder.store.list(NO_MEMORY);
        let seeded: Vec<&HookView> = without.hooks.iter().filter(|view| view.id != off.id).collect();
        assert_eq!(seeded.len(), 4);
        assert!(seeded.iter().all(|view| view.status == HookStatus::Paused));

        let with = folder.store.list(MEMORY);
        assert!(
            with.hooks
                .iter()
                .filter(|view| view.id != off.id)
                .all(|view| view.status == HookStatus::Active)
        );
    }

    #[test]
    fn an_edit_keeps_the_origin_and_unknown_fields() {
        let folder = folder();
        let id = hex_id(7);
        let value = json!({
            "version": 1, "id": id, "name": "Agent hook", "scope": "global", "event": "turnEnd",
            "action": { "type": "followUp", "text": "Check again." }, "enabled": true,
            "requires": ["harness-memory"], "seed": "memory-save",
            "createdAt": "2026-10-01T08:00:00Z", "createdBy": "agent",
            "updatedAt": "2026-10-01T08:00:00Z", "updatedBy": "agent",
            "futureField": { "kept": [1, 2] }
        });
        std::fs::create_dir_all(folder.store.hooks_dir()).unwrap();
        std::fs::write(file_of(&folder.store, &id), serde_json::to_vec(&value).unwrap()).unwrap();

        let listed = folder.store.list(MEMORY);
        let pending = view(&listed.hooks, &id);
        folder.store.approve(&id, &pending.sha256).unwrap();

        let mut edit = edit_of(pending);
        edit.name = "Renamed".to_owned();

        let saved = folder.store.save(&edit, DESKTOP, MEMORY, at(5)).unwrap();
        let hook = saved.hook.clone().unwrap();

        assert_eq!(saved.status, HookStatus::Active);
        assert_eq!(hook.name, "Renamed");
        assert_eq!(hook.created_at, "2026-10-01T08:00:00Z");
        assert_eq!(hook.created_by, HookAuthor::Agent);
        assert_eq!(hook.updated_by, HookAuthor::Desktop);
        assert_ne!(hook.updated_at, "2026-10-01T08:00:00Z");
        assert_eq!(hook.seed, Some(HookSeed::MemorySave));
        assert_eq!(hook.requires, vec![HookRequirement::HarnessMemory]);

        let written: Value = serde_json::from_slice(&std::fs::read(file_of(&folder.store, &id)).unwrap()).unwrap();
        assert_eq!(written["futureField"], json!({ "kept": [1, 2] }));
    }

    #[test]
    fn the_desktop_does_not_save_over_a_pending_hook() {
        let folder = folder();
        let saved = folder
            .store
            .save(&draft("Answer style"), DESKTOP, NO_MEMORY, at(0))
            .unwrap();
        rewrite_same_values(&folder.store, &saved.id);
        let pending = view(&folder.store.list(NO_MEMORY).hooks, &saved.id).clone();

        let mut edit = edit_of(&pending);
        edit.enabled = false;

        let result = folder.store.save(&edit, DESKTOP, NO_MEMORY, at(1));
        assert!(matches!(result, Err(BusinessError::InvalidInput(_))), "{result:?}");

        let after = view(&folder.store.list(NO_MEMORY).hooks, &saved.id).clone();
        assert_eq!(after.sha256, pending.sha256, "the agent's version is still there");
        assert_eq!(after.status, HookStatus::Pending);
    }

    #[test]
    fn an_edit_of_a_changed_file_is_a_conflict() {
        let folder = folder();
        let saved = folder
            .store
            .save(&draft("Answer style"), DESKTOP, NO_MEMORY, at(0))
            .unwrap();
        let edit = edit_of(&saved);

        rewrite_same_values(&folder.store, &saved.id);

        let result = folder.store.save(&edit, DESKTOP, NO_MEMORY, at(1));
        assert!(matches!(result, Err(BusinessError::Conflict(_))), "{result:?}");
    }

    #[test]
    fn a_draft_that_breaks_the_format_is_refused() {
        let folder = folder();

        let mut follow_up_on_prompt = draft("Wrong");
        follow_up_on_prompt.action = HookAction::FollowUp { text: "x".to_owned() };

        let mut matcher = draft("Matcher");
        matcher.matcher = Some("Edit".to_owned());

        let mut blank = draft("   ");
        blank.name = "   ".to_owned();

        for bad in [follow_up_on_prompt, matcher, blank] {
            let result = folder.store.save(&bad, DESKTOP, NO_MEMORY, at(0));
            assert!(matches!(result, Err(BusinessError::InvalidInput(_))), "{result:?}");
        }

        assert!(folder.store.list(NO_MEMORY).hooks.is_empty());
    }

    #[test]
    fn deleting_removes_the_file_and_its_approval() {
        let folder = folder();
        let (saved, bytes) = saved_with_bytes(&folder, "Answer style");

        folder.store.delete(&saved.id, DESKTOP, None).unwrap();
        assert!(folder.store.list(NO_MEMORY).hooks.is_empty());

        // The same bytes written back are not approved any more.
        std::fs::write(file_of(&folder.store, &saved.id), &bytes).unwrap();
        let back = folder.store.list(NO_MEMORY);
        assert_eq!(
            (back.hooks[0].status, back.hooks[0].approval),
            (HookStatus::Pending, HookApproval::Never)
        );

        folder.store.delete(&saved.id, DESKTOP, None).unwrap();
        folder.store.delete(&saved.id, DESKTOP, None).unwrap();
    }

    #[test]
    fn a_file_removed_outside_the_app_loses_its_approval() {
        let folder = folder();
        let kept = folder.store.save(&draft("Kept"), DESKTOP, NO_MEMORY, at(0)).unwrap();
        let removed = folder.store.save(&draft("Removed"), DESKTOP, NO_MEMORY, at(1)).unwrap();
        let bytes = std::fs::read(file_of(&folder.store, &removed.id)).unwrap();

        std::fs::remove_file(file_of(&folder.store, &removed.id)).unwrap();
        assert_eq!(folder.store.prune_approvals().unwrap(), 1);
        assert_eq!(folder.store.prune_approvals().unwrap(), 0);

        // The same bytes written back (an agent restoring its file) are pending again.
        std::fs::write(file_of(&folder.store, &removed.id), &bytes).unwrap();
        let listing = folder.store.list(NO_MEMORY);
        assert_eq!(view(&listing.hooks, &kept.id).status, HookStatus::Active);
        assert_eq!(
            (
                view(&listing.hooks, &removed.id).status,
                view(&listing.hooks, &removed.id).approval
            ),
            (HookStatus::Pending, HookApproval::Never)
        );
        assert!(
            !folder
                .store
                .config_dir()
                .join("hook-approved")
                .join(format!("{}.json", removed.id))
                .exists()
        );
    }

    #[test]
    fn a_missing_folder_keeps_the_approvals() {
        let folder = folder();
        let saved = folder.store.save(&draft("Kept"), DESKTOP, NO_MEMORY, at(0)).unwrap();
        let bytes = std::fs::read(file_of(&folder.store, &saved.id)).unwrap();

        std::fs::rename(folder.store.hooks_dir(), folder.store.root().join("away")).unwrap();
        assert_eq!(folder.store.prune_approvals().unwrap(), 0);

        std::fs::create_dir_all(folder.store.hooks_dir()).unwrap();
        std::fs::write(file_of(&folder.store, &saved.id), &bytes).unwrap();
        assert_eq!(folder.store.list(NO_MEMORY).hooks[0].status, HookStatus::Active);
    }

    #[test]
    fn an_entry_that_is_not_a_readable_file_keeps_its_approval() {
        let folder = folder();
        let saved = folder.store.save(&draft("Kept"), DESKTOP, NO_MEMORY, at(0)).unwrap();
        let path = file_of(&folder.store, &saved.id);
        let bytes = std::fs::read(&path).unwrap();

        // Replaced by a folder for a moment: still an entry with that name.
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert_eq!(folder.store.prune_approvals().unwrap(), 0);

        std::fs::remove_dir(&path).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(
            view(&folder.store.list(NO_MEMORY).hooks, &saved.id).status,
            HookStatus::Active
        );
    }

    #[cfg(unix)]
    #[test]
    fn approvals_that_cannot_be_read_stop_every_app_write() {
        use std::os::unix::fs::PermissionsExt;

        let folder = folder();
        let saved = folder.store.save(&draft("Kept"), DESKTOP, NO_MEMORY, at(0)).unwrap();
        let approvals = folder.store.config_dir().join(APPROVALS_FILE);
        let before = std::fs::read(&approvals).unwrap();

        std::fs::set_permissions(&approvals, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read(&approvals).is_ok() {
            // Running as root: permissions do not stop reading.
            return;
        }

        let added = folder.store.save(&draft("New"), DESKTOP, NO_MEMORY, at(1));
        assert!(added.is_err(), "{added:?}");
        assert!(folder.store.delete(&saved.id, DESKTOP, Some(&saved.sha256)).is_err());
        assert!(file_of(&folder.store, &saved.id).exists(), "nothing was deleted");
        assert!(folder.store.prune_approvals().is_err());

        // Readers see every hook pending meanwhile.
        assert_eq!(folder.store.list(NO_MEMORY).hooks.len(), 1, "no file was added");
        assert_eq!(folder.store.list(NO_MEMORY).hooks[0].status, HookStatus::Pending);

        std::fs::set_permissions(&approvals, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(std::fs::read(&approvals).unwrap(), before, "never saved over");
        assert_eq!(folder.store.list(NO_MEMORY).hooks[0].status, HookStatus::Active);
    }

    #[test]
    fn invalid_approvals_are_set_aside_by_the_next_app_write() {
        let folder = folder();
        let saved = folder.store.save(&draft("Kept"), DESKTOP, NO_MEMORY, at(0)).unwrap();
        let approvals = folder.store.config_dir().join(APPROVALS_FILE);
        std::fs::write(&approvals, b"{ not json").unwrap();

        assert_eq!(folder.store.list(NO_MEMORY).hooks[0].status, HookStatus::Pending);

        let added = folder.store.save(&draft("New"), DESKTOP, NO_MEMORY, at(1)).unwrap();
        assert_eq!(added.status, HookStatus::Active);

        let listing = folder.store.list(NO_MEMORY);
        assert_eq!(view(&listing.hooks, &saved.id).status, HookStatus::Pending);

        let aside: Vec<String> = std::fs::read_dir(folder.store.config_dir())
            .unwrap()
            .filter_map(|entry| entry.unwrap().file_name().into_string().ok())
            .filter(|name| name.starts_with("hook-approvals.json.corrupt-"))
            .collect();
        assert_eq!(aside.len(), 1, "{aside:?}");
        assert_eq!(
            std::fs::read(folder.store.config_dir().join(&aside[0])).unwrap(),
            b"{ not json"
        );
    }

    #[test]
    fn deleting_all_removes_the_listed_files_with_their_approvals() {
        let folder = folder();
        let project = tempfile::tempdir().unwrap();

        let global = folder.store.save(&draft("Global"), DESKTOP, NO_MEMORY, at(0)).unwrap();
        let mut own = draft("Own project");
        own.scope = HookScope::Project;
        own.project = Some(project.path().to_string_lossy().into_owned());
        let own = folder.store.save(&own, DESKTOP, NO_MEMORY, at(1)).unwrap();
        let root = own.hook.as_ref().unwrap().project.clone();

        let other = tempfile::tempdir().unwrap();
        let mut elsewhere = draft("Other project");
        elsewhere.scope = HookScope::Project;
        elsewhere.project = Some(other.path().to_string_lossy().into_owned());
        let elsewhere = folder.store.save(&elsewhere, DESKTOP, NO_MEMORY, at(2)).unwrap();

        let pending = folder
            .store
            .save(&draft("Pending"), HookAuthor::Agent, NO_MEMORY, at(3))
            .unwrap();
        let invalid = hex_id(9);
        std::fs::write(file_of(&folder.store, &invalid), b"{ not json").unwrap();

        assert_eq!(folder.store.delete_all(root.as_deref()).unwrap(), 4);

        let left: Vec<String> = folder
            .store
            .list(NO_MEMORY)
            .hooks
            .into_iter()
            .map(|view| view.id)
            .collect();
        assert_eq!(left, vec![elsewhere.id.clone()]);

        for gone in [&global.id, &own.id, &pending.id, &invalid] {
            assert!(!file_of(&folder.store, gone).exists());
        }

        // The deleted hooks lost their approvals: the same bytes written back are pending.
        let approvals: Value =
            serde_json::from_slice(&std::fs::read(folder.store.config_dir().join(APPROVALS_FILE)).unwrap()).unwrap();
        let approved: Vec<&String> = approvals["approvals"].as_object().unwrap().keys().collect();
        assert_eq!(approved, vec![&elsewhere.id]);
        assert!(
            !folder
                .store
                .config_dir()
                .join("hook-approved")
                .join(format!("{}.json", global.id))
                .exists()
        );

        // Without a project only global hooks (and invalid files) go.
        assert_eq!(folder.store.delete_all(None).unwrap(), 0);
        assert_eq!(folder.store.list(NO_MEMORY).hooks.len(), 1);
    }
}
