//! The Harness Knowledge family hooks ("Check Knowledge first" and "Recall from Harness
//! Knowledge"): both need the member (installed, its setting on, `mcp` listed), are seeded and
//! restored with it, pause without it, and the dialog offers them only with it.

mod hooks_family_knowledge {
    use harness_business::hooks::{apply_family, family_hook, family_status, restore_member, seed_defaults};
    use harness_business::model::{HookEvent, HookFamilyStatus, HookRequirement, HookSeed, HookStatus, HookView};
    use pretty_assertions::assert_eq;

    use crate::hook_store_support::{DESKTOP, KNOWLEDGE, MEMORY, at, folder};

    const BOTH: [HookSeed; 2] = [HookSeed::KnowledgeFirst, HookSeed::KnowledgeRecall];

    fn seeded(views: Vec<HookView>, seed: HookSeed) -> HookView {
        views
            .into_iter()
            .find(|view| view.hook.as_ref().is_some_and(|hook| hook.seed == Some(seed)))
            .expect("a hook with the seed")
    }

    #[test]
    fn the_knowledge_hooks_are_seeded_once_and_only_where_knowledge_is_available() {
        let folder = folder();
        let store = &folder.store;

        // Harness Memory alone brings none of them, and they are not recorded as seeded.
        let created = seed_defaults(store, MEMORY, at(0)).unwrap();
        assert!(BOTH.iter().all(|seed| !created.contains(seed)));

        assert_eq!(seed_defaults(store, KNOWLEDGE, at(1)).unwrap(), BOTH.to_vec());
        assert_eq!(seed_defaults(store, KNOWLEDGE, at(2)).unwrap(), vec![]);

        let first = seeded(store.list(KNOWLEDGE).hooks, HookSeed::KnowledgeFirst);
        let hook = first.hook.as_ref().unwrap();
        assert_eq!(first.status, HookStatus::Active);
        assert_eq!(hook.name, "Check Knowledge first");
        assert_eq!(hook.event, HookEvent::PromptSubmit);
        assert_eq!(hook.requires, vec![HookRequirement::HarnessKnowledge]);
        assert_eq!(hook.action, family_hook(HookSeed::KnowledgeFirst).action());

        let recall = seeded(store.list(KNOWLEDGE).hooks, HookSeed::KnowledgeRecall);
        let hook = recall.hook.as_ref().unwrap();
        assert_eq!(recall.status, HookStatus::Active);
        assert!(hook.enabled);
        assert_eq!(hook.event, HookEvent::PromptSubmit);
        assert_eq!(hook.requires, vec![HookRequirement::HarnessKnowledge]);
    }

    #[test]
    fn the_knowledge_hooks_pause_while_knowledge_is_unavailable() {
        let folder = folder();
        let store = &folder.store;
        seed_defaults(store, KNOWLEDGE, at(0)).unwrap();

        for seed in BOTH {
            assert_eq!(seeded(store.list(KNOWLEDGE).hooks, seed).status, HookStatus::Active);
            assert_eq!(seeded(store.list(MEMORY).hooks, seed).status, HookStatus::Paused);
            assert_eq!(seeded(store.list(KNOWLEDGE).hooks, seed).status, HookStatus::Active);
        }
    }

    #[test]
    fn the_dialog_offers_the_knowledge_hooks_with_knowledge_and_adds_them_on_request() {
        let folder = folder();
        let store = &folder.store;

        let statuses = |installed| -> Vec<HookFamilyStatus> {
            family_status(&store.list(installed), installed)
                .into_iter()
                .filter(|entry| BOTH.contains(&entry.seed))
                .map(|entry| entry.status)
                .collect()
        };

        assert_eq!(statuses(MEMORY), vec![HookFamilyStatus::NeedsMember; 2]);
        assert_eq!(statuses(KNOWLEDGE), vec![HookFamilyStatus::Missing; 2]);

        assert_eq!(apply_family(store, &BOTH, KNOWLEDGE, at(1)).unwrap(), BOTH.to_vec());
        assert_eq!(statuses(KNOWLEDGE), vec![HookFamilyStatus::Installed; 2]);
    }

    #[test]
    fn restoring_knowledge_adds_its_hooks_again_and_never_overwrites_one() {
        let folder = folder();
        let store = &folder.store;
        seed_defaults(store, KNOWLEDGE, at(0)).unwrap();

        let member = HookRequirement::HarnessKnowledge;
        assert_eq!(restore_member(store, member, KNOWLEDGE, at(1)).unwrap(), vec![]);

        let recall = seeded(store.list(KNOWLEDGE).hooks, HookSeed::KnowledgeRecall);
        store.delete(&recall.id, DESKTOP, Some(&recall.sha256)).unwrap();

        assert_eq!(
            restore_member(store, member, KNOWLEDGE, at(3)).unwrap(),
            vec![HookSeed::KnowledgeRecall]
        );
    }
}
