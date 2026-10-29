//! The "Use the code graph first" family hook: it needs the code graph of Harness Memory (the
//! server lists `code-index` and its Code index switch is on) on top of Harness Memory itself. It
//! is seeded and restored only with it, pauses without it, and the dialog offers it only when the
//! server has the feature.

mod hooks_family_code_graph {
    use harness_business::hooks::{apply_family, family_hook, family_status, restore_member, seed_defaults};
    use harness_business::model::{HookEvent, HookFamilyStatus, HookRequirement, HookSeed, HookStatus};
    use pretty_assertions::assert_eq;

    use crate::hook_store_support::{CODE_GRAPH, MEMORY, at, folder};

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

        let view = store
            .list(CODE_GRAPH)
            .hooks
            .into_iter()
            .find(|view| {
                view.hook
                    .as_ref()
                    .is_some_and(|hook| hook.seed == Some(HookSeed::CodeGraphFirst))
            })
            .unwrap();
        let hook = view.hook.as_ref().unwrap();
        assert_eq!(view.status, HookStatus::Active);
        assert_eq!(hook.name, "Use the code graph first");
        assert_eq!(hook.event, HookEvent::PromptSubmit);
        assert_eq!(hook.requires, vec![HookRequirement::HarnessMemory]);
        assert_eq!(hook.action, family_hook(HookSeed::CodeGraphFirst).action());
    }

    #[test]
    fn the_code_graph_hook_pauses_while_the_code_graph_is_off_and_runs_again_with_it() {
        let folder = folder();
        let store = &folder.store;
        seed_defaults(store, CODE_GRAPH, at(0)).unwrap();

        let status = |capabilities| {
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
                .status
        };

        assert_eq!(status(CODE_GRAPH), HookStatus::Active);
        assert_eq!(status(MEMORY), HookStatus::Paused);
        assert_eq!(status(CODE_GRAPH), HookStatus::Active);

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
    fn restoring_memory_adds_the_code_graph_hook_only_with_the_code_graph() {
        let folder = folder();
        let store = &folder.store;
        seed_defaults(store, MEMORY, at(0)).unwrap();

        assert_eq!(
            restore_member(store, HookRequirement::HarnessMemory, MEMORY, at(1)).unwrap(),
            vec![]
        );
        assert_eq!(
            restore_member(store, HookRequirement::HarnessMemory, CODE_GRAPH, at(2)).unwrap(),
            vec![HookSeed::CodeGraphFirst]
        );
    }
}
