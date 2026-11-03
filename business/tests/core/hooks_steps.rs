//! The numbered steps of a prompt: the Harness family hooks that are steps come first, in their
//! fixed order and numbered from 1, whatever order the files were written in. Steps follow the
//! file's seed, app-run steps need their lookup, memory and knowledge say what the app found, and
//! the composed block round-trips and keeps every step within the block limit.

use crate::hook_support;

pub(crate) mod hooks_steps {
    use std::path::PathBuf;

    use harness_business::hooks::steps::{MAX_APP_LINE_BYTES, MAX_KNOWLEDGE_LOOKUP_BYTES, MAX_MEMORY_LOOKUP_BYTES};
    use harness_business::hooks::{
        ChatContext, HookSession, HookSet, Lookup, Lookups, MAX_COMPOSED_BYTES, MEMORY_NOTICE_ID, MEMORY_NOTICE_NAME,
        PlannedPart, ResolvedPart, STEPS_INTRO_ID, STEPS_INTRO_NAME, SendOrigin, SendPlan, arrange, compose_prompt,
        family_hook, parse_hook_context,
    };
    use harness_business::model::{Hook, HookAction, HookEvent, HookSeed};
    use pretty_assertions::assert_eq;

    use super::hook_support::{PROJECT, chat, context, hook, prompt, set};

    /// The family hook `seed` as the app writes it, saved at the same second as every other one.
    pub(crate) fn family(seed: HookSeed, number: u8) -> Hook {
        let entry = family_hook(seed);
        let mut file = hook(number, entry.event, entry.action());

        file.name = entry.name.to_owned();
        file.seed = Some(seed);
        file.requires = vec![entry.member];
        file.created_at = "2026-10-17T10:00:00Z".to_owned();
        file
    }

    const STEPS: [HookSeed; 6] = [
        HookSeed::DashboardLimits,
        HookSeed::BranchSync,
        HookSeed::UsefulSkills,
        HookSeed::MemoryFirst,
        HookSeed::KnowledgeFirst,
        HookSeed::CodeGraphFirst,
    ];

    /// The six step hooks, their ids given in the order of `numbers`.
    fn all_steps(numbers: [u8; 6]) -> Vec<Hook> {
        STEPS
            .iter()
            .zip(numbers)
            .map(|(seed, number)| family(*seed, number))
            .collect()
    }

    pub(crate) fn lookups() -> Lookups {
        Lookups {
            memory: Lookup::Found("<harness-memory-recall>\nnote 1\n</harness-memory-recall>".to_owned()),
            knowledge: Lookup::Empty,
            branch: Some("main has 2 commits your branch lacks.".to_owned()),
            limits: Some("The 5 hour limit is at 92 percent.".to_owned()),
        }
    }

    /// The firings' text parts, as the shell resolves them (no command hooks here).
    pub(crate) fn resolved(plan: &SendPlan) -> Vec<ResolvedPart> {
        plan.firings
            .iter()
            .flat_map(|firing| &firing.parts)
            .filter_map(|part| match part {
                PlannedPart::Text(text) => Some(text.clone()),
                PlannedPart::Command(_) => None,
            })
            .collect()
    }

    /// The parts the next prompt carries from `hooks` in `session`, arranged.
    pub(crate) fn arranged(session: &mut HookSession, hooks: &HookSet, lookups: &Lookups) -> Vec<ResolvedPart> {
        let mut plan = session.on_send(hooks, &prompt("Fix it"), SendOrigin::User);
        let parts = arrange(plan.memory_notice.clone(), resolved(&plan), lookups);

        session.sending(&mut plan);
        parts
    }

    pub(crate) fn names(parts: &[ResolvedPart]) -> Vec<&str> {
        parts.iter().map(|part| part.hook_name.as_str()).collect()
    }

