use super::*;

#[test]
fn budgeted_walks_share_one_budget() {
    let temp = tempfile::TempDir::new().unwrap();
    for index in 0..10 {
        std::fs::write(temp.path().join(format!("{index}.md")), "").unwrap();
    }

    let mut budget = WalkBudget::new(15);
    let visits: Vec<usize> = (0..3)
        .map(|_| {
            let mut visited = 0;
            walk_tree_budgeted(temp.path(), 1, &mut budget, &mut |_| {
                visited += 1;
                false
            });
            visited
        })
        .collect();

    assert_eq!(visits, [10, 5, 0]);
}

/// A `package.json` of exactly `size` bytes declaring `./main.ts`.
fn write_manifest(path: &Path, size: usize) {
    let head = r#"{"pi":{"extensions":["./main.ts"]},"pad":""#;
    let tail = r#""}"#;
    let padding = "x".repeat(size - head.len() - tail.len());

    std::fs::write(path, format!("{head}{padding}{tail}")).unwrap();
}

/// Package directories `e0000`, `e0001`, ... under `dir`, each with `main.ts`, `index.ts` and
/// a `package.json` linked to `manifest`.
#[cfg(unix)]
fn linked_packages(dir: &Path, manifest: &Path, count: usize) {
    for index in 0..count {
        let package = dir.join(format!("e{index:04}"));
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(package.join("main.ts"), "").unwrap();
        std::fs::write(package.join("index.ts"), "").unwrap();
        std::os::unix::fs::symlink(manifest, package.join("package.json")).unwrap();
    }
}

fn count_named(paths: &[PathBuf], name: &str) -> usize {
    paths.iter().filter(|path| path.ends_with(name)).count()
}

#[cfg(unix)]
#[test]
fn links_to_one_package_directory_list_it_once_and_read_its_manifest_once() {
    let temp = tempfile::TempDir::new().unwrap();
    let target = temp.path().join("target");
    let dir = temp.path().join("extensions");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(target.join("main.ts"), "").unwrap();
    write_manifest(&target.join("package.json"), 100);

    for index in 0..MAX_DIR_ENTRIES {
        std::os::unix::fs::symlink(&target, dir.join(format!("e{index:04}"))).unwrap();
    }

    let mut budget = ScanBudget::per_view();
    let found = extension_module_paths(&dir, &mut budget);

    let main = dunce::canonicalize(target.join("main.ts")).unwrap();
    assert_eq!(found, [main]);
    assert_eq!(budget.bytes_left, MAX_VIEW_READ_BYTES - MIN_READ_COST);
}

#[cfg(unix)]
#[test]
fn oversized_manifests_fall_back_to_index_files_without_being_read() {
    let temp = tempfile::TempDir::new().unwrap();
    let dir = temp.path().join("extensions");
    let manifest = temp.path().join("package.json");
    write_manifest(&manifest, 2 * 1024 * 1024);
    linked_packages(&dir, &manifest, MAX_DIR_ENTRIES);

    let mut budget = ScanBudget::per_view();
    let found = extension_module_paths(&dir, &mut budget);

    assert_eq!(found.len(), MAX_DIR_ENTRIES);
    assert_eq!(count_named(&found, "index.ts"), MAX_DIR_ENTRIES);
    // Rejected on size before any byte is read, so each costs only the minimum.
    assert_eq!(
        budget.bytes_left,
        MAX_VIEW_READ_BYTES - MAX_DIR_ENTRIES as u64 * MIN_READ_COST
    );
    // Over the per-file cap is not a budget running out.
    assert!(!budget.truncated());
}

#[cfg(unix)]
#[test]
fn manifest_reads_stop_once_the_shared_budget_is_spent() {
    let size = 200 * 1024;
    let temp = tempfile::TempDir::new().unwrap();
    let first = temp.path().join("first");
    let second = temp.path().join("second");
    let manifest = temp.path().join("package.json");
    write_manifest(&manifest, size);
    linked_packages(&first, &manifest, 30);
    linked_packages(&second, &manifest, 30);

    let mut budget = ScanBudget::per_view();
    let from_first = extension_module_paths(&first, &mut budget);
    let from_second = extension_module_paths(&second, &mut budget);

    // 40 manifests fit in the view budget; the rest fall back to their index files.
    let honoured = (MAX_VIEW_READ_BYTES / size as u64) as usize;
    assert_eq!(honoured, 40);
    assert_eq!(count_named(&from_first, "main.ts"), 30);
    assert_eq!(count_named(&from_second, "main.ts"), honoured - 30);
    assert_eq!(count_named(&from_second, "index.ts"), 60 - honoured);
    // The other 20 do not fit the bytes left, so they are skipped for the minimum each.
    let left = MAX_VIEW_READ_BYTES - honoured as u64 * size as u64;
    assert_eq!(budget.bytes_left, left - 20 * MIN_READ_COST);
    assert!(budget.truncated());
}

