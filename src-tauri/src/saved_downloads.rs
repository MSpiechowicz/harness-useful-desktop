//! The files the user saved with the native save dialog (`knowledge_doc_download`), so "Show in
//! folder" can reveal them. A download lands wherever the user chose, usually outside every folder
//! [`crate::access::reveal_target`] allows, and those folders are not widened for it: only the exact
//! paths the dialog returned are revealed (`knowledge_reveal_download`).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use harness_business::BusinessError;

use crate::error::CommandResult;

/// How many saved files are remembered: the oldest is forgotten first.
const KEPT: usize = 16;

/// The paths the save dialog returned for the latest downloads.
#[derive(Debug, Default)]
pub struct SavedDownloads(Mutex<VecDeque<PathBuf>>);

impl SavedDownloads {
    /// Remembers `path` as returned by the save dialog.
    pub fn remember(&self, path: &Path) {
        let mut kept = self.0.lock().unwrap_or_else(PoisonError::into_inner);

        kept.retain(|saved| saved != path);
        kept.push_back(path.to_path_buf());

        while kept.len() > KEPT {
            kept.pop_front();
        }
    }

    /// Remembers what a download wrote: the `chosen` path of the dialog, and the `printed` path the sidecar says it wrote when
    /// that is another file of the same folder. The stored text of a document whose original was not kept is written as `x.md` or
    /// `x.txt` next to the chosen `x.pdf`, so only that spelling is trusted: the same folder, the same name without its
    /// extension and a `.md` or `.txt` extension. Any other printed path is never remembered.
    pub fn remember_download(&self, chosen: &Path, printed: &str) {
        self.remember(chosen);

        let printed = Path::new(printed);

        if is_text_beside(chosen, printed) {
            self.remember(printed);
        }
    }

    /// `raw` when it is exactly a remembered path and the file is still there. A path that was
    /// never remembered is `forbidden`, whether it exists or not, so the command cannot be used to
    /// probe the disk; a remembered file that is gone is `notFound`.
    pub fn reveal_target(&self, raw: &Path) -> CommandResult<PathBuf> {
        let remembered = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .any(|saved| saved == raw);

        if !remembered {
            return Err(BusinessError::Forbidden("refused by the access rules".to_owned()).into());
        }

        if !raw.exists() {
            return Err(BusinessError::NotFound(raw.display().to_string()).into());
        }

        Ok(raw.to_path_buf())
    }
}

/// Whether `printed` is the stored-text file the sidecar writes next to the file the user chose: the same folder, the same
/// file name without its extension, and `md` or `txt` as its extension.
fn is_text_beside(chosen: &Path, printed: &Path) -> bool {
    let (Some(folder), Some(printed_folder)) = (chosen.parent(), printed.parent()) else {
        return false;
    };

    folder == printed_folder
        && chosen.file_stem().is_some_and(|stem| printed.file_stem() == Some(stem))
        && printed
            .extension()
            .is_some_and(|extension| extension == "md" || extension == "txt")
}

#[cfg(test)]
mod tests {
    use harness_business::model::IpcErrorCode;

    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn only_a_path_the_dialog_returned_is_revealed() {
        let dir = TempDir::new("saved-downloads");
        let saved = dir.path().join("Downloads").join("guide.md");
        let other = dir.path().join("Downloads").join("other.md");
        std::fs::create_dir_all(saved.parent().unwrap()).unwrap();
        std::fs::write(&saved, "# Guide").unwrap();
        std::fs::write(&other, "# Other").unwrap();

        let downloads = SavedDownloads::default();
        let before = downloads.reveal_target(&saved).expect_err("nothing was saved yet");
        assert_eq!(before.code(), IpcErrorCode::Forbidden);

        downloads.remember(&saved);

        assert_eq!(downloads.reveal_target(&saved).expect("the saved file"), saved);

        let beside = downloads
            .reveal_target(&other)
            .expect_err("a sibling in the same folder");
        let folder = downloads
            .reveal_target(saved.parent().unwrap())
            .expect_err("the folder itself");
        let escaping = downloads
            .reveal_target(
                &dir.path()
                    .join("Downloads")
                    .join("..")
                    .join("Downloads")
                    .join("guide.md"),
            )
            .expect_err("a dot-dot spelling");
        let relative = downloads.reveal_target(Path::new("guide.md")).expect_err("relative");

        for error in [beside, folder, escaping, relative] {
            assert_eq!(error.code(), IpcErrorCode::Forbidden);
        }
    }

    #[test]
    fn a_saved_file_that_is_gone_is_not_found_and_a_stranger_is_forbidden_either_way() {
        let dir = TempDir::new("saved-downloads-gone");
        let saved = dir.path().join("gone.md");
        let downloads = SavedDownloads::default();

        downloads.remember(&saved);

        let gone = downloads.reveal_target(&saved).expect_err("removed since");
        let stranger = downloads
            .reveal_target(&dir.path().join("never.md"))
            .expect_err("never saved");

        assert_eq!(gone.code(), IpcErrorCode::NotFound);
        assert_eq!(stranger.code(), IpcErrorCode::Forbidden);
    }

    #[test]
    fn the_oldest_saved_paths_are_forgotten_first() {
        let dir = TempDir::new("saved-downloads-kept");
        let downloads = SavedDownloads::default();
        let files: Vec<PathBuf> = (0..=KEPT)
            .map(|number| dir.path().join(format!("{number}.md")))
            .collect();

        for file in &files {
            std::fs::write(file, "x").unwrap();
            downloads.remember(file);
        }

        assert_eq!(
            downloads.reveal_target(&files[0]).expect_err("forgotten").code(),
            IpcErrorCode::Forbidden
        );
        assert!(downloads.reveal_target(&files[KEPT]).is_ok());
        assert!(downloads.reveal_target(&files[1]).is_ok());
    }

    #[test]
    fn the_stored_text_written_beside_the_chosen_file_is_revealed_and_no_other_printed_path_is() {
        let dir = TempDir::new("saved-downloads-text");
        let folder = dir.path().join("Downloads");
        let elsewhere = dir.path().join("Other");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::create_dir_all(&elsewhere).unwrap();

        let chosen = folder.join("x.pdf");
        let text = folder.join("x.md");
        let plain = folder.join("x.txt");
        let hostile = [
            folder.join("y.md"),
            folder.join("x.sh"),
            folder.join("x.md.sh"),
            folder.join("x"),
            folder.join("x.MD"),
            elsewhere.join("x.md"),
            folder.join("sub").join("x.md"),
            folder.join("..").join("Downloads").join("x.md"),
            folder.clone(),
            PathBuf::from("x.md"),
        ];

        for file in [&text, &plain] {
            std::fs::write(file, "x").unwrap();
        }

        let downloads = SavedDownloads::default();

        downloads.remember_download(&chosen, text.to_str().unwrap());
        downloads.remember_download(&chosen, plain.to_str().unwrap());

        assert_eq!(downloads.reveal_target(&text).expect("the stored text"), text);
        assert_eq!(downloads.reveal_target(&plain).expect("the stored text"), plain);

        let strangers = SavedDownloads::default();

        for file in &hostile {
            strangers.remember_download(&chosen, file.to_str().unwrap());

            assert_eq!(
                strangers
                    .reveal_target(file)
                    .expect_err("not what the dialog led to")
                    .code(),
                IpcErrorCode::Forbidden,
                "{}",
                file.display()
            );
        }
    }
}
