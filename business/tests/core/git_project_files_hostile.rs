//! `project_files` against a hostile project: names an index or a folder can hold that must never
//! reach the menu or a prompt, links that lead to private files, and listings built to cost time.
//! A hostile `.git/index` is stood in for by a runner that replaces what `git ls-files` prints.

use crate::git_support::{GitHome, git_home};

use std::fs;
use std::path::Path;
use std::time::Instant;

use super::git_project_files::{base_repo, paths, search};
use harness_business::error::Result;
use harness_business::git::ProjectFileIndex;
use harness_business::model::ProjectFile;
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ProcessRunner, TokioRunner};
use pretty_assertions::assert_eq;

/// Runs git for real, but prints `tracked` for `ls-files --cached` and nothing for the untracked
/// listing: the names a crafted index would hold.
pub(crate) struct Index {
    tracked: String,
}

impl Index {
    pub(crate) fn of(names: &[&str]) -> Self {
        Self {
            tracked: names.iter().map(|name| format!("{name}\0")).collect(),
        }
    }
}

impl ProcessRunner for Index {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        Box::pin(async move {
            let mut output = TokioRunner.run(spec).await?;
            let words: Vec<&str> = spec.args.iter().filter_map(|word| word.to_str()).collect();

            if words.contains(&"ls-files") {
                output.stdout = if words.contains(&"--cached") {
                    self.tracked.clone()
                } else {
                    String::new()
                };
            }

            Ok(output)
        })
    }
}

pub(crate) async fn indexed(home: &GitHome, project: &Path, tracked: &Index) -> ProjectFileIndex {
    ProjectFileIndex::build(&home.env, tracked, project).await.unwrap()
}

pub(crate) fn everything(index: &ProjectFileIndex, project: &Path) -> Vec<ProjectFile> {
    index.search(project, "", 50)
}

fn sorted(files: &[ProjectFile]) -> Vec<&str> {
    let mut listed = paths(files);
    listed.sort_unstable();

    listed
}

/// One name for each class of character that hides or reorders text.
const HIDDEN_NAMES: [&str; 4] = [
    "README\u{202e}gnp.exe",
    "zero\u{200b}width.md",
    "tag\u{e0049}\u{e0067}\u{e006e}.md",
    "line\u{2028}break.md",
];

#[tokio::test]
async fn bidirectional_and_invisible_characters_in_a_name_are_never_listed_from_git() {
    let home = git_home();
    let mut repo = base_repo(&home);
    for name in HIDDEN_NAMES {
        repo.write(name, "x\n");
    }
    repo.write("zażółć.md", "x\n");
    repo.commit("hostile names");

    let files = search(&home, &repo.path, "").await;

    assert!(paths(&files).contains(&"zażółć.md"), "{:?}", paths(&files));
    assert!(
        files
            .iter()
            .all(|file| !HIDDEN_NAMES.iter().any(|name| file.path.contains(name))),
        "{:?}",
        paths(&files)
    );

    for query in ["gnp.exe", "width", "break"] {
        assert!(search(&home, &repo.path, query).await.is_empty(), "{query}");
    }
}

#[tokio::test]
async fn bidirectional_and_invisible_characters_in_a_name_are_never_listed_from_a_walk() {
    let home = git_home();
    let plain = home.home.join("plain");
    fs::create_dir_all(plain.join("folder\u{200b}name")).unwrap();
    fs::write(plain.join("folder\u{200b}name/inside.md"), "x\n").unwrap();
    for name in HIDDEN_NAMES {
        fs::write(plain.join(name), "x\n").unwrap();
    }
    fs::write(plain.join("zażółć.md"), "x\n").unwrap();

    let files = search(&home, &plain, "").await;

    assert_eq!(paths(&files), ["zażółć.md"]);
}

#[tokio::test]
async fn an_index_entry_that_leaves_the_project_is_never_listed_nor_gives_a_folder() {
    let home = git_home();
    let repo = base_repo(&home);
    fs::write(home.home.join("outside.txt"), "x\n").unwrap();
    fs::create_dir_all(home.home.join("outsidedir")).unwrap();
    fs::write(home.home.join("outsidedir/report.txt"), "x\n").unwrap();

    let index = Index::of(&[
        "../outside.txt",
        "../outsidedir/report.txt",
        "src/../README.md",
        "./README.md",
        "src/./main.rs",
        "src//main.rs",
        "src/lib.rs",
    ]);
    let files = everything(&indexed(&home, &repo.path, &index).await, &repo.path);

    assert_eq!(sorted(&files), ["src", "src/lib.rs"]);
}

