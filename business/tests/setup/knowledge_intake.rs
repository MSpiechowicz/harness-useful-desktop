//! The files and folders picked or dropped to import: checked without following links, kept under
//! a token the import names them by, the drop taken once, and the most files an import takes.

use harness_business::BusinessError;
use harness_business::ecosystem::{IMPORT_EXTENSIONS, KnowledgeIntakes, MAX_INTAKE_FILES, check_paths};
use harness_business::model::KnowledgeSkipReason;
use pretty_assertions::assert_eq;

fn dir_with_docs() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("guide.md"), "# Guide").unwrap();
    std::fs::create_dir(dir.path().join("handbook")).unwrap();

    dir
}

#[test]
fn files_and_folders_are_kept_and_what_cannot_be_imported_is_named_with_its_reason() {
    let dir = dir_with_docs();
    let guide = dir.path().join("guide.md");
    let handbook = dir.path().join("handbook");

    let mut paths = vec![
        guide.clone(),
        handbook.clone(),
        guide.clone(),
        dir.path().join("gone.md"),
        std::path::PathBuf::from("relative/notes.md"),
    ];

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&guide, dir.path().join("link.md")).unwrap();
        paths.push(dir.path().join("link.md"));
    }

    let checked = check_paths(&paths);

    assert_eq!(checked.files.len(), 1, "a repeat counts once");
    assert_eq!(
        (checked.files[0].name.as_str(), checked.files[0].bytes),
        ("guide.md", Some(7))
    );
    assert_eq!(
        (checked.folders[0].name.as_str(), checked.folders[0].bytes),
        ("handbook", None)
    );

    let reasons: Vec<_> = checked
        .skipped
        .iter()
        .map(|skipped| (skipped.name.as_str(), skipped.reason))
        .collect();
    assert_eq!(reasons[0], ("gone.md", KnowledgeSkipReason::Missing));
    assert_eq!(reasons[1], ("notes.md", KnowledgeSkipReason::Missing));

    #[cfg(unix)]
    assert_eq!(reasons[2], ("link.md", KnowledgeSkipReason::Symlink));
}

#[test]
fn an_intake_is_kept_by_its_token_until_forgotten_and_an_empty_one_is_not_kept() {
    let dir = dir_with_docs();
    let intakes = KnowledgeIntakes::new();

    let intake = intakes.put(&[dir.path().join("guide.md"), dir.path().join("handbook")]);
    let kept = intakes.get(&intake.token).unwrap();
    assert_eq!(kept.files, [dir.path().join("guide.md").to_string_lossy()]);
    assert_eq!(kept.folders, [dir.path().join("handbook").to_string_lossy()]);

    intakes.forget(&intake.token);
    assert!(matches!(intakes.get(&intake.token), Err(BusinessError::NotFound(_))));

    let nothing = intakes.put(&[dir.path().join("gone.md")]);
    assert!(nothing.files.is_empty());
    assert!(matches!(intakes.get(&nothing.token), Err(BusinessError::NotFound(_))));
    assert!(matches!(intakes.get("intake-999"), Err(BusinessError::NotFound(_))));
}

#[test]
fn only_the_newest_intakes_are_kept() {
    let dir = dir_with_docs();
    let intakes = KnowledgeIntakes::new();
    let tokens: Vec<String> = (0..9)
        .map(|_| intakes.put(&[dir.path().join("guide.md")]).token)
        .collect();

    assert!(intakes.get(&tokens[0]).is_err(), "the oldest went");
    assert!(intakes.get(&tokens[1]).is_ok());
    assert!(intakes.get(&tokens[8]).is_ok());
}

#[test]
fn the_last_drop_is_taken_once_and_a_newer_drop_replaces_it() {
    let dir = dir_with_docs();
    let intakes = KnowledgeIntakes::new();

    assert!(intakes.take_drop().is_none());

    intakes.stash_drop(&[dir.path().join("handbook")]);
    intakes.stash_drop(&[dir.path().join("guide.md")]);

    let taken = intakes.take_drop().expect("a drop");
    assert_eq!(taken.files.len(), 1);
    assert!(taken.folders.is_empty());
    assert!(intakes.get(&taken.token).is_ok());
    assert!(intakes.take_drop().is_none());
}

#[test]
fn no_more_files_than_an_import_takes_are_kept() {
    let dir = tempfile::tempdir().unwrap();
    let paths: Vec<_> = (0..MAX_INTAKE_FILES + 2)
        .map(|n| {
            let path = dir.path().join(format!("{n}.md"));
            std::fs::write(&path, "x").unwrap();
            path
        })
        .collect();

    let checked = check_paths(&paths);

    assert_eq!(checked.files.len(), MAX_INTAKE_FILES);
    assert_eq!(checked.skipped.len(), 2);
    assert!(
        checked
            .skipped
            .iter()
            .all(|skipped| skipped.reason == KnowledgeSkipReason::TooMany)
    );
}

#[test]
fn the_picker_offers_every_format_an_import_reads() {
    for extension in ["md", "txt", "html", "json", "yaml", "adoc", "rst", "pdf"] {
        assert!(IMPORT_EXTENSIONS.contains(&extension), "{extension}");
    }

    assert!(IMPORT_EXTENSIONS.iter().all(|extension| !extension.starts_with('.')));
}
