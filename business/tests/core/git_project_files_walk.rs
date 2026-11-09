//! `project_files` in a folder that is no git repository: the bounded walk.

use crate::git_support::git_home;

use std::fs;

use super::git_project_files::{paths, search};
use harness_business::git::ProjectFileIndex;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn a_folder_without_a_repository_is_walked_within_its_limits() {
    let home = git_home();
    let plain = home.home.join("plain");

    for name in [
        "readme.md",
        "src/app.rs",
        "src/deep/mod.rs",
        "node_modules/pkg/index.js",
        "target/debug/out.o",
        "dist/bundle.js",
        "build/out.txt",
        ".venv/lib/site.py",
        "__pycache__/mod.pyc",
        ".git/config",
        "src/target/nested.rs",
    ] {
        let path = plain.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "x\n").unwrap();
    }

    let files = search(&home, &plain, "").await;
    let mut listed = paths(&files);
    listed.sort_unstable();

    assert_eq!(
        listed,
        ["readme.md", "src", "src/app.rs", "src/deep", "src/deep/mod.rs"]
    );
    assert!(files.iter().all(|file| !file.changed));

    // Nothing here is a tracked or changed file: a query finds by name.
    assert_eq!(paths(&search(&home, &plain, "mod.rs").await), ["src/deep/mod.rs"]);
}

#[tokio::test]
async fn the_walk_never_lists_the_files_of_a_ssh_folder() {
    let home = git_home();
    let plain = home.home.join("plain");

    for name in [
        ".ssh/config",
        ".ssh/known_hosts",
        "deploy/.SSH/authorized_keys",
        "readme.md",
    ] {
        let path = plain.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "x\n").unwrap();
    }

    for query in ["", "config", "known_hosts", "authorized_keys", "ssh", "deploy"] {
        let files = search(&home, &plain, query).await;
        let listed: Vec<&str> = paths(&files).into_iter().filter(|path| *path != "readme.md").collect();

        assert!(listed.is_empty(), "{listed:?} for {query:?}");
    }
}

#[tokio::test]
async fn the_walk_stops_at_twelve_folders_deep() {
    let home = git_home();
    let plain = home.home.join("plain");
    let twelve = (1..=12).map(|level| format!("d{level}")).collect::<Vec<_>>().join("/");
    let thirteen = format!("{twelve}/d13");
    fs::create_dir_all(plain.join(&thirteen)).unwrap();
    fs::write(plain.join(&twelve).join("inside.txt"), "x\n").unwrap();
    fs::write(plain.join(&thirteen).join("beyond.txt"), "x\n").unwrap();

    assert_eq!(
        paths(&search(&home, &plain, "inside").await),
        [format!("{twelve}/inside.txt")]
    );
    assert!(search(&home, &plain, "beyond").await.is_empty());
}

#[tokio::test]
async fn the_walk_looks_at_a_bounded_number_of_entries_and_does_not_fail_at_the_limit() {
    let home = git_home();
    let plain = home.home.join("plain");
    fs::create_dir_all(plain.join("many")).unwrap();
    for number in 0..20_100 {
        fs::write(plain.join("many").join(format!("f{number}.txt")), "").unwrap();
    }
    fs::write(plain.join("top.txt"), "x\n").unwrap();

    let index = ProjectFileIndex::build(&home.env, &home.runner, &plain).await.unwrap();

    assert!(index.len() <= 20_000, "{}", index.len());
    assert!(index.len() > 19_000, "{}", index.len());
    assert_eq!(paths(&index.search(&plain, "top.txt", 5)), ["top.txt"]);
}

#[cfg(unix)]
#[tokio::test]
async fn the_walk_never_follows_or_lists_a_symbolic_link_and_skips_names_that_are_not_text() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let home = git_home();
    let plain = home.home.join("plain");
    fs::create_dir_all(plain.join("real")).unwrap();
    fs::write(plain.join("real/inner.txt"), "x\n").unwrap();
    std::os::unix::fs::symlink("real", plain.join("link")).unwrap();
    std::os::unix::fs::symlink("real/inner.txt", plain.join("file-link.txt")).unwrap();
    fs::write(plain.join(OsStr::from_bytes(b"raw\xffname.txt")), "x\n").unwrap();

    let files = search(&home, &plain, "").await;
    let mut listed = paths(&files);
    listed.sort_unstable();

    assert_eq!(listed, ["real", "real/inner.txt"]);
}