#[cfg(unix)]
#[tokio::test]
async fn a_backslash_in_a_name_is_never_listed_but_a_colon_is_off_windows() {
    let home = git_home();
    let mut repo = base_repo(&home);
    for name in [
        "C:\\x",
        "a\\.ssh\\config",
        "C:notes.md",
        "a:b.md",
        "c:notes",
        "notes:v2.md",
        "dir/v1:draft.md",
    ] {
        repo.write(name, "x\n");
    }
    repo.commit("odd names");

    let files = search(&home, &repo.path, "").await;
    let listed = sorted(&files);

    for kept in [
        "C:notes.md",
        "a:b.md",
        "c:notes",
        "notes:v2.md",
        "dir/v1:draft.md",
        "dir",
    ] {
        assert!(listed.contains(&kept), "{kept} is missing from {listed:?}");
    }
    for refused in ["C:\\x", "a\\.ssh\\config"] {
        assert!(!listed.contains(&refused), "{refused} is in {listed:?}");
    }
    assert!(files.iter().all(|file| !file.path.contains('\\')));
}

/// On Windows a colon is a drive (`C:x`) or an NTFS stream (`a:b`), in any component.
#[cfg(windows)]
#[tokio::test]
async fn a_colon_in_any_component_is_never_listed_on_windows() {
    let home = git_home();
    let repo = base_repo(&home);

    let index = Index::of(&["C:x", "a:b.md", "dir/c:d.md", "dir:x/e.md", "README.md"]);
    let files = everything(&indexed(&home, &repo.path, &index).await, &repo.path);

    assert_eq!(sorted(&files), ["README.md"]);
}

#[tokio::test]
async fn a_git_folder_is_never_listed_whatever_its_case_and_a_name_that_only_starts_like_it_is() {
    let home = git_home();
    let repo = base_repo(&home);
    for name in [".github/x", ".gitignore", "x/.GIT/config"] {
        repo.write(name, "x\n");
    }

    let index = Index::of(&[
        ".git/config",
        ".GIT/config",
        "x/.Git/config",
        "x/.GIT/config",
        ".git",
        ".github/x",
        ".gitignore",
    ]);
    let files = everything(&indexed(&home, &repo.path, &index).await, &repo.path);

    assert_eq!(sorted(&files), [".github", ".github/x", ".gitignore"]);
}

#[cfg(unix)]
#[tokio::test]
async fn a_name_or_a_folder_with_whitespace_at_its_edge_is_never_listed() {
    let home = git_home();
    let mut repo = base_repo(&home);
    for name in [" /compact", " lead.md", "trail.md ", "dir /inside.md", "inner space.md"] {
        repo.write(name, "x\n");
    }
    repo.commit("whitespace names");

    let files = search(&home, &repo.path, "").await;

    assert!(paths(&files).contains(&"inner space.md"), "{:?}", paths(&files));
    assert!(
        files
            .iter()
            .all(|file| file.path.split('/').all(|part| part == part.trim() && !part.is_empty())),
        "{:?}",
        paths(&files)
    );
    assert!(search(&home, &repo.path, "compact").await.is_empty());
}

#[tokio::test]
async fn a_path_with_a_huge_number_of_components_is_refused_at_once() {
    let home = git_home();
    let repo = base_repo(&home);
    let deep = vec!["a"; 5_000].join("/");
    let tracked = Index::of(&[&format!("{deep}/file"), "README.md"]);

    let started = Instant::now();
    let index = indexed(&home, &repo.path, &tracked).await;

    assert!(started.elapsed().as_secs() < 5, "{:?}", started.elapsed());
    assert_eq!(sorted(&everything(&index, &repo.path)), ["README.md"]);
    assert_eq!(index.len(), 1, "no folder is derived from a refused path");
}

#[tokio::test]
async fn a_path_over_the_length_or_depth_limit_is_refused_and_one_at_the_limit_is_kept() {
    let home = git_home();
    let repo = base_repo(&home);
    let long = format!("{}/x.md", "n".repeat(4_100));
    let too_deep = vec!["d"; 129].join("/");
    let deepest = vec!["d"; 128].join("/");

    let tracked = Index::of(&[&long, &too_deep, &deepest]);
    let index = indexed(&home, &repo.path, &tracked).await;

    // None of them exist on disk: only what the list holds is looked at.
    assert_eq!(index.len(), 128, "the 128 components and the 127 folders above them");
}

#[tokio::test]
async fn the_folders_derived_from_the_files_are_bounded_without_an_error() {
    let home = git_home();
    let repo = base_repo(&home);
    let names: Vec<String> = (0..500)
        .map(|number| format!("p{number}/{}", vec!["a"; 119].join("/")))
        .collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();

    let started = Instant::now();
    let index = indexed(&home, &repo.path, &Index::of(&names)).await;

    // 500 files and 60,000 folders would be listed without a bound.
    assert!(index.len() >= 500, "{}", index.len());
    assert!(index.len() <= 500 + 50_000, "{}", index.len());
    assert!(started.elapsed().as_secs() < 20, "{:?}", started.elapsed());
}

