//! `project_files` names: the invisible characters that ordinary names hold (emoji, keycaps, Persian
//! and Hebrew words) are listed where they shape visible text, and refused everywhere else. A
//! variation selector follows an emoji or nothing, and a path holds at most 16 of these characters.

use crate::git_support::git_home;

use std::fs;

use super::git_project_files::{base_repo, paths, search};
use pretty_assertions::assert_eq;

/// Names that hold a variation selector, a joiner, a non-joiner or a direction mark in their place.
const KEPT: [&str; 12] = [
    "\u{2764}\u{fe0f}.md",
    "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}.md",
    "\u{2764}\u{fe0f}\u{200d}\u{1f525}.md",
    "\u{0645}\u{06cc}\u{200c}\u{062e}\u{0648}\u{0627}\u{0647}\u{0645}.md",
    "\u{0915}\u{094d}\u{200c}\u{0937}.md",
    "\u{05e9}\u{05dc}\u{05d5}\u{05dd}\u{200f}.md",
    "\u{200e}\u{05e9}.md",
    "\u{200f}\u{05e9}\u{05dc}.md",
    "1\u{fe0f}\u{20e3} intro.md",
    "#\u{fe0f}\u{20e3}.md",
    "\u{2764}\u{fe0e}.md",
    "\u{a9}\u{fe0f} notes.md",
];

/// Names that hide something: each is refused for the character in it.
const REFUSED: [&str; 21] = [
    "a\u{200d}b.md",
    "caf\u{e9}\u{200c}",
    "\u{1f468}\u{200d}\u{200d}\u{1f469}.md",
    "\u{0645}\u{200c}a.md",
    "\u{200d}\u{1f525}.md",
    "a\u{fe0f}.md",
    "\u{2764}\u{fe0f}\u{fe0f}.md",
    "\u{05e9}\u{200f}\u{200f}.md",
    "README\u{202e}gnp.exe",
    "zero\u{200b}width.md",
    "tag\u{e0049}\u{e0067}.md",
    "\u{2764}\u{e0100}.md",
    "ab\u{200e}cd.md",
    "\u{65e5}\u{fe01}\u{672c}\u{fe02}\u{8a9e}\u{fe03}.md",
    "\u{65e5}\u{fe0f}.md",
    "\u{65e5}\u{fe0e}.md",
    "\u{2764}\u{fe01}.md",
    "1\u{fe0f} intro.md",
    "1\u{fe0e}\u{20e3}.md",
    "x\u{fe0f}\u{20e3}.md",
    "\u{fe0f}.md",
];

/// A kept folder name with a file in it.
const KEPT_FOLDER: &str = "\u{2764}\u{fe0f}";

#[cfg(unix)]
#[tokio::test]
async fn names_with_ordinary_invisible_characters_are_listed_from_git_and_hidden_ones_are_not() {
    let home = git_home();
    let mut repo = base_repo(&home);
    for name in KEPT.iter().chain(&REFUSED) {
        repo.write(name, "x\n");
    }
    repo.write(&format!("{KEPT_FOLDER}/inside.md"), "x\n");
    repo.commit("names");

    let files = search(&home, &repo.path, "").await;
    let listed = paths(&files);

    for kept in KEPT.iter().copied().chain([KEPT_FOLDER, "\u{2764}\u{fe0f}/inside.md"]) {
        assert!(listed.contains(&kept), "{kept:?} is missing from {listed:?}");
    }
    for refused in REFUSED {
        assert!(!listed.contains(&refused), "{refused:?} is in {listed:?}");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn names_with_ordinary_invisible_characters_are_listed_from_a_walk_and_hidden_ones_are_not() {
    let home = git_home();
    let plain = home.home.join("plain");
    fs::create_dir_all(plain.join(KEPT_FOLDER)).unwrap();
    fs::write(plain.join(KEPT_FOLDER).join("inside.md"), "x\n").unwrap();
    for name in KEPT.iter().chain(&REFUSED) {
        fs::write(plain.join(name), "x\n").unwrap();
    }

    let files = search(&home, &plain, "").await;
    let mut listed = paths(&files);
    listed.sort_unstable();

    let mut expected: Vec<&str> = KEPT.to_vec();
    expected.extend([KEPT_FOLDER, "\u{2764}\u{fe0f}/inside.md"]);
    expected.sort_unstable();

    assert_eq!(listed, expected);
}

/// The characters a path may hold at most 16 of: a name of `count` emoji with a selector each.
fn emoji_name(count: usize) -> String {
    format!("{}.md", "\u{2764}\u{fe0f}".repeat(count))
}

#[cfg(unix)]
#[tokio::test]
async fn a_path_holds_at_most_sixteen_of_the_characters_that_shape_text() {
    let home = git_home();
    let mut repo = base_repo(&home);
    repo.write(&emoji_name(16), "x\n");
    repo.write(&emoji_name(17), "x\n");
    repo.write(&format!("{}/{}", "\u{2764}\u{fe0f}".repeat(9), emoji_name(8)), "x\n");
    repo.write(&format!("{}/{}", "\u{2764}\u{fe0f}".repeat(9), emoji_name(7)), "x\n");
    repo.commit("many selectors");

    let files = search(&home, &repo.path, "").await;
    let listed = paths(&files);
    let folder = "\u{2764}\u{fe0f}".repeat(9);

    assert!(listed.contains(&emoji_name(16).as_str()), "16 are kept");
    assert!(!listed.contains(&emoji_name(17).as_str()), "17 are refused");
    assert!(
        listed.contains(&format!("{folder}/{}", emoji_name(7)).as_str()),
        "16 in all are kept"
    );
    assert!(
        !listed.contains(&format!("{folder}/{}", emoji_name(8)).as_str()),
        "17 in all are refused"
    );
}
