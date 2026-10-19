//! The example notes file the user can fill in before an import (`memory_notes_import_template`),
//! written only where the user chose in a native save dialog, as "Share setup" saves a setup file
//! (see [`super::sync`]).

use std::path::Path;

use harness_business::ecosystem::import_template;
use harness_business::fs_atomic::replace_atomically;
use harness_business::model::NotesImportFormat;
use tauri::{AppHandle, State};

use super::{blocking, saved_language};
use crate::dialogs::{self, FileFilter};
use crate::error::CommandResult;
use crate::native_text::native_text;
use crate::state::AppState;

/// Writes an example notes file in `format` (three notes that import with no problems) where the
/// user chooses in a native save dialog (`notes-example.csv` or `notes-example.json` by default,
/// opened in the home folder). The path written, or `None` when the user cancelled. `io` when the
/// file cannot be written.
#[tauri::command]
pub async fn memory_notes_import_template(
    app: AppHandle,
    state: State<'_, AppState>,
    format: NotesImportFormat,
) -> CommandResult<Option<String>> {
    let text = native_text(&saved_language(&state).await?);
    let (default_name, filter) = match format {
        NotesImportFormat::Csv => (
            "notes-example.csv",
            FileFilter {
                name: text.csv_files,
                extensions: &["csv"],
            },
        ),
        NotesImportFormat::Json => (
            "notes-example.json",
            FileFilter {
                name: text.json_files,
                extensions: &["json"],
            },
        ),
    };

    let chosen = dialogs::save(
        &app,
        &state.dialog,
        text.save_notes_example,
        Some(&state.env.home),
        default_name,
        filter,
    )
    .await?;
    let Some(path) = chosen else {
        return Ok(None);
    };

    let saved = blocking(move || write_template(&path, format)).await?;

    tracing::info!(?format, "notes import example saved");

    Ok(Some(saved))
}

/// Replaces `target` with the example in `format`; the path written.
fn write_template(target: &Path, format: NotesImportFormat) -> CommandResult<String> {
    replace_atomically(
        target,
        import_template(format).as_bytes(),
        "saving the example notes file",
    )?;

    Ok(target.display().to_string())
}

#[cfg(test)]
mod tests {
    use harness_business::ecosystem::read_notes_file;

    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn a_saved_example_replaces_the_target_and_reads_as_three_notes() {
        let dir = TempDir::new("notes-template");

        for (name, format) in [
            ("notes-example.csv", NotesImportFormat::Csv),
            ("notes-example.json", NotesImportFormat::Json),
        ] {
            let target = dir.path().join(name);
            std::fs::write(&target, "an older, longer file that the example replaces whole").expect("write old file");

            let saved = write_template(&target, format).expect("saved");
            assert_eq!(saved, target.display().to_string());

            let bytes = std::fs::read(&target).expect("read");
            let read = read_notes_file(name, &bytes, Some(dir.path())).expect("a notes file");

            assert_eq!(read.plan.format, format);
            assert_eq!(read.plan.rows.len(), 3, "{name}");
            assert!(read.plan.rows.iter().all(|row| row.problems.is_empty()), "{name}");
        }

        let entries = std::fs::read_dir(dir.path()).expect("list").count();
        assert_eq!(entries, 2, "no temporary file is left behind");
    }
}