#[test]
fn missing_manifests_cost_no_bytes_and_small_ones_the_minimum() {
    let temp = tempfile::TempDir::new().unwrap();
    for index in 0..100 {
        let package = temp.path().join(format!("plain{index:03}"));
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(package.join("index.ts"), "").unwrap();
    }
    let declared = temp.path().join("zz-declared");
    std::fs::create_dir_all(&declared).unwrap();
    std::fs::write(declared.join("main.ts"), "").unwrap();
    write_manifest(&declared.join("package.json"), 100);

    let mut budget = ScanBudget::new(MAX_VIEW_SCAN_STEPS, MIN_READ_COST);
    let found = extension_module_paths(temp.path(), &mut budget);

    assert_eq!(count_named(&found, "index.ts"), 100);
    assert_eq!(count_named(&found, "main.ts"), 1);
    assert_eq!(budget.bytes_left, 0);
}

#[test]
fn a_scan_stops_once_its_steps_are_spent() {
    let temp = tempfile::TempDir::new().unwrap();
    for index in 0..20 {
        let package = temp.path().join(format!("e{index:02}"));
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(package.join("index.ts"), "").unwrap();
    }

    let all = extension_module_paths(temp.path(), &mut ScanBudget::per_view());
    let mut budget = ScanBudget::new(30, MAX_VIEW_READ_BYTES);
    let bounded = extension_module_paths(temp.path(), &mut budget);

    assert_eq!(all.len(), 20);
    assert!(!bounded.is_empty() && bounded.len() < all.len());
    assert_eq!(bounded, all[..bounded.len()]);
    assert_eq!(budget.steps_left, 0);
    assert!(extension_module_paths(temp.path(), &mut budget).is_empty());
}

#[cfg(unix)]
#[test]
fn aliases_of_one_target_count_once() {
    let temp = tempfile::TempDir::new().unwrap();
    let hub = temp.path().join("hub");
    let other = temp.path().join("other");
    std::fs::create_dir_all(&hub).unwrap();
    std::fs::create_dir_all(&other).unwrap();
    std::os::unix::fs::symlink(&hub, temp.path().join("link")).unwrap();

    let missing = temp.path().join("missing");
    let paths = [
        hub.clone(),
        temp.path().join("hub/../hub"),
        temp.path().join("link"),
        missing.clone(),
        temp.path().join("other/../hub/../hub"),
        other.clone(),
        missing.clone(),
    ];

    let mut budget = ScanBudget::per_view();
    assert_eq!(
        distinct_targets(paths.clone(), 10, &mut budget),
        [hub.clone(), missing, other]
    );
    assert_eq!(distinct_targets(paths, 3, &mut budget), [hub]);
}

#[test]
fn declared_paths_resolve_inside_the_canonical_root_only() {
    let temp = tempfile::TempDir::new().unwrap();
    let root = dunce::canonicalize(temp.path()).unwrap().join("package");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/main.ts"), "").unwrap();
    std::fs::write(temp.path().join("outside.ts"), "").unwrap();

    assert_eq!(
        canonical_inside(&root, Path::new("./src/../src/main.ts")),
        Some(root.join("src/main.ts"))
    );
    assert_eq!(canonical_inside(&root, Path::new("../outside.ts")), None);
    assert_eq!(canonical_inside(&root, Path::new("missing.ts")), None);
}

#[test]
fn distinct_keeps_the_first_of_each_in_order() {
    assert_eq!(distinct(["b", "a", "b", "c", "a"], 10), ["b", "a", "c"]);
    assert_eq!(distinct(["b", "b", "a", "c"], 2), ["b", "a"]);
}

#[cfg(unix)]
#[test]
fn a_walk_stops_after_its_entry_budget() {
    let temp = tempfile::TempDir::new().unwrap();
    let files = temp.path().join("files");
    let root = temp.path().join("root");
    std::fs::create_dir_all(&files).unwrap();
    std::fs::create_dir_all(&root).unwrap();

    for index in 0..MAX_DIR_ENTRIES {
        std::fs::write(files.join(format!("{index}.md")), "").unwrap();
    }
    // Six links to the same full directory are not a loop, so each is walked in turn.
    for name in ["a", "b", "c", "d", "e", "f"] {
        std::os::unix::fs::symlink(&files, root.join(name)).unwrap();
    }

    let mut visited = 0;
    walk_tree(&root, 2, &mut |_| {
        visited += 1;
        true
    });

    assert_eq!(visited, MAX_WALK_ENTRIES);
}

