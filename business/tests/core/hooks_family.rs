//! Harness family hooks: launch seeding per available member, the dialog's status against the
//! catalog, adding missing and updating outdated hooks on request, and restoring a member's
//! missing hooks after it is installed without overwriting existing ones.

mod hooks_family {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use harness_business::hooks::{
        HookCapabilities, HookStore, apply_family, family_hook, family_status, restore_member, seed_defaults,
    };
    use harness_business::model::{
        HookAction, HookApproval, HookAuthor, HookDraft, HookFamilyStatus, HookRequirement, HookSeed, HookStatus,
        HookView,
    };
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    const NOTHING: HookCapabilities = HookCapabilities {
        memory_available: false,
        dashboard_available: false,
    };
    const MEMORY: HookCapabilities = HookCapabilities {
        memory_available: true,
        dashboard_available: false,
    };
    const BOTH: HookCapabilities = HookCapabilities {
        memory_available: true,
        dashboard_available: true,
    };

    fn store() -> (TempDir, HookStore) {
        let temp = tempfile::tempdir().unwrap();
        let store = HookStore::new(temp.path().join("harness-hooks"), temp.path().join("config"));

        (temp, store)
    }

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_791_000_000 + secs)
    }

    fn statuses(store: &HookStore, installed: HookCapabilities) -> Vec<(HookSeed, HookFamilyStatus)> {
        family_status(&store.list(installed), installed)
            .into_iter()
            .map(|entry| (entry.seed, entry.status))
            .collect()
    }

    fn seeded(store: &HookStore, seed: HookSeed) -> Option<HookView> {
        store
            .list(BOTH)
            .hooks
            .into_iter()
            .find(|view| view.hook.as_ref().is_some_and(|hook| hook.seed == Some(seed)))
    }

    /// The user deletes the family hook `seed` in the app.
    fn delete_seed(store: &HookStore, seed: HookSeed) {
        let view = seeded(store, seed).unwrap();
        store.delete(&view.id, HookAuthor::Desktop, None).unwrap();
    }

    /// An agent's rewrite of the family hook `seed` with another text (pending).
    fn agent_edit(store: &HookStore, seed: HookSeed) -> HookView {
        edit(store, seed, HookAuthor::Agent)
    }

    /// The user's own rewrite of the family hook `seed` with another text (approved).
    fn user_edit(store: &HookStore, seed: HookSeed) -> HookView {
        edit(store, seed, HookAuthor::Desktop)
    }

    fn edit(store: &HookStore, seed: HookSeed, author: HookAuthor) -> HookView {
        edit_text(store, seed, author, "Something else.")
    }

    fn edit_text(store: &HookStore, seed: HookSeed, author: HookAuthor, text: &str) -> HookView {
        let view = seeded(store, seed).unwrap();
        let hook = view.hook.clone().unwrap();
        let draft = HookDraft {
            id: Some(hook.id),
            expected_sha256: Some(view.sha256),
            name: hook.name,
            description: hook.description,
            scope: hook.scope,
            project: hook.project,
            event: hook.event,
            matcher: hook.matcher,
            action: match hook.action {
                HookAction::FollowUp { .. } => HookAction::FollowUp { text: text.to_owned() },
                _ => HookAction::AddContext { text: text.to_owned() },
            },
            enabled: hook.enabled,
        };

        store.save(&draft, author, BOTH, at(5)).unwrap()
    }

    #[test]
    fn launch_seeding_creates_the_hooks_of_available_members_only() {
        let (_temp, store) = store();

        assert_eq!(seed_defaults(&store, NOTHING, at(0)).unwrap(), vec![]);
        assert_eq!(
            seed_defaults(&store, MEMORY, at(0)).unwrap(),
            vec![
                HookSeed::MemorySearch,
                HookSeed::MemorySave,
                HookSeed::UsefulSkills,
                HookSeed::BranchSync
            ]
        );
        assert_eq!(
            seed_defaults(&store, BOTH, at(1)).unwrap(),
            vec![HookSeed::DashboardLimits]
        );

        let limits = seeded(&store, HookSeed::DashboardLimits).unwrap();
        let hook = limits.hook.as_ref().unwrap();
        assert_eq!(limits.status, HookStatus::Active);
        assert_eq!(hook.requires, vec![HookRequirement::HarnessDashboard]);
        assert_eq!(hook.name, "Check limits before long work");

        assert_eq!(
            store
                .list(MEMORY)
                .hooks
                .iter()
                .filter(|view| view.status == HookStatus::Paused)
                .count(),
            1
        );
    }

    #[test]
    fn the_status_compares_the_folder_with_the_catalog() {
        let (_temp, store) = store();

        assert_eq!(
            statuses(&store, MEMORY),
            vec![
                (HookSeed::MemorySearch, HookFamilyStatus::Missing),
                (HookSeed::MemorySave, HookFamilyStatus::Missing),
                (HookSeed::DashboardLimits, HookFamilyStatus::NeedsMember),
                (HookSeed::UsefulSkills, HookFamilyStatus::Missing),
                (HookSeed::BranchSync, HookFamilyStatus::Missing),
            ]
        );

        seed_defaults(&store, MEMORY, at(0)).unwrap();
        user_edit(&store, HookSeed::MemorySearch);

        assert_eq!(
            statuses(&store, MEMORY),
            vec![
                (HookSeed::MemorySearch, HookFamilyStatus::Outdated),
                (HookSeed::MemorySave, HookFamilyStatus::Installed),
                (HookSeed::DashboardLimits, HookFamilyStatus::NeedsMember),
                (HookSeed::UsefulSkills, HookFamilyStatus::Installed),
                (HookSeed::BranchSync, HookFamilyStatus::Installed),
            ]
        );

        // A pending edit is not the user's hook yet: the seed counts as missing.
        agent_edit(&store, HookSeed::MemorySave);

        let entries = family_status(&store.list(MEMORY), MEMORY);
        assert_eq!(entries[1].status, HookFamilyStatus::Missing);
        assert_eq!(entries[1].hook_id, None);
        assert_eq!(
            entries[0].hook_id,
            seeded(&store, HookSeed::MemorySearch).map(|view| view.id)
        );
        assert_eq!(entries[2].hook_id, None);
    }

    #[test]
    fn only_an_earlier_catalog_version_word_for_word_is_stale() {
        let (_temp, store) = store();
        seed_defaults(&store, MEMORY, at(0)).unwrap();
        let earlier = family_hook(HookSeed::MemorySearch).earlier[0];
        edit_text(&store, HookSeed::MemorySearch, HookAuthor::Desktop, earlier);
        user_edit(&store, HookSeed::MemorySave);

        let entries = family_status(&store.list(MEMORY), MEMORY);

        assert_eq!(
            (entries[0].status, entries[0].stale),
            (HookFamilyStatus::Outdated, true)
        );
        assert_eq!(
            (entries[1].status, entries[1].stale),
            (HookFamilyStatus::Outdated, false)
        );
        assert!(!entries[2].stale);
    }

    #[test]
    fn the_previous_save_and_search_texts_are_earlier_versions_and_the_current_ones_are_not() {
        let (_temp, store) = store();
        seed_defaults(&store, MEMORY, at(0)).unwrap();

        let entries = family_status(&store.list(MEMORY), MEMORY);
        assert!(
            entries.iter().all(|entry| !entry.stale),
            "the seeded texts are the current ones"
        );

        for seed in [HookSeed::MemorySave, HookSeed::MemorySearch] {
            let entry = family_hook(seed);
            assert_ne!(
                entry.earlier[0], entry.text,
                "the first earlier text is the one before this"
            );

            edit_text(&store, seed, HookAuthor::Desktop, entry.earlier[0]);
            let entries = family_status(&store.list(MEMORY), MEMORY);
            let found = entries.iter().find(|found| found.seed == seed).unwrap();

            assert_eq!(
                (found.status, found.stale),
                (HookFamilyStatus::Outdated, true),
                "{seed:?}"
            );
        }
    }

    #[test]
    fn a_pending_file_with_a_seed_is_replaced_by_the_approved_catalog_version() {
        let (_temp, store) = store();
        seed_defaults(&store, MEMORY, at(0)).unwrap();
        let pending = agent_edit(&store, HookSeed::MemorySearch);
        assert_eq!(pending.status, HookStatus::Pending);

        assert_eq!(
            restore_member(&store, HookRequirement::HarnessMemory, at(2)).unwrap(),
            vec![HookSeed::MemorySearch]
        );

        let search = seeded(&store, HookSeed::MemorySearch).unwrap();
        assert_eq!(search.id, pending.id);
        assert_eq!(
            (search.status, search.approval),
            (HookStatus::Active, HookApproval::Approved)
        );
        assert_eq!(
            search.hook.unwrap().action,
            family_hook(HookSeed::MemorySearch).action()
        );
        assert_eq!(store.list(MEMORY).hooks.len(), 4, "replaced, not duplicated");
    }

    #[test]
    fn applying_adds_missing_and_updates_outdated_hooks_as_approved_desktop_writes() {
        let (_temp, store) = store();
        seed_defaults(&store, MEMORY, at(0)).unwrap();
        let before = user_edit(&store, HookSeed::MemorySearch);

        let changed = apply_family(
            &store,
            &[HookSeed::MemorySearch, HookSeed::DashboardLimits],
            BOTH,
            at(9),
        )
        .unwrap();
        assert_eq!(changed, vec![HookSeed::MemorySearch, HookSeed::DashboardLimits]);

        let search = seeded(&store, HookSeed::MemorySearch).unwrap();
        let hook = search.hook.as_ref().unwrap();
        assert_eq!(
            (search.status, search.approval),
            (HookStatus::Active, HookApproval::Approved)
        );
        assert_eq!(search.id, before.id);
        assert_eq!(hook.action, family_hook(HookSeed::MemorySearch).action());
        assert_eq!(hook.created_at, before.hook.as_ref().unwrap().created_at);
        assert_eq!(
            (hook.created_by, hook.updated_by),
            (HookAuthor::Desktop, HookAuthor::Desktop)
        );

        assert_eq!(
            seeded(&store, HookSeed::DashboardLimits).unwrap().status,
            HookStatus::Active
        );
        assert!(
            statuses(&store, BOTH)
                .iter()
                .all(|(_, status)| *status == HookFamilyStatus::Installed)
        );

        assert_eq!(
            apply_family(&store, &[HookSeed::MemorySearch], BOTH, at(10)).unwrap(),
            vec![]
        );
    }

    #[test]
    fn restoring_a_member_adds_its_missing_hooks_and_overwrites_nothing() {
        let (_temp, store) = store();
        seed_defaults(&store, MEMORY, at(0)).unwrap();

        delete_seed(&store, HookSeed::MemorySave);
        let edited = user_edit(&store, HookSeed::MemorySearch);

        // Deleted hooks are not seeded again at launch.
        assert_eq!(seed_defaults(&store, MEMORY, at(1)).unwrap(), vec![]);
        assert_eq!(seeded(&store, HookSeed::MemorySave), None);

        assert_eq!(
            restore_member(&store, HookRequirement::HarnessMemory, at(2)).unwrap(),
            vec![HookSeed::MemorySave]
        );
        assert_eq!(seeded(&store, HookSeed::MemorySave).unwrap().status, HookStatus::Active);
        assert_eq!(seeded(&store, HookSeed::MemorySearch).unwrap().sha256, edited.sha256);

        assert_eq!(
            restore_member(&store, HookRequirement::HarnessDashboard, at(3)).unwrap(),
            vec![HookSeed::DashboardLimits]
        );

        // A hook deleted after a restore stays deleted at launch.
        delete_seed(&store, HookSeed::MemorySave);
        assert_eq!(seed_defaults(&store, BOTH, at(4)).unwrap(), vec![]);
        assert_eq!(seeded(&store, HookSeed::MemorySave), None);
    }

    #[test]
    fn an_invalid_seed_record_is_set_aside_and_deleted_hooks_stay_deleted() {
        let (_temp, store) = store();
        seed_defaults(&store, MEMORY, at(0)).unwrap();

        delete_seed(&store, HookSeed::MemorySave);

        let record = store.config_dir().join("hooks-state.json");
        std::fs::write(&record, b"{ broken").unwrap();

        assert_eq!(seed_defaults(&store, BOTH, at(1)).unwrap(), vec![]);
        assert_eq!(seeded(&store, HookSeed::MemorySave), None);

        let rebuilt: serde_json::Value = serde_json::from_slice(&std::fs::read(&record).unwrap()).unwrap();
        assert_eq!(rebuilt["version"], 1);

        let aside = std::fs::read_dir(store.config_dir())
            .unwrap()
            .filter_map(|entry| entry.unwrap().file_name().into_string().ok())
            .filter(|name| name.starts_with("hooks-state.json.corrupt-"))
            .count();
        assert_eq!(aside, 1);
    }

    #[test]
    fn family_hooks_deleted_all_at_once_stay_deleted_at_launch() {
        let (_temp, store) = store();
        seed_defaults(&store, BOTH, at(0)).unwrap();

        assert_eq!(store.delete_all(None).unwrap(), 5);
        assert_eq!(seed_defaults(&store, BOTH, at(1)).unwrap(), vec![]);
        assert!(store.list(BOTH).hooks.is_empty());
    }
}
