//! Where Harness Skills keeps its data files, under the name of 0.4.0 and newer
//! ([`SKILLS_DIR`]) and the one before ([`LEGACY_SKILLS_DIR`]).
//!
//! The rule for reading: the new path wins, and the old path is used only when the new one does
//! not exist (a dangling or symlinked entry counts as existing, so its error is shown instead of
//! a silent fallback). Re-running the Harness Skills installer moves the data to the new path.
//! The app itself never writes these files. An editor added later must write the new path when
//! the skills install is new (its install record is under the new name) and the old path when
//! only the old record exists, so that the installer's migration finds one copy.

use std::fs;
use std::path::{Path, PathBuf};

use super::catalog::{LEGACY_SKILLS_DIR, SKILLS_DIR};

/// `<root>/harness-useful-skills/<rest…>`, or `<root>/useful-skills/<rest…>` when only that
/// exists.
pub fn skills_data_path(root: &Path, rest: &[&str]) -> PathBuf {
    let current = join(root, SKILLS_DIR, rest);

    if exists(&current) {
        return current;
    }

    let legacy = join(root, LEGACY_SKILLS_DIR, rest);

    if exists(&legacy) { legacy } else { current }
}

/// True when `path` is there, whatever it is (a symlink counts, even a dangling one).
pub fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn join(root: &Path, dir: &str, rest: &[&str]) -> PathBuf {
    rest.iter().fold(root.join(dir), |path, part| path.join(part))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "modelRoles:\n").unwrap();
    }

    #[test]
    fn neither_path_resolves_to_the_new_name() {
        let root = tempfile::tempdir().unwrap();

        assert_eq!(
            skills_data_path(root.path(), &["models.yml"]),
            root.path().join("harness-useful-skills").join("models.yml")
        );
    }

    #[test]
    fn only_the_old_path_is_used_when_it_alone_exists() {
        let root = tempfile::tempdir().unwrap();
        touch(&root.path().join("useful-skills").join("models.yml"));

        assert_eq!(
            skills_data_path(root.path(), &["models.yml"]),
            root.path().join("useful-skills").join("models.yml")
        );
    }

    #[test]
    fn the_new_path_wins_when_both_exist_or_only_it_does() {
        let root = tempfile::tempdir().unwrap();
        let new = root.path().join("harness-useful-skills").join("models.yml");

        touch(&new);
        assert_eq!(skills_data_path(root.path(), &["models.yml"]), new);

        touch(&root.path().join("useful-skills").join("models.yml"));
        assert_eq!(skills_data_path(root.path(), &["models.yml"]), new);
    }

    #[test]
    fn each_item_is_resolved_on_its_own() {
        let root = tempfile::tempdir().unwrap();
        touch(&root.path().join("harness-useful-skills").join("models.yml"));
        touch(
            &root
                .path()
                .join("useful-skills")
                .join("workflow-settings")
                .join("plan.json"),
        );

        assert_eq!(
            skills_data_path(root.path(), &["workflow-settings"]),
            root.path().join("useful-skills").join("workflow-settings")
        );
    }
}