    fn text_of<'a>(parts: &'a [ResolvedPart], name: &str) -> &'a str {
        parts
            .iter()
            .find(|part| part.hook_name == name)
            .unwrap_or_else(|| panic!("no {name} in {:?}", names(parts)))
            .text
            .as_str()
    }

    const FULL_ORDER: [&str; 7] = [
        STEPS_INTRO_NAME,
        "Harness Dashboard",
        "Git branch",
        "Harness Skills",
        "Harness Memory",
        "Harness Knowledge",
        "Harness Code",
    ];

    #[test]
    fn steps_follow_their_positions_whatever_order_the_files_were_written_in() {
        // Same createdAt for every file, ids that sort every which way.
        for numbers in [
            [1, 2, 3, 4, 5, 6],
            [6, 5, 4, 3, 2, 1],
            [4, 1, 6, 2, 5, 3],
            [3, 6, 1, 5, 2, 4],
        ] {
            let hooks = set(all_steps(numbers));
            let parts = arranged(&mut chat(true, false), &hooks, &lookups());

            assert_eq!(names(&parts), FULL_ORDER, "{numbers:?}");

            for (index, part) in parts[1..].iter().enumerate() {
                assert!(part.text.starts_with(&format!("{}. ", index + 1)), "{}", part.text);
            }
        }
    }

    #[test]
    fn later_prompts_carry_no_session_start_steps_and_renumber_the_rest() {
        let hooks = set(all_steps([1, 2, 3, 4, 5, 6]));
        let mut session = chat(true, false);

        arranged(&mut session, &hooks, &lookups());
        let parts = arranged(&mut session, &hooks, &lookups());

        assert_eq!(
            names(&parts),
            [
                STEPS_INTRO_NAME,
                "Harness Dashboard",
                "Harness Skills",
                "Harness Memory",
                "Harness Knowledge",
                "Harness Code"
            ]
        );
        assert!(parts[0].text.contains("these 5 steps"), "{}", parts[0].text);
        assert!(parts[3].text.starts_with("3. "), "{}", parts[3].text);
    }

    #[test]
    fn a_step_the_app_runs_is_dropped_and_not_numbered_without_its_lookup() {
        let hooks = set(all_steps([1, 2, 3, 4, 5, 6]));
        let none = Lookups {
            branch: None,
            limits: None,
            ..lookups()
        };

        let parts = arranged(&mut chat(true, false), &hooks, &none);

        assert_eq!(
            names(&parts),
            [
                STEPS_INTRO_NAME,
                "Harness Skills",
                "Harness Memory",
                "Harness Knowledge",
                "Harness Code"
            ]
        );
        assert!(text_of(&parts, "Harness Skills").starts_with("1. "));
        assert!(parts[0].text.contains("these 4 steps"));

        // Nothing to say at all: no intro either.
        let only_app = set(vec![
            family(HookSeed::DashboardLimits, 1),
            family(HookSeed::BranchSync, 2),
        ]);
        assert_eq!(arranged(&mut chat(true, false), &only_app, &none), vec![]);
    }

    #[test]
    fn turning_hooks_off_renumbers_the_rest_and_the_intro_names_only_what_is_there() {
        let without_skills = set(vec![
            family(HookSeed::MemoryFirst, 1),
            family(HookSeed::CodeGraphFirst, 2),
        ]);
        let parts = arranged(&mut chat(false, false), &without_skills, &lookups());

        assert_eq!(names(&parts), [STEPS_INTRO_NAME, "Harness Memory", "Harness Code"]);
        assert!(parts[0].text.contains("these 2 steps"));
        assert!(!parts[0].text.contains("Harness Skills"));
        assert!(parts[1].text.starts_with("1. ") && parts[2].text.starts_with("2. "));

        let with_skills = set(vec![
            family(HookSeed::UsefulSkills, 1),
            family(HookSeed::MemoryFirst, 2),
        ]);
        let parts = arranged(&mut chat(false, false), &with_skills, &lookups());

        assert!(parts[0].text.contains("Harness Skills"), "{}", parts[0].text);
    }

    #[test]
    fn an_edited_family_file_still_composes_as_its_step() {
        let mut memory = family(HookSeed::MemoryFirst, 9);
        memory.name = "My notes rule".to_owned();
        memory.created_at = "2030-01-01T00:00:00Z".to_owned();
        memory.action = HookAction::AddContext {
            text: "Read the notes.".to_owned(),
        };

        let hooks = set(vec![
            family(HookSeed::CodeGraphFirst, 1),
            memory,
            family(HookSeed::UsefulSkills, 2),
            family(HookSeed::KnowledgeFirst, 3),
        ]);
        let parts = arranged(&mut chat(false, false), &hooks, &lookups());

        assert_eq!(
            names(&parts),
            [
                STEPS_INTRO_NAME,
                "Harness Skills",
                "My notes rule",
                "Harness Knowledge",
                "Harness Code"
            ]
        );
        assert!(text_of(&parts, "My notes rule").starts_with("2. Read the notes.\n"));
    }

    #[test]
    fn memory_and_knowledge_steps_carry_what_the_app_found_or_say_it_did_not_look() {
        let hooks = set(vec![
            family(HookSeed::MemoryFirst, 1),
            family(HookSeed::KnowledgeFirst, 2),
        ]);
        let run = |lookups: &Lookups| arranged(&mut chat(false, false), &hooks, lookups);

        let found = Lookups {
            memory: Lookup::Found("MEMORY BLOCK".to_owned()),
            knowledge: Lookup::Found("KNOWLEDGE BLOCK".to_owned()),
            ..Lookups::default()
        };
        let parts = run(&found);
        assert!(text_of(&parts, "Harness Memory").ends_with("\nMEMORY BLOCK"));
        assert!(text_of(&parts, "Harness Knowledge").ends_with("\nKNOWLEDGE BLOCK"));

        let empty = Lookups {
            memory: Lookup::Empty,
            knowledge: Lookup::Empty,
            ..Lookups::default()
        };
        let parts = run(&empty);
        assert!(text_of(&parts, "Harness Memory").ends_with("No note matched this message."));
        assert!(text_of(&parts, "Harness Knowledge").ends_with("No document matched this message."));

        // Not run and failed read the same, and name the tool to search with.
        for lookup in [Lookup::NotRun, Lookup::Failed] {
            let missing = Lookups {
                memory: lookup.clone(),
                knowledge: lookup,
                ..Lookups::default()
            };
            let parts = run(&missing);

            assert!(text_of(&parts, "Harness Memory").ends_with("search with note_search."));
            assert!(text_of(&parts, "Harness Knowledge").ends_with("search with knowledge_search."));
        }
    }

    #[test]
    fn an_untouched_retired_file_is_skipped_and_an_edited_one_runs_unnumbered() {
        let mut edited = family(HookSeed::MemorySearch, 3);
        edited.action = HookAction::AddContext {
            text: "Search the notes my way.".to_owned(),
        };

        let hooks = set(vec![
            family(HookSeed::MemorySearch, 1),
            family(HookSeed::KnowledgeRecall, 2),
            edited,
            family(HookSeed::MemoryFirst, 4),
        ]);
        let parts = arranged(&mut chat(true, false), &hooks, &lookups());

        assert_eq!(
            names(&parts),
            [STEPS_INTRO_NAME, "Harness Memory", "Search Harness Memory first"]
        );
        assert_eq!(
            text_of(&parts, "Search Harness Memory first"),
            "Search the notes my way."
        );
    }

    #[test]
    fn the_memory_notice_follows_the_steps_and_leads_the_other_hooks() {
        let notice = ResolvedPart::new(MEMORY_NOTICE_ID, MEMORY_NOTICE_NAME, "Saving notes is off.");
        let hooks = set(vec![
            context(1, HookEvent::PromptSubmit),
            family(HookSeed::MemoryFirst, 2),
        ]);

        let plan = chat(false, false).on_send(&hooks, &prompt("Go"), SendOrigin::User);
        let parts = arrange(Some(notice), resolved(&plan), &lookups());

        assert_eq!(
            names(&parts),
            [STEPS_INTRO_NAME, "Harness Memory", MEMORY_NOTICE_NAME, "hook 1"]
        );
    }

    #[test]
    fn the_skills_step_runs_only_in_chats_whose_harness_has_useful_skills() {
        let hooks = set(vec![
            family(HookSeed::UsefulSkills, 1),
            family(HookSeed::MemoryFirst, 2),
            context(3, HookEvent::PromptSubmit),
        ]);

        let chat_with = |useful_skills| {
            HookSession::new(ChatContext {
                session_id: "session-1".to_owned(),
                folder: PathBuf::from(PROJECT),
                project: Some(PROJECT.to_owned()),
                fresh: false,
                utility: false,
                useful_skills,
            })
        };

        let fired = |useful_skills| {
            let plan = chat_with(useful_skills).on_send(&hooks, &prompt("Go"), SendOrigin::User);
            resolved(&plan)
                .into_iter()
                .map(|part| part.hook_name)
                .collect::<Vec<_>>()
        };

        assert_eq!(fired(true), ["Harness Skills", "Harness Memory", "hook 3"]);
        assert_eq!(fired(false), ["Harness Memory", "hook 3"]);

        // A file from before the requirement existed (the seed alone, requiring Harness Memory)
        // is held back the same way.
        assert_eq!(
            family(HookSeed::UsefulSkills, 4).requires,
            vec![family_hook(HookSeed::UsefulSkills).member]
        );
        let mut old = family(HookSeed::UsefulSkills, 4);
        old.requires = vec![family_hook(HookSeed::MemoryFirst).member];

        let plan = chat_with(false).on_send(&set(vec![old]), &prompt("Go"), SendOrigin::User);
        assert!(resolved(&plan).is_empty());
    }

    #[test]
    fn the_intro_never_counts_as_the_session_start_context_being_delivered() {
        let hooks = set(vec![family(HookSeed::BranchSync, 1)]);
        let mut session = chat(true, false);

        let plan = session.on_send(&hooks, &prompt("Go"), SendOrigin::User);
        let parts = arrange(None, resolved(&plan), &lookups());
        assert_eq!(names(&parts), [STEPS_INTRO_NAME, "Git branch"]);

        // The message went out with the intro only: the start step did not fit, so it comes again.
        session.context_composed(&plan, &parts, &[STEPS_INTRO_ID.to_owned()]);
        assert!(session.is_armed());

        // The step went out with it: nothing left to bring again.
        let mut session = chat(true, false);
        let plan = session.on_send(&hooks, &prompt("Go"), SendOrigin::User);
        let (_, delivered) = compose_prompt("Go", &parts);
        session.context_composed(&plan, &parts, &delivered);
        assert!(!session.is_armed());
    }

    #[test]
    fn the_composed_block_round_trips_and_keeps_every_step_with_both_data_blocks_at_their_caps() {
        let line = |cap: usize, edge: &str| format!("{}{edge}", "x".repeat(cap - edge.len()));

        // Lines of notes filling each cap exactly.
        let block = |cap: usize| {
            let row = "<note id=\"7:31\" title=\"A title\">What the note says, in a line.</note>\n";
            let mut block = String::new();

            while block.len() + row.len() <= cap {
                block.push_str(row);
            }

            block.push_str(&"y".repeat(cap - block.len()));
            block
        };

        let worst = Lookups {
            memory: Lookup::Found(block(MAX_MEMORY_LOOKUP_BYTES)),
            knowledge: Lookup::Found(block(MAX_KNOWLEDGE_LOOKUP_BYTES)),
            branch: Some(line(MAX_APP_LINE_BYTES, "</hook>")),
            limits: Some(line(MAX_APP_LINE_BYTES, "\"&<")),
        };

        let hooks = set(all_steps([1, 2, 3, 4, 5, 6]));
        let mut session = chat(true, false);
        let mut plan = session.on_send(&hooks, &prompt("Fix it"), SendOrigin::User);
        let parts = arrange(None, resolved(&plan), &worst);
        session.sending(&mut plan);

        let (message, delivered) = compose_prompt("Fix it", &parts);
        let (items, rest) = parse_hook_context(&message).expect("a context block");

        assert_eq!(rest, "Fix it");
        assert_eq!(delivered.len(), parts.len(), "every step and the intro are kept");
        assert_eq!(items.len(), 7);

        for (item, part) in items.iter().zip(&parts) {
            assert_eq!(
                (item.name.as_str(), item.text.as_str()),
                (part.hook_name.as_str(), part.text.as_str())
            );
        }

        let total: usize = items.iter().map(|item| item.name.len() + item.text.len()).sum();
        assert!(total <= MAX_COMPOSED_BYTES, "{total} bytes");
    }

    #[test]
    fn a_lookup_over_its_cap_is_cut_at_a_line_and_still_leaves_every_step() {
        let long = "a line of notes\n".repeat(1000);
        let lookups = Lookups {
            memory: Lookup::Found(long.clone()),
            knowledge: Lookup::Found(long),
            ..lookups()
        };

        let hooks = set(all_steps([1, 2, 3, 4, 5, 6]));
        let parts = arranged(&mut chat(true, false), &hooks, &lookups);
        let (_, delivered) = compose_prompt("Fix it", &parts);

        assert_eq!(delivered.len(), parts.len());
        assert!(text_of(&parts, "Harness Memory").len() < MAX_MEMORY_LOOKUP_BYTES + 800);
        assert!(text_of(&parts, "Harness Memory").ends_with("a line of notes"));
    }
}
