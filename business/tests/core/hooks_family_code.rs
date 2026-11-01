//! The "Use the code graph first" family hook: it is Harness Code's. It needs the code graph (the
//! server lists `code-index`, the code graph setting and its Code index switch are on), not Harness
//! Memory. It is seeded and restored only with it, pauses without it, and the dialog offers it only
//! when the server has the feature. A file written while the graph was part of Harness Memory is an
//! earlier version that the update brings to Harness Code.

mod hooks_family_code {
    use harness_business::hooks::{HookStore, apply_family, family_hook, family_status, restore_member, seed_defaults};
    use harness_business::model::{
        HookAction, HookEvent, HookFamilyStatus, HookRequirement, HookSeed, HookStatus, HookView,
    };
    use pretty_assertions::assert_eq;

    use crate::hook_store_support::{CODE_GRAPH, CODE_ONLY, DESKTOP, MEMORY, at, edit_of, folder};

    fn code_hook(store: &HookStore, capabilities: harness_business::hooks::HookCapabilities) -> HookView {
        store
            .list(capabilities)
            .hooks
            .into_iter()
            .find(|view| {
                view.hook
                    .as_ref()
                    .is_some_and(|hook| hook.seed == Some(HookSeed::CodeGraphFirst))
            })
            .unwrap()
    }

    #[test]
    fn the_code_graph_hook_is_seeded_only_where_the_code_graph_is_available() {
        let folder = folder();
        let store = &folder.store;

        let created = seed_defaults(store, MEMORY, at(0)).unwrap();
        assert!(!created.contains(&HookSeed::CodeGraphFirst));
        assert_eq!(store.list(CODE_GRAPH).hooks.len(), 5);

        // Not recorded as seeded: it comes once the code graph is there, and only once.
        assert_eq!(
            seed_defaults(store, CODE_GRAPH, at(1)).unwrap(),
            vec![HookSeed::CodeGraphFirst]
        );
        assert_eq!(seed_defaults(store, CODE_GRAPH, at(2)).unwrap(), vec![]);

        let view = code_hook(store, CODE_GRAPH);
        let hook = view.hook.as_ref().unwrap();
        assert_eq!(view.status, HookStatus::Active);
        assert_eq!(hook.name, "Use the code graph first");
        assert_eq!(hook.event, HookEvent::PromptSubmit);
        assert_eq!(hook.requires, vec![HookRequirement::HarnessCode]);
        assert_eq!(hook.action, family_hook(HookSeed::CodeGraphFirst).action());
    }

    #[test]
    fn harness_code_alone_is_enough_for_its_hook() {
        let folder = folder();
        let store = &folder.store;

        assert_eq!(
            seed_defaults(store, CODE_ONLY, at(0)).unwrap(),
            vec![HookSeed::CodeGraphFirst]
        );
        assert_eq!(code_hook(store, CODE_ONLY).status, HookStatus::Active);
        assert_eq!(
            code_hook(store, MEMORY).status,
            HookStatus::Paused,
            "Harness Memory without Harness Code does not run it"
        );
    }

    #[test]
    fn the_code_graph_hook_pauses_while_the_code_graph_is_off_and_runs_again_with_it() {
        let folder = folder();
        let store = &folder.store;
        seed_defaults(store, CODE_GRAPH, at(0)).unwrap();

        assert_eq!(code_hook(store, CODE_GRAPH).status, HookStatus::Active);
        assert_eq!(code_hook(store, MEMORY).status, HookStatus::Paused);
        assert_eq!(code_hook(store, CODE_GRAPH).status, HookStatus::Active);

        // The other memory hooks do not care.
        let memory_hooks = store.list(MEMORY).hooks;
        assert_eq!(
            memory_hooks
                .iter()
                .filter(|view| view.status == HookStatus::Active)
                .count(),
            5
        );
    }

    #[test]
    fn the_dialog_offers_the_code_graph_hook_once_the_server_has_the_code_graph() {
        let folder = folder();
        let store = &folder.store;

        let status = |installed| {
            family_status(&store.list(installed), installed)
                .into_iter()
                .find(|entry| entry.seed == HookSeed::CodeGraphFirst)
                .unwrap()
                .status
        };

        assert_eq!(status(MEMORY), HookFamilyStatus::NeedsMember);
        assert_eq!(status(CODE_GRAPH), HookFamilyStatus::Missing);

        apply_family(store, &[HookSeed::CodeGraphFirst], CODE_GRAPH, at(1)).unwrap();
        assert_eq!(status(CODE_GRAPH), HookFamilyStatus::Installed);
    }

    #[test]
    fn installing_harness_code_adds_the_code_graph_hook_only_with_the_code_graph() {
        let folder = folder();
        let store = &folder.store;
        seed_defaults(store, MEMORY, at(0)).unwrap();

        assert_eq!(
            restore_member(store, HookRequirement::HarnessCode, MEMORY, at(1)).unwrap(),
            vec![]
        );
        assert_eq!(
            restore_member(store, HookRequirement::HarnessCode, CODE_GRAPH, at(2)).unwrap(),
            vec![HookSeed::CodeGraphFirst]
        );
        assert_eq!(
            restore_member(store, HookRequirement::HarnessMemory, CODE_GRAPH, at(3)).unwrap(),
            vec![],
            "Harness Memory no longer brings the code graph hook"
        );
    }

    #[test]
    fn a_hook_from_when_the_graph_was_in_harness_memory_is_stale_and_moves_to_harness_code() {
        let folder = folder();
        let store = &folder.store;
        seed_defaults(store, CODE_GRAPH, at(0)).unwrap();

        let entry = family_hook(HookSeed::CodeGraphFirst);
        let before_move = entry.earlier[0];
        assert!(
            before_move.contains("Harness Memory code tool"),
            "the text the graph had in memory"
        );

        let mut draft = edit_of(&code_hook(store, CODE_GRAPH));
        draft.action = HookAction::AddContext {
            text: before_move.to_owned(),
        };
        store.save(&draft, DESKTOP, CODE_GRAPH, at(1)).unwrap();

        let found = |installed| {
            family_status(&store.list(installed), installed)
                .into_iter()
                .find(|found| found.seed == HookSeed::CodeGraphFirst)
                .unwrap()
        };
        assert_eq!(
            (found(CODE_GRAPH).status, found(CODE_GRAPH).stale),
            (HookFamilyStatus::Outdated, true)
        );

        apply_family(store, &[HookSeed::CodeGraphFirst], CODE_GRAPH, at(2)).unwrap();

        let updated = code_hook(store, CODE_GRAPH).hook.unwrap();
        assert_eq!(found(CODE_GRAPH).status, HookFamilyStatus::Installed);
        assert_eq!(updated.requires, vec![HookRequirement::HarnessCode]);
        assert_eq!(updated.action, entry.action());
    }
}