// macOS (APFS) refuses file names that are not UTF-8, so this setup cannot be built there.
#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn skipped_names_still_count_against_the_budget() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let temp = tempfile::TempDir::new().unwrap();
    let leaf = temp.path().join("leaf");
    let middle = temp.path().join("middle");
    let root = temp.path().join("root");
    for dir in [&leaf, &middle, &root] {
        std::fs::create_dir_all(dir).unwrap();
    }

    // Names that are not UTF-8 are never visited, but reading them is not free.
    for index in 0..MAX_DIR_ENTRIES {
        let name = [b"\xff".as_slice(), index.to_string().as_bytes()].concat();
        std::fs::write(leaf.join(OsStr::from_bytes(&name)), "").unwrap();
    }
    for index in 0..MAX_DIR_ENTRIES {
        std::os::unix::fs::symlink(&leaf, middle.join(format!("{index}"))).unwrap();
    }
    for name in ["a", "b", "c", "d", "e"] {
        std::os::unix::fs::symlink(&middle, root.join(name)).unwrap();
    }

    let mut visited = 0;
    walk_tree(&root, 3, &mut |_| {
        visited += 1;
        true
    });

    // The root's links and one `middle` listing; the leaf reads then use up the budget.
    assert_eq!(visited, 5 + MAX_DIR_ENTRIES);
}

#[cfg(unix)]
#[test]
fn files_rejected_before_reading_cost_the_minimum_and_unreadable_text_its_size() {
    let temp = tempfile::TempDir::new().unwrap();
    let big = vec![b'x'; 300 * 1024];
    for index in 0..32 {
        std::fs::write(temp.path().join(format!("big{index:02}.md")), &big).unwrap();
    }
    std::fs::write(temp.path().join("secret.pem"), vec![b'x'; 200 * 1024]).unwrap();
    std::os::unix::fs::symlink(temp.path().join("secret.pem"), temp.path().join("denied.md")).unwrap();
    std::fs::write(temp.path().join("binary.md"), vec![0xff; 10_000]).unwrap();
    std::fs::create_dir(temp.path().join("folder.md")).unwrap();

    let mut budget = ScanBudget::per_view();
    for entry in list_dir(temp.path()) {
        assert_eq!(budget.read_frontmatter(&entry.path), None, "{}", entry.name);
    }

    // 32 oversized files, the denied file and its link, and the directory at the minimum, the
    // file that was read and then rejected (not UTF-8) at its size.
    assert_eq!(budget.bytes_left, MAX_VIEW_READ_BYTES - 35 * MIN_READ_COST - 10_000);
    assert!(!budget.truncated());
}

#[test]
fn caps_that_drop_entries_mark_the_budget_truncated() {
    let temp = tempfile::TempDir::new().unwrap();
    for index in 0..MAX_DIR_ENTRIES + 1 {
        std::fs::write(temp.path().join(format!("{index:04}.ts")), "").unwrap();
    }

    let mut full = ScanBudget::per_view();
    assert_eq!(full.list_dir(temp.path()).len(), MAX_DIR_ENTRIES);
    assert!(full.truncated(), "over the per-directory cap");

    let mut short = ScanBudget::new(10, MAX_VIEW_READ_BYTES);
    assert_eq!(short.list_dir(temp.path()).len(), 10);
    assert!(short.truncated(), "out of steps");

    let paths: Vec<PathBuf> = (0..3).map(|index| temp.path().join(format!("{index:04}.ts"))).collect();
    let mut listed = ScanBudget::per_view();
    assert_eq!(distinct_targets(paths.clone(), 3, &mut listed).len(), 3);
    assert!(!listed.truncated(), "everything fits");
    assert_eq!(distinct_targets(paths, 2, &mut listed).len(), 2);
    assert!(listed.truncated(), "over the list cap");

    let empty = tempfile::TempDir::new().unwrap();
    let mut spent = ScanBudget::new(0, 0);
    assert!(spent.list_dir(empty.path()).is_empty());
    assert!(spent.canonicalize(&empty.path().join("missing")).is_none());
    assert!(!spent.truncated(), "nothing was there to skip");
}

#[test]
fn a_walk_cut_short_marks_its_budget_truncated() {
    let temp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(temp.path().join("a/deeper")).unwrap();
    std::fs::write(temp.path().join("a/deeper/file"), "").unwrap();

    let mut shallow = WalkBudget::new(100);
    walk_tree_budgeted(temp.path(), 2, &mut shallow, &mut |_| true);
    assert!(!shallow.truncated(), "the depth limit is not a cap");

    let mut short = WalkBudget::new(2);
    walk_tree_budgeted(temp.path(), 4, &mut short, &mut |_| true);
    assert!(short.truncated());
}
