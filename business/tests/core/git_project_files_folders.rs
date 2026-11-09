//! `project_files` folders: which ones are derived from the files when a list holds more of them
//! than the limits (their number, and the bytes of their paths) allow. A crafted `.git/index` is
//! stood in for by the `Index` runner.

use std::fs;

use super::git_project_files::base_repo;
use super::git_project_files_hostile::{Index, indexed};
use crate::git_support::git_home;

/// What the list may derive in all (`MAX_DERIVED_FOLDERS`).
const FOLDER_LIMIT: usize = 50_000;

/// The bytes of folder paths the list looks at in all (`MAX_FOLDER_BYTES`).
const FOLDER_BYTES: usize = 16 * 1024 * 1024;

#[tokio::test]
async fn a_late_top_level_folder_is_offered_when_earlier_files_hold_more_folders_than_the_limit() {
    let home = git_home();
    let repo = base_repo(&home);
    fs::create_dir_all(repo.path.join("zzz")).unwrap();
    fs::write(repo.path.join("zzz/last.md"), "x\n").unwrap();

    // 30,000 top-level folders with a folder in each (60,000 in all), and `zzz` last in git's order.
    let mut names: Vec<String> = (0..30_000).map(|number| format!("a{number}/b/f.md")).collect();
    names.push("zzz/last.md".to_owned());
    let names: Vec<&str> = names.iter().map(String::as_str).collect();

    let index = indexed(&home, &repo.path, &Index::of(&names)).await;
    let found = index.search(&repo.path, "zzz", 50);

    assert!(
        found.iter().any(|file| file.path == "zzz" && file.is_dir),
        "{:?}",
        found.iter().map(|file| &file.path).collect::<Vec<_>>()
    );
    assert_eq!(
        index.len(),
        names.len() + FOLDER_LIMIT,
        "files, and folders up to the limit"
    );
}

#[tokio::test]
async fn a_top_level_folder_reached_only_by_a_deep_file_beats_the_deeper_folders_of_others() {
    let home = git_home();
    let repo = base_repo(&home);
    fs::create_dir_all(repo.path.join("top/x/y/z")).unwrap();

    // 30,000 folders with a folder in each come first. `top` holds nothing but a file four down.
    let mut names: Vec<String> = (0..30_000).map(|number| format!("a{number}/b/f.md")).collect();
    names.push("top/x/y/z/f.md".to_owned());
    let names: Vec<&str> = names.iter().map(String::as_str).collect();

    let index = indexed(&home, &repo.path, &Index::of(&names)).await;
    let found = index.search(&repo.path, "top", 50);

    // The 30,001 top-level folders all fit. Only 19,999 of the 30,000 second-level ones do.
    assert!(found.iter().any(|file| file.path == "top" && file.is_dir));
    assert!(!found.iter().any(|file| file.path == "top/x/y" && file.is_dir));
    assert_eq!(index.len(), names.len() + FOLDER_LIMIT);
}

#[tokio::test]
async fn every_folder_is_offered_when_more_files_than_the_limit_sit_in_a_few_folders() {
    let home = git_home();
    let repo = base_repo(&home);
    for folder in ["docs", "ui/web", "src/deep"] {
        fs::create_dir_all(repo.path.join(folder)).unwrap();
    }

    let mut names: Vec<String> = (0..FOLDER_LIMIT + 500)
        .map(|number| format!("src/deep/f{number}.md"))
        .collect();
    names.extend(["docs/x.md".to_owned(), "ui/web/y.md".to_owned()]);
    let names: Vec<&str> = names.iter().map(String::as_str).collect();

    let index = indexed(&home, &repo.path, &Index::of(&names)).await;
    let mut folders: Vec<String> = index
        .search(&repo.path, "", 50)
        .into_iter()
        .filter(|file| file.is_dir)
        .map(|file| file.path)
        .collect();
    folders.sort_unstable();

    assert_eq!(folders, ["docs", "src", "src/deep", "ui", "ui/web"]);
    assert_eq!(index.len(), names.len() + 5);
}

#[tokio::test]
async fn the_folder_paths_derived_are_bounded_in_bytes_and_not_only_in_number() {
    let home = git_home();
    let repo = base_repo(&home);

    // 300 files, each 62 folders of 62 bytes down: 63 folders each, 18,900 in all, far under the
    // folder limit, but about 37 MB of folder paths.
    let folder = "c".repeat(62);
    let names: Vec<String> = (0..300)
        .map(|number| format!("p{number:03}/{}/f.md", vec![folder.as_str(); 62].join("/")))
        .collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();

    // The folders are found level by level: `p000` has 4 bytes, each level below adds 63.
    let mut bytes = 0;
    let mut expected = 0;
    'levels: for level in 0..63 {
        for _ in 0..names.len() {
            bytes += 4 + 63 * level;

            if bytes > FOLDER_BYTES {
                break 'levels;
            }

            expected += 1;
        }
    }

    let index = indexed(&home, &repo.path, &Index::of(&names)).await;

    assert!(expected < 18_900, "{expected} folders fit the bytes");
    assert_eq!(index.len(), names.len() + expected);
}

#[tokio::test]
async fn folders_the_deny_list_refuses_count_toward_the_bytes_looked_at() {
    let home = git_home();
    let repo = base_repo(&home);

    // `.env` and `id_rsa` are denied names: a folder called so is refused, a file in it is not. 2,000
    // chains of such folders, told apart by their first 11 (all refused, so nothing is derived).
    let chain = |number: usize| -> String {
        let first: Vec<&str> = (0..11)
            .map(|bit| if number >> bit & 1 == 0 { ".env" } else { "id_rsa" })
            .collect();

        format!("{}/{}/f.md", first.join("/"), vec![".env"; 114].join("/"))
    };
    let mut names: Vec<String> = (0..2_000).map(chain).collect();

    // One more chain ends in a folder that is offered, 122 levels down. The refused folders above it
    // use up the bytes before the search gets there.
    names.push(format!("{}/docs/f.md", vec![".env"; 121].join("/")));
    let names: Vec<&str> = names.iter().map(String::as_str).collect();

    let index = indexed(&home, &repo.path, &Index::of(&names)).await;

    assert_eq!(
        index.len(),
        names.len(),
        "no folder is derived, the last is out of reach"
    );
}
