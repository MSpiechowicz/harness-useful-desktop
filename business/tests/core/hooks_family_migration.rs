//! Updating the Harness family hooks: every untouched file from any earlier catalog version (its
//! old name, event and text) is rewritten in place by the automatic update, with its id and its
//! on/off state kept. An untouched file of a retired hook is deleted. An edited file is never
//! overwritten.

mod hooks_family_migration {
    use std::iter::once;

    use harness_business::hooks::{
        FAMILY, FamilyHook, HookCapabilities, HookStore, apply_family, family_status, update_stale_family,
    };
    use harness_business::model::{
        HookAction, HookApproval, HookEvent, HookFamilyStatus, HookRequirement, HookSeed, HookView,
    };
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use crate::hook_store_support::{Folder, at, file_of, folder, hex_id};

    const ALL: HookCapabilities = HookCapabilities {
        memory_available: true,
        dashboard_available: true,
        code_graph_available: true,
        knowledge_available: true,
        skills_available: true,
    };

    /// The catalog texts of `entry`, the earlier ones then the current one.
    fn versions(entry: &FamilyHook) -> Vec<&'static str> {
        entry.earlier.iter().copied().chain(once(entry.text)).collect()
    }

    /// The most versions any entry has.
    fn most_versions() -> usize {
        FAMILY.iter().map(|entry| versions(entry).len()).max().unwrap()
    }

    /// The id of the file of `entry` in the folders below.
    fn id_of(entry: &FamilyHook) -> String {
        let index = FAMILY.iter().position(|other| other.seed == entry.seed).unwrap();

        hex_id(index + 1)
    }

    /// A folder with one approved file per catalog entry, as an earlier release wrote it: the
    /// `version`th text of the entry (the last one when it has fewer) under its old name and event.
    /// The Git branch hook is switched off and the Harness Knowledge one is the user's own edit.
    fn earlier_folder(version: usize) -> Folder {
        let folder = folder();
        std::fs::create_dir_all(folder.store.hooks_dir()).unwrap();

        for entry in FAMILY {
            let texts = versions(&entry);
            let mut text = texts[version.min(texts.len() - 1)];

            if entry.seed == HookSeed::KnowledgeFirst {
                text = "My own rule for documentation.";
            }

            let event = entry.earlier_events.first().copied().unwrap_or(entry.event);
            let requires = match entry.seed {
                HookSeed::UsefulSkills | HookSeed::CodeGraphFirst => HookRequirement::HarnessMemory,
                _ => entry.member,
            };

            let kind = if event == HookEvent::TurnEnd {
                "followUp"
            } else {
                "addContext"
            };
            let file = json!({
                "version": 1, "id": id_of(&entry),
                "name": entry.earlier_names.first().copied().unwrap_or(entry.name),
                "description": "Seeded default. An older description.",
                "scope": "global", "project": null, "event": event.as_str(), "matcher": null,
                "action": { "type": kind, "text": text },
                "enabled": entry.seed != HookSeed::BranchSync,
                "createdAt": "2026-09-01T10:00:00Z", "createdBy": "desktop",
                "updatedAt": "2026-09-01T10:00:00Z", "updatedBy": "desktop",
                "requires": [requires.as_str()], "seed": entry.seed.as_str(),
            });

            std::fs::write(
                file_of(&folder.store, &id_of(&entry)),
                serde_json::to_vec_pretty(&file).unwrap(),
            )
            .unwrap();
        }

        for view in folder.store.list(ALL).hooks {
            folder.store.approve(&view.id, &view.sha256).unwrap();
        }

        folder
    }

    fn file(store: &HookStore, entry: &FamilyHook) -> Option<HookView> {
        store.list(ALL).hooks.into_iter().find(|view| view.id == id_of(entry))
    }

    /// What the app's automatic update asks for: every stale entry.
    fn stale_seeds(store: &HookStore) -> Vec<HookSeed> {
        family_status(&store.list(ALL), ALL)
            .into_iter()
            .filter(|entry| entry.stale)
            .map(|entry| entry.seed)
            .collect()
    }

    #[test]
    fn every_historic_version_is_stale_and_migrates_in_place() {
        for version in 0..most_versions() {
            let folder = earlier_folder(version);
            let store = &folder.store;

            let edited = FAMILY
                .iter()
                .find(|entry| entry.seed == HookSeed::KnowledgeFirst)
                .unwrap();
            let edited_before = file(store, edited).unwrap().sha256;

            let stale = stale_seeds(store);
            let expected: Vec<HookSeed> = FAMILY
                .iter()
                .map(|entry| entry.seed)
                .filter(|seed| *seed != HookSeed::KnowledgeFirst)
                .collect();
            assert_eq!(stale, expected, "version {version}");

            apply_family(store, &stale, ALL, at(10)).unwrap();

            for entry in FAMILY
                .iter()
                .filter(|entry| !entry.retired && entry.seed != HookSeed::KnowledgeFirst)
            {
                let view = file(store, entry).unwrap_or_else(|| panic!("{:?} lost its file", entry.seed));
                let hook = view.hook.as_ref().unwrap();

                assert_eq!(hook.name, entry.name, "{:?} v{version}", entry.seed);
                assert_eq!(hook.event, entry.event, "{:?} v{version}", entry.seed);
                assert_eq!(hook.action, entry.action(), "{:?} v{version}", entry.seed);
                assert_eq!(hook.requires, vec![entry.member], "{:?} v{version}", entry.seed);
                assert_eq!(hook.description.as_deref(), Some(entry.description));
                assert_eq!(hook.created_at, "2026-09-01T10:00:00Z", "the creation date is kept");
                assert_eq!(view.approval, HookApproval::Approved);
                assert_eq!(
                    hook.enabled,
                    entry.seed != HookSeed::BranchSync,
                    "{:?}: the switch stays as the user left it",
                    entry.seed
                );
            }

            // Retired hooks are gone.
            for entry in FAMILY.iter().filter(|entry| entry.retired) {
                assert!(file(store, entry).is_none(), "{:?} v{version}", entry.seed);
            }

            // The user's edit is untouched, outdated, and still the hook of its step.
            let kept = file(store, edited).unwrap();
            assert_eq!(kept.sha256, edited_before);

            let rows = family_status(&store.list(ALL), ALL);
            let row = rows.iter().find(|row| row.seed == HookSeed::KnowledgeFirst).unwrap();
            assert_eq!(
                (row.status, row.stale, row.step),
                (HookFamilyStatus::Outdated, false, Some(50))
            );
            assert!(
                rows.iter().all(|row| !row.retired && (!row.stale)),
                "nothing is left to update"
            );
            assert_eq!(stale_seeds(store), vec![], "a second run changes nothing");
        }
    }

    #[test]
    fn untouched_retired_files_are_listed_stale_until_the_update_deletes_them() {
        let folder = earlier_folder(0);
        let store = &folder.store;

        let rows: Vec<_> = family_status(&store.list(ALL), ALL)
            .into_iter()
            .filter(|row| row.retired)
            .map(|row| (row.seed, row.status, row.stale, row.step))
            .collect();

        assert_eq!(
            rows,
            vec![
                (HookSeed::MemorySearch, HookFamilyStatus::Outdated, true, None),
                (HookSeed::KnowledgeRecall, HookFamilyStatus::Outdated, true, None),
            ]
        );

        let changed = apply_family(store, &[HookSeed::MemorySearch, HookSeed::KnowledgeRecall], ALL, at(1)).unwrap();
        assert_eq!(changed, vec![HookSeed::MemorySearch, HookSeed::KnowledgeRecall]);
        assert_eq!(
            apply_family(store, &[HookSeed::MemorySearch], ALL, at(2)).unwrap(),
            vec![]
        );
    }

    #[test]
    fn an_edited_retired_file_is_left_alone_and_is_no_catalog_entry() {
        let folder = earlier_folder(0);
        let store = &folder.store;
        let entry = FAMILY
            .iter()
            .find(|entry| entry.seed == HookSeed::MemorySearch)
            .unwrap();

        let mut view = file(store, entry).unwrap();
        let mut draft = crate::hook_store_support::edit_of(&view);
        draft.action = HookAction::AddContext {
            text: "Search the notes my way.".to_owned(),
        };
        view = store
            .save(&draft, crate::hook_store_support::DESKTOP, ALL, at(1))
            .unwrap();

        assert!(
            family_status(&store.list(ALL), ALL)
                .iter()
                .all(|row| row.seed != HookSeed::MemorySearch)
        );
        assert_eq!(
            apply_family(store, &[HookSeed::MemorySearch], ALL, at(2)).unwrap(),
            vec![]
        );
        assert_eq!(file(store, entry).unwrap().sha256, view.sha256);
    }

    /// Sets each of `changes` in the file of `entry` (the user changed it by hand) and approves it
    /// again.
    fn change_by_hand(store: &HookStore, entry: &FamilyHook, changes: &[(&str, serde_json::Value)]) {
        let path = file_of(store, &id_of(entry));
        let mut content: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();

        for (key, value) in changes {
            content[*key] = value.clone();
        }

        std::fs::write(&path, serde_json::to_vec_pretty(&content).unwrap()).unwrap();

        let view = file(store, entry).unwrap();
        store.approve(&view.id, &view.sha256).unwrap();
    }

    /// The user moves the file to a project: it no longer has the catalog's shape. (A matcher is
    /// no option here: the store only accepts one on a `toolEnd` hook.)
    fn moved_to_a_project(folder: &Folder) -> Vec<(&'static str, serde_json::Value)> {
        vec![
            ("scope", json!("project")),
            ("project", json!(folder.store.hooks_dir().to_string_lossy())),
        ]
    }

    #[test]
    fn a_retired_file_moved_to_a_project_survives_the_update() {
        let entry = FAMILY
            .iter()
            .find(|entry| entry.seed == HookSeed::MemorySearch)
            .unwrap();

        let folder = earlier_folder(0);
        let store = &folder.store;

        change_by_hand(store, entry, &moved_to_a_project(&folder));
        let before = file(store, entry).unwrap();

        assert!(
            family_status(&store.list(ALL), ALL)
                .iter()
                .all(|row| row.seed != HookSeed::MemorySearch),
            "not the catalog's file any more"
        );
        assert_eq!(
            apply_family(store, &[HookSeed::MemorySearch], ALL, at(2)).unwrap(),
            vec![]
        );
        assert_eq!(file(store, entry).unwrap().sha256, before.sha256);
    }

    #[test]
    fn a_stale_file_moved_to_a_project_is_outdated_and_never_rewritten_automatically() {
        let entry = FAMILY.iter().find(|entry| entry.seed == HookSeed::MemoryFirst).unwrap();

        let folder = earlier_folder(0);
        let store = &folder.store;

        change_by_hand(store, entry, &moved_to_a_project(&folder));
        let before = file(store, entry).unwrap();

        let row = family_status(&store.list(ALL), ALL)
            .into_iter()
            .find(|row| row.seed == HookSeed::MemoryFirst)
            .unwrap();
        assert_eq!((row.status, row.stale), (HookFamilyStatus::Outdated, false));

        update_stale_family(store, ALL, ALL, at(3)).unwrap();
        assert_eq!(file(store, entry).unwrap().sha256, before.sha256);
    }

    #[test]
    fn the_automatic_update_rewrites_what_is_stale_in_the_files_as_they_are_now() {
        let folder = earlier_folder(0);
        let store = &folder.store;

        // The user edits one stale file after the dialog last looked.
        let memory = FAMILY.iter().find(|entry| entry.seed == HookSeed::MemoryFirst).unwrap();
        assert!(stale_seeds(store).contains(&HookSeed::MemoryFirst));
        change_by_hand(
            store,
            memory,
            &[("action", json!({ "type": "addContext", "text": "My own rule." }))],
        );
        let edited = file(store, memory).unwrap().sha256;

        let changed = update_stale_family(store, ALL, ALL, at(4)).unwrap();

        assert!(!changed.contains(&HookSeed::MemoryFirst));
        assert!(!changed.contains(&HookSeed::KnowledgeFirst));
        assert!(changed.contains(&HookSeed::MemorySearch), "a retired file is deleted");
        assert_eq!(file(store, memory).unwrap().sha256, edited);
        assert_eq!(stale_seeds(store), vec![]);
        assert_eq!(update_stale_family(store, ALL, ALL, at(5)).unwrap(), vec![]);
    }

    #[test]
    fn the_automatic_update_switches_on_a_family_hook_the_user_had_off_and_the_dialog_does_not() {
        let branch = FAMILY.iter().find(|entry| entry.seed == HookSeed::BranchSync).unwrap();

        // The folder holds Git branch switched off, on an earlier version.
        let automatic = earlier_folder(0);
        assert!(!file(&automatic.store, branch).unwrap().hook.unwrap().enabled);

        update_stale_family(&automatic.store, ALL, ALL, at(4)).unwrap();

        assert!(file(&automatic.store, branch).unwrap().hook.unwrap().enabled);

        let asked = earlier_folder(0);
        apply_family(&asked.store, &[HookSeed::BranchSync], ALL, at(4)).unwrap();

        assert!(!file(&asked.store, branch).unwrap().hook.unwrap().enabled);
    }

    #[test]
    fn retired_hooks_are_never_seeded_or_restored() {
        let folder = folder();
        let store = &folder.store;

        let seeded = harness_business::hooks::seed_defaults(store, ALL, at(0)).unwrap();
        assert!(!seeded.contains(&HookSeed::MemorySearch) && !seeded.contains(&HookSeed::KnowledgeRecall));

        for member in [HookRequirement::HarnessMemory, HookRequirement::HarnessKnowledge] {
            let restored = harness_business::hooks::restore_member(store, member, ALL, at(1)).unwrap();
            assert!(!restored.contains(&HookSeed::MemorySearch) && !restored.contains(&HookSeed::KnowledgeRecall));
        }

        assert!(family_status(&store.list(ALL), ALL).iter().all(|row| !row.retired));
    }
}