#[tokio::test]
async fn a_search_looks_at_a_bounded_number_of_files_on_the_disk() {
    let home = git_home();
    let mut repo = base_repo(&home);
    repo.write("zreal.txt", "x\n");
    repo.commit("the one that exists");

    let gone: Vec<String> = (0..500).map(|number| format!("a{number:04}.txt")).collect();
    let mut names: Vec<&str> = gone.iter().map(String::as_str).collect();
    names.push("zreal.txt");

    let index = indexed(&home, &repo.path, &Index::of(&names)).await;

    // The 1,000 absent files rank first and a search for one result probes 400 of them.
    assert!(index.search(&repo.path, "", 1).is_empty());
    assert_eq!(index.search(&repo.path, "", 50).len(), 1, "50 results probe 2,000");
}

#[cfg(unix)]
mod links {
    use super::{GitHome, Index, base_repo, everything, fs, git_home, indexed, search, sorted};
    use pretty_assertions::assert_eq;
    use std::os::unix::fs::symlink;
    use std::path::Path;

    /// A home with the private files of an agent and an ssh key.
    fn private_home(home: &GitHome) {
        for name in [
            ".ssh/id_rsa",
            ".ssh/config",
            ".claude/.credentials.json",
            ".codex/auth.json",
        ] {
            let path = home.home.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "FAKE\n").unwrap();
        }
    }

    /// Links, named with `prefix`, to the private files of `home` and to others inside the project.
    fn link_all(home: &GitHome, project: &Path, prefix: &str) {
        let targets = [
            ("ssh-notes.md", home.home.join(".ssh/id_rsa")),
            ("claude-notes.md", home.home.join(".claude/.credentials.json")),
            ("codex-notes.md", home.home.join(".codex/auth.json")),
            ("ssh-folder", home.home.join(".ssh")),
            ("outside.md", home.home.join("outside.txt")),
            ("inner-env.md", project.join(".env")),
            ("inner-ssh.md", project.join(".ssh/config")),
            ("dangling.md", project.join("nothing-here")),
            ("ok-link.md", project.join("real.md")),
        ];

        for (name, target) in targets {
            symlink(target, project.join(format!("{prefix}{name}"))).unwrap();
        }
    }

    #[tokio::test]
    async fn a_link_to_a_private_file_or_folder_is_never_offered_tracked_or_not() {
        let home = git_home();
        private_home(&home);
        fs::write(home.home.join("outside.txt"), "x\n").unwrap();

        let mut repo = base_repo(&home);
        repo.write("real.md", "x\n");
        repo.write(".env", "FAKE\n");
        repo.write(".ssh/config", "FAKE\n");
        link_all(&home, &repo.path, "");
        repo.commit("with links");
        link_all(&home, &repo.path, "u-");

        let files = search(&home, &repo.path, "").await;

        // Besides the files of the base repository, only the links to an ordinary file stay.
        assert_eq!(
            sorted(&files),
            [
                "README.md",
                "docs",
                "docs/guide.md",
                "ok-link.md",
                "real.md",
                "src",
                "src/lib.rs",
                "src/main.rs",
                "u-ok-link.md"
            ]
        );
    }

    #[tokio::test]
    async fn a_tracked_link_to_the_git_folder_or_its_config_is_never_offered() {
        let home = git_home();
        let mut repo = base_repo(&home);
        symlink(repo.path.join(".git/config"), repo.path.join("notes.md")).unwrap();
        symlink(repo.path.join(".git"), repo.path.join("gitdir")).unwrap();
        symlink(Path::new(".git"), repo.path.join("relative-gitdir")).unwrap();
        repo.commit("links to git");

        let files = search(&home, &repo.path, "").await;

        assert_eq!(
            sorted(&files),
            ["README.md", "docs", "docs/guide.md", "src", "src/lib.rs", "src/main.rs"]
        );
    }

    #[tokio::test]
    async fn a_folder_link_in_the_middle_of_a_path_cannot_reach_a_private_file() {
        let home = git_home();
        private_home(&home);

        let repo = base_repo(&home);
        symlink(home.home.join(".ssh"), repo.path.join("ssh-folder")).unwrap();
        symlink(repo.path.join("src"), repo.path.join("src-link")).unwrap();

        let index = Index::of(&["ssh-folder/config", "ssh-folder/known", "src-link/lib.rs", "src/lib.rs"]);
        let files = everything(&indexed(&home, &repo.path, &index).await, &repo.path);

        // A link inside the project to a path that is offered stays offered.
        assert_eq!(sorted(&files), ["src", "src-link", "src-link/lib.rs", "src/lib.rs"]);
    }
}
