//! The example file the Notes view offers to fill in before an import
//! (`memory_notes_import_template`): three notes that read with no problems, one of each kind of
//! scope (a repository note with tags, a global how-to tip, and one without a scope or tags, which
//! gets the dialog's default).

use serde::Serialize;

use crate::model::NotesImportFormat;

/// One note of the example, its fields in the order the file shows them.
#[derive(Serialize)]
struct ExampleNote {
    title: &'static str,
    body: &'static str,
    kind: &'static str,
    tags: &'static [&'static str],
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<&'static str>,
}

const EXAMPLE_NOTES: [ExampleNote; 3] = [
    ExampleNote {
        title: "File watcher misses renamed folders on Linux",
        body: "inotify reports a moved folder as one rename event, not one per file.\n\
               Rescan the folder after a rename to pick up its files.",
        kind: "problem",
        tags: &["watcher", "linux"],
        scope: Some("repo"),
    },
    ExampleNote {
        title: "Show a Rust test's output",
        body: "Run cargo test <name> -- --nocapture to see what the test prints.",
        kind: "note",
        tags: &["rust", "testing"],
        scope: Some("global"),
    },
    ExampleNote {
        title: "Use bun for the UI scripts",
        body: "The UI is built and tested with bun, so run bun install and bun run test instead of npm.",
        kind: "decision",
        tags: &[],
        scope: None,
    },
];

/// The JSON file's shape: the notes under `notes`.
#[derive(Serialize)]
struct ExampleFile<'a> {
    notes: &'a [ExampleNote],
}

/// An example notes file in `format` the user can fill in: CSV with the header
/// `title,body,kind,tags,scope` (the tags of a note in one quoted cell, one body over two lines),
/// or pretty-printed JSON `{"notes": [...]}` with the tags as arrays. Ends with a line break.
pub fn import_template(format: NotesImportFormat) -> String {
    match format {
        NotesImportFormat::Csv => csv_template(),
        NotesImportFormat::Json => json_template(),
    }
}

fn csv_template() -> String {
    let mut writer = csv::Writer::from_writer(Vec::new());

    write_csv_record(&mut writer, ["title", "body", "kind", "tags", "scope"]);

    for note in &EXAMPLE_NOTES {
        let tags = note.tags.join(",");
        let scope = note.scope.unwrap_or_default();

        write_csv_record(&mut writer, [note.title, note.body, note.kind, &tags, scope]);
    }

    let bytes = writer.into_inner().expect("writing to memory cannot fail");

    String::from_utf8(bytes).expect("the example is UTF-8")
}

fn write_csv_record(writer: &mut csv::Writer<Vec<u8>>, record: [&str; 5]) {
    writer.write_record(record).expect("writing to memory cannot fail");
}

fn json_template() -> String {
    let file = ExampleFile { notes: &EXAMPLE_NOTES };
    let mut json = serde_json::to_string_pretty(&file).expect("the example serialises");

    json.push('\n');
    json
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::ecosystem::memory_notes_import::read_notes_file;
    use crate::model::{NoteKind, ScriptScope};

    /// The notes the import reads from the example in `format`, as (title, kind, tags, scope).
    fn read_back(
        format: NotesImportFormat,
        file_name: &str,
    ) -> Vec<(String, NoteKind, Vec<String>, Option<ScriptScope>)> {
        let template = import_template(format);
        let read = read_notes_file(file_name, template.as_bytes(), Some(Path::new("/work/app"))).expect("a notes file");

        assert_eq!(read.plan.format, format);
        assert!(!read.plan.truncated);

        read.plan
            .rows
            .into_iter()
            .map(|row| {
                assert_eq!(row.problems, Vec::new(), "{}", row.title);
                assert_eq!(row.duplicate_of, None, "{}", row.title);

                (row.title, row.kind, row.tags, row.scope)
            })
            .collect()
    }

    fn expected() -> Vec<(String, NoteKind, Vec<String>, Option<ScriptScope>)> {
        let tags = |tags: &[&str]| tags.iter().map(|tag| (*tag).to_owned()).collect();

        vec![
            (
                "File watcher misses renamed folders on Linux".to_owned(),
                NoteKind::Problem,
                tags(&["watcher", "linux"]),
                Some(ScriptScope::Repo),
            ),
            (
                "Show a Rust test's output".to_owned(),
                NoteKind::Note,
                tags(&["rust", "testing"]),
                Some(ScriptScope::Global),
            ),
            (
                "Use bun for the UI scripts".to_owned(),
                NoteKind::Decision,
                Vec::new(),
                None,
            ),
        ]
    }

    #[test]
    fn the_csv_example_imports_three_notes_without_problems() {
        assert_eq!(read_back(NotesImportFormat::Csv, "notes-example.csv"), expected());
    }

    #[test]
    fn the_json_example_imports_three_notes_without_problems() {
        assert_eq!(read_back(NotesImportFormat::Json, "notes-example.json"), expected());
    }

    #[test]
    fn the_csv_example_shows_the_header_quoted_tags_and_a_multi_line_body() {
        let template = import_template(NotesImportFormat::Csv);

        assert!(template.starts_with("title,body,kind,tags,scope\n"), "{template}");
        assert!(template.contains(",\"watcher,linux\",repo\n"), "{template}");
        assert!(template.contains("one per file.\nRescan"), "{template}");
        assert!(template.ends_with(",decision,,\n"), "{template}");
    }

    #[test]
    fn the_json_example_is_pretty_with_tags_as_arrays() {
        let template = import_template(NotesImportFormat::Json);
        let value: serde_json::Value = serde_json::from_str(&template).expect("JSON");

        assert!(template.starts_with("{\n  \"notes\": [\n"), "{template}");
        assert_eq!(value["notes"][0]["tags"], serde_json::json!(["watcher", "linux"]));
        assert_eq!(value["notes"][2]["tags"], serde_json::json!([]));
        assert!(template.ends_with("}\n"));
    }
}
