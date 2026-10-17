//! The hook file format against the shared fixtures (`tests/fixtures/hooks`): every
//! `valid-*.json` is accepted and every `invalid-*.json` rejected, both by content and when loaded
//! from a hooks folder as `<id>.json`; the family fixtures match the catalog; the approvals
//! fixture holds the real SHA-256 of the files beside it; the hooks root is found as the format
//! doc says.

mod hooks_format {
    use std::path::{Path, PathBuf};

    use harness_business::env::{Env, OsKind};
    use harness_business::hooks::approvals::Approvals;
    use harness_business::hooks::{
        FAMILY, HookCapabilities, HookStore, SEED_MEMORY_SAVE_TEXT, SEED_MEMORY_SEARCH_TEXT, family_hook, hooks_root,
        is_hook_id, parse_hook_file, sha256_hex,
    };
    use harness_business::model::{
        HookAction, HookAuthor, HookEvent, HookRequirement, HookScope, HookSeed, HookStatus,
    };
    use pretty_assertions::assert_eq;
    use serde_json::Value;

    fn fixtures() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hooks")
    }

    /// `(file name, bytes)` of the fixtures whose name starts with `prefix`, sorted.
    fn fixture_files(prefix: &str) -> Vec<(String, Vec<u8>)> {
        let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(fixtures())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file())
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(prefix) && name.ends_with(".json"))
            .map(|name| {
                let bytes = std::fs::read(fixtures().join(&name)).unwrap();
                (name, bytes)
            })
            .collect();

        files.sort();
        assert!(!files.is_empty(), "no {prefix} fixtures");
        files
    }

    /// The `id` inside a fixture, when it is JSON with a string id.
    fn id_of(bytes: &[u8]) -> Option<String> {
        let value: Value = serde_json::from_slice(bytes).ok()?;
        value.get("id")?.as_str().map(str::to_owned)
    }

    #[test]
    fn every_valid_fixture_parses() {
        for (name, bytes) in fixture_files("valid-") {
            let id = id_of(&bytes).unwrap();

            if let Err(problem) = parse_hook_file(&id, &bytes) {
                panic!("{name} was rejected: {problem}");
            }
        }
    }

    #[test]
    fn every_invalid_fixture_is_rejected_by_content() {
        for (name, bytes) in fixture_files("invalid-") {
            let id = id_of(&bytes).unwrap_or_else(|| "0".repeat(32));

            assert!(parse_hook_file(&id, &bytes).is_err(), "{name} was accepted");
        }
    }

    #[test]
    fn fixtures_load_from_a_hooks_folder_as_their_id() {
        let temp = tempfile::tempdir().unwrap();
        let store = HookStore::new(temp.path().join("root"), temp.path().join("config"));
        std::fs::create_dir_all(store.hooks_dir()).unwrap();

        let mut valid = Vec::new();
        let mut invalid = Vec::new();

        for (name, bytes) in fixture_files("valid-").into_iter().chain(fixture_files("invalid-")) {
            // A fixture without a well-formed id cannot be named `<id>.json`; content tests cover it.
            let Some(id) = id_of(&bytes).filter(|id| is_hook_id(id)) else {
                continue;
            };

            std::fs::write(store.hooks_dir().join(format!("{id}.json")), &bytes).unwrap();

            if name.starts_with("valid-") {
                valid.push(id);
            } else {
                invalid.push(id);
            }
        }

        let listing = store.list(HookCapabilities::default());
        assert_eq!(listing.folder_problem, None);

        for id in &valid {
            let view = listing.hooks.iter().find(|view| &view.id == id).unwrap();
            assert_ne!(view.status, HookStatus::Invalid, "{id}: {:?}", view.problem);
            assert!(view.hook.is_some());
        }

        for id in &invalid {
            let view = listing.hooks.iter().find(|view| &view.id == id).unwrap();
            assert_eq!(view.status, HookStatus::Invalid, "{id}");
            assert!(view.problem.is_some());
            assert_eq!(view.hook, None);
        }

        assert_eq!(listing.hooks.len(), valid.len() + invalid.len());
    }

    #[test]
    fn a_fixture_parses_to_its_fields() {
        let bytes = std::fs::read(fixtures().join("valid-command-tool-end.json")).unwrap();
        let hook = parse_hook_file("1a2b3c4d5e6f70818293a4b5c6d70008", &bytes).unwrap();

        assert_eq!(hook.name, "Lint touched files");
        assert_eq!(hook.scope, HookScope::Global);
        assert_eq!(hook.event, HookEvent::ToolEnd);
        assert_eq!(hook.matcher.as_deref(), Some("Edit|Write"));
        assert_eq!(
            hook.action,
            HookAction::Command {
                command: "printf '%s\\n' \"$HARNESS_HOOK_TOUCHED\" | xargs -r eslint".to_owned(),
                timeout_secs: Some(30),
            }
        );
        assert_eq!(hook.created_by, HookAuthor::Desktop);

        let bytes = std::fs::read(fixtures().join("valid-agent-disabled.json")).unwrap();
        let hook = parse_hook_file("1a2b3c4d5e6f70818293a4b5c6d7000c", &bytes).unwrap();

        assert!(!hook.enabled);
        assert_eq!(hook.created_by, HookAuthor::Agent);
        assert_eq!(hook.requires, vec![HookRequirement::HarnessMemory]);
    }

    #[test]
    fn the_family_fixtures_match_the_catalog() {
        for entry in FAMILY {
            let name = format!("valid-seed-{}.json", entry.seed.as_str());
            let bytes = std::fs::read(fixtures().join(&name)).unwrap();
            let hook = parse_hook_file(&id_of(&bytes).unwrap(), &bytes).unwrap();

            assert_eq!(hook.seed, Some(entry.seed), "{name}");
            assert_eq!(hook.name, entry.name, "{name}");
            assert_eq!(hook.description.as_deref(), Some(entry.description), "{name}");
            assert_eq!(hook.event, entry.event, "{name}");
            assert_eq!(hook.action, entry.action(), "{name}");
            assert_eq!(hook.requires, vec![entry.member], "{name}");
        }

        assert_eq!(
            family_hook(HookSeed::MemorySearch).action(),
            HookAction::AddContext {
                text: SEED_MEMORY_SEARCH_TEXT.to_owned()
            }
        );
        assert_eq!(
            family_hook(HookSeed::MemorySave).action(),
            HookAction::FollowUp {
                text: SEED_MEMORY_SAVE_TEXT.to_owned()
            }
        );
        assert_eq!(
            family_hook(HookSeed::DashboardLimits).member,
            HookRequirement::HarnessDashboard
        );
    }

    #[test]
    fn names_and_descriptions_reject_separators_fillers_and_variation_selectors() {
        let bytes = std::fs::read(fixtures().join("valid-addcontext-session-start.json")).unwrap();
        let id = id_of(&bytes).unwrap();
        let with = |field: &str, value: &str| {
            let mut file: Value = serde_json::from_slice(&bytes).unwrap();
            file[field] = Value::from(value);
            parse_hook_file(&id, &serde_json::to_vec(&file).unwrap())
        };

        for character in [
            '\u{2028}',
            '\u{2029}',
            '\u{034F}',
            '\u{115F}',
            '\u{1160}',
            '\u{17B4}',
            '\u{17B5}',
            '\u{180B}',
            '\u{180D}',
            '\u{180F}',
            '\u{2800}',
            '\u{3164}',
            '\u{FE00}',
            '\u{FE0F}',
            '\u{FFA0}',
            '\u{E0100}',
            '\u{E01EF}',
        ] {
            let value = format!("Safe{character}rules");

            assert!(
                with("name", &value).is_err(),
                "name with U+{:04X}",
                u32::from(character)
            );
            assert!(
                with("description", &value).is_err(),
                "description with U+{:04X}",
                u32::from(character)
            );
        }

        let visible = "Zasady za\u{017C}\u{00F3}\u{0142}\u{0107} \u{65E5}\u{672C} \u{D55C}\u{AD6D}\u{C5B4} \u{1F44D}";
        assert!(with("name", visible).is_ok());
        assert!(with("description", visible).is_ok());
    }

    #[test]
    fn an_id_that_differs_from_the_file_name_is_invalid() {
        let bytes = std::fs::read(fixtures().join("valid-addcontext-session-start.json")).unwrap();

        let problem = parse_hook_file(&"0".repeat(32), &bytes).unwrap_err();
        assert!(problem.contains("file name"), "{problem}");
    }

    #[test]
    fn the_approvals_fixture_holds_real_hashes_and_one_stale_one() {
        let config = tempfile::tempdir().unwrap();
        std::fs::copy(
            fixtures().join("approvals/valid-approvals.json"),
            config.path().join("hook-approvals.json"),
        )
        .unwrap();
        let approvals = Approvals::read(config.path()).unwrap();
        let mut matched = 0;
        let mut stale = 0;

        for (_, bytes) in fixture_files("valid-") {
            let id = id_of(&bytes).unwrap();

            match approvals.get(&id) {
                Some(hash) if hash == sha256_hex(&bytes) => matched += 1,
                Some(_) => stale += 1,
                None => {}
            }
        }

        assert_eq!((matched, stale), (3, 1));
    }

    #[test]
    fn the_hooks_root_is_found_as_the_format_doc_says() {
        let linux = Env::for_tests("/home/u").with_os(OsKind::Linux);

        assert_eq!(
            hooks_root(&linux),
            Some(PathBuf::from("/home/u/.local/share/harness-hooks"))
        );
        assert_eq!(
            hooks_root(&linux.clone().with_var("XDG_DATA_HOME", "/data")),
            Some(PathBuf::from("/data/harness-hooks"))
        );
        assert_eq!(
            hooks_root(&linux.clone().with_var("XDG_DATA_HOME", "relative")),
            Some(PathBuf::from("/home/u/.local/share/harness-hooks"))
        );
        assert_eq!(
            hooks_root(&linux.clone().with_var("HARNESS_HOOKS_DIR", "/srv/hooks")),
            Some(PathBuf::from("/srv/hooks"))
        );
        assert_eq!(
            hooks_root(&linux.clone().with_var("HARNESS_HOOKS_DIR", "~/hooks")),
            Some(Path::new("/home/u").join("hooks"))
        );
        assert_eq!(
            hooks_root(&linux.clone().with_var("HARNESS_HOOKS_DIR", "relative")),
            Some(PathBuf::from("/home/u/.local/share/harness-hooks"))
        );

        let mac = Env::for_tests("/Users/u").with_os(OsKind::MacOs);
        assert_eq!(
            hooks_root(&mac),
            Some(PathBuf::from("/Users/u/Library/Application Support/harness-hooks"))
        );

        let windows = Env::for_tests("/home/u").with_os(OsKind::Windows);
        assert_eq!(hooks_root(&windows), None);
    }
}
