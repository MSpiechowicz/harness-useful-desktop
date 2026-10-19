use pretty_assertions::assert_eq;

use super::*;
use crate::model::{NotesImportFileProblem, NotesImportFormat};

const PROJECT: &str = "/work/app";

fn read(file_name: &str, text: &str) -> NotesImportRead {
    read_notes_file(file_name, text.as_bytes(), Some(Path::new(PROJECT))).expect("a notes file")
}

fn titles(read: &NotesImportRead) -> Vec<&str> {
    read.plan.rows.iter().map(|row| row.title.as_str()).collect()
}

fn problems(row: &NotesImportRow) -> Vec<(NotesImportProblemCode, Option<NotesImportField>)> {
    row.problems.iter().map(|found| (found.code, found.field)).collect()
}

fn refusal(file_name: &str, bytes: &[u8]) -> NotesImportFileProblem {
    match read_notes_file(file_name, bytes, None) {
        Err(FileRefusal { reason, line: None }) => reason,
        other => panic!("expected a refusal without a line, got {other:?}"),
    }
}

#[test]
fn a_csv_file_is_read_with_its_columns_in_any_case_and_order() {
    let read = read(
        "notes.csv",
        "\u{feff}Scope,TITLE,Body,kind,tags,extra\n\
         global,Prefer bun,Use bun over npm.,Decision,\"tooling, js\",x\n\
         ,Cache the probe,It is slow.,,,\n",
    );

    assert_eq!(read.plan.format, NotesImportFormat::Csv);
    assert_eq!(read.plan.file_name, "notes.csv");
    assert!(!read.plan.truncated);
    assert_eq!(read.plan.plan_id, read.stored.id());
    assert_eq!(read.stored.project(), Some(Path::new(PROJECT)));

    let first = &read.plan.rows[0];
    assert_eq!(first.index, 0);
    assert_eq!(
        first.title, "Prefer bun",
        "the byte order mark is not part of the header"
    );
    assert_eq!(first.kind, NoteKind::Decision);
    assert_eq!(first.tags, ["tooling", "js"]);
    assert_eq!(first.scope, Some(ScriptScope::Global));
    assert_eq!(first.body_preview, "Use bun over npm.");
    assert!(first.problems.is_empty());

    let second = &read.plan.rows[1];
    assert_eq!(second.kind, NoteKind::Note, "no kind is a plain note");
    assert_eq!(second.scope, None, "no scope takes the dialog's default");
    assert!(second.tags.is_empty());
}

#[test]
fn the_separator_comes_from_the_header_line() {
    let semicolons = read("notes.csv", "title;body;tags\nA, B;Text, with commas;x,y\n");
    assert_eq!(titles(&semicolons), ["A, B"]);
    assert_eq!(semicolons.plan.rows[0].tags, ["x", "y"]);

    let tabs = read("notes.tsv", "title\tbody\nOne\tFirst; second\n");
    assert_eq!(titles(&tabs), ["One"]);
    assert_eq!(tabs.plan.rows[0].body_preview, "First; second");

    let quoted = read("notes.csv", "\"a;b;c\",title,body\nx,A,B\n");
    assert_eq!(titles(&quoted), ["A"], "a separator inside quotes does not count");
}

#[test]
fn quoted_fields_hold_separators_quotes_and_line_breaks() {
    let read = read(
        "notes.csv",
        "title,body\r\n\"Say \"\"hi\"\"\",\"Line one\r\nline two, still body\"\r\n\r\nNext,Short\r\n",
    );

    assert_eq!(titles(&read), ["Say \"hi\"", "Next"], "the blank record is skipped");
    assert_eq!(read.plan.rows[0].body_preview, "Line one line two, still body");
    assert_eq!(read.stored.rows[0].body, "Line one\r\nline two, still body");
}

#[test]
fn a_csv_file_without_a_title_or_a_body_column_is_refused() {
    assert_eq!(
        refusal("notes.csv", b"name,body\nA,B\n"),
        NotesImportFileProblem::MissingColumns
    );
    assert_eq!(
        refusal("notes.csv", b"title\nA\n"),
        NotesImportFileProblem::MissingColumns
    );
}

#[test]
fn a_file_without_notes_is_empty() {
    for (file_name, text) in [
        ("notes.csv", &b""[..]),
        ("notes.csv", b"\xef\xbb\xbf \n\n"),
        ("notes.csv", b"title,body\n , \n"),
        ("notes.json", b"[]"),
        ("notes.json", br#"{"notes":[]}"#),
    ] {
        assert_eq!(refusal(file_name, text), NotesImportFileProblem::Empty, "{text:?}");
    }
}

#[test]
fn kinds_scopes_and_tags_are_read_loosely_and_checked() {
    let read = read(
        "notes.csv",
        "title,body,kind,scope,tags\n\
         A,b,OPTIMIZATION,Repository,Perf|Cache  CI_2\n\
         B,b,optimisation,project,\n\
         C,b,idea,everywhere,\n\
         D,b,fix,GLOBAL,\"a,b,c,d,e,f,g,h,i\"\n",
    );
    let rows = &read.plan.rows;

    assert_eq!(rows[0].kind, NoteKind::Optimisation);
    assert_eq!(rows[0].scope, Some(ScriptScope::Repo));
    assert_eq!(rows[0].tags, ["perf", "cache", "ci_2"]);
    assert!(rows[0].problems.is_empty());
    assert_eq!(rows[1].kind, NoteKind::Optimisation);
    assert_eq!(rows[1].scope, Some(ScriptScope::Repo));

    assert_eq!(rows[2].kind, NoteKind::Note);
    assert_eq!(rows[2].scope, None);
    assert_eq!(
        problems(&rows[2]),
        [
            (NotesImportProblemCode::UnknownKind, Some(NotesImportField::Kind)),
            (NotesImportProblemCode::UnknownScope, Some(NotesImportField::Scope)),
        ]
    );

    assert_eq!(rows[3].scope, Some(ScriptScope::Global));
    assert_eq!(
        problems(&rows[3]),
        [(NotesImportProblemCode::TooMany, Some(NotesImportField::Tags))]
    );
    assert_eq!(
        rows[3].tags,
        ["a", "b", "c", "d", "e", "f", "g", "h"],
        "the review shows the tags a note can keep"
    );
}

#[test]
fn a_tag_with_other_characters_is_a_problem_not_split() {
    let read = read(
        "notes.csv",
        "title,body,tags\n\
         A,b,perf/cache\n\
         B,b,\"Größe; ci\"\n\
         C,b,\"#build,ok\"\n\
         D,b,\"One;TWO|three  four,five\"\n",
    );
    let rows = &read.plan.rows;
    let invalid = [(NotesImportProblemCode::InvalidChars, Some(NotesImportField::Tags))];

    assert_eq!(rows[0].tags, ["perf/cache"]);
    assert_eq!(problems(&rows[0]), invalid);
    assert_eq!(rows[1].tags, ["größe", "ci"], "lowercased, not cut into pieces");
    assert_eq!(problems(&rows[1]), invalid);
    assert_eq!(rows[2].tags, ["#build", "ok"]);
    assert_eq!(problems(&rows[2]), invalid);

    assert_eq!(rows[3].tags, ["one", "two", "three", "four", "five"]);
    assert!(rows[3].problems.is_empty());
    assert!(!read.stored.rows[1].importable);
}

#[test]
fn a_row_with_a_huge_number_of_tags_is_read_in_bounded_time() {
    let tags: Vec<String> = (0..250_000).map(|tag| format!("t{tag}")).collect();
    let text = format!("title,body,tags\nA,b,{}\n", tags.join(" "));
    assert!(text.len() < MAX_IMPORT_BYTES);

    let started = std::time::Instant::now();
    let read = read("notes.csv", &text);
    let elapsed = started.elapsed();

    let row = &read.plan.rows[0];
    assert_eq!(row.tags.len(), TAGS_MAX);
    assert_eq!(
        problems(row),
        [(NotesImportProblemCode::TooMany, Some(NotesImportField::Tags))]
    );
    assert!(elapsed < std::time::Duration::from_secs(1), "took {elapsed:?}");
}

#[test]
fn rows_over_a_limit_have_problems_and_cannot_be_imported() {
    let long_tag = "t".repeat(TAG_MAX + 1);
    let text = format!(
        "title,body,tags\n\
         ,body,\n\
         {},body,\n\
         \"two\nlines\",body,\n\
         Long body,{},\n\
         No body, ,\n\
         Long tag,body,{long_tag}\n\
         {},{},{}\n",
        "t".repeat(TITLE_MAX + 1),
        "b".repeat(BODY_MAX + 1),
        "t".repeat(TITLE_MAX),
        "b".repeat(BODY_MAX),
        "t".repeat(TAG_MAX),
    );
    let read = read("notes.csv", &text);
    let found: Vec<_> = read.plan.rows.iter().map(problems).collect();

    use NotesImportField::{Body, Tags, Title};
    use NotesImportProblemCode::{Missing, MultiLine, TooLong};
    assert_eq!(
        found,
        [
            vec![(Missing, Some(Title))],
            vec![(TooLong, Some(Title))],
            vec![(MultiLine, Some(Title))],
            vec![(TooLong, Some(Body))],
            vec![(Missing, Some(Body))],
            vec![(TooLong, Some(Tags))],
            vec![],
        ]
    );

    let importable: Vec<bool> = read.stored.rows.iter().map(|row| row.importable).collect();
    assert_eq!(
        importable,
        [false, false, false, false, false, false, true],
        "limits are inclusive"
    );
    let preview = &read.plan.rows[3].body_preview;
    assert!(preview.ends_with('…'), "a long body is cut in the review");
    assert_eq!(preview.chars().count(), PREVIEW_CHARS + 1);
    assert_eq!(read.stored.rows[6].title.chars().count(), TITLE_MAX, "but kept whole");
}

#[test]
fn a_repository_row_without_a_project_needs_one() {
    let read = read_notes_file("notes.csv", b"title,body,scope\nA,b,repo\nB,b,\n", None).expect("read");

    assert_eq!(
        problems(&read.plan.rows[0]),
        [(NotesImportProblemCode::NeedsProject, Some(NotesImportField::Scope))]
    );
    assert!(
        read.plan.rows[1].problems.is_empty(),
        "the default scope is the dialog's"
    );
    assert_eq!(read.stored.project(), None);
}

#[test]
fn later_rows_with_an_earlier_title_point_at_the_first() {
    let read = read(
        "notes.csv",
        "title,body\nCache the probe,a\nOther,b\n  cache   THE probe ,c\nCache the probe,d\n",
    );
    let duplicates: Vec<Option<u32>> = read.plan.rows.iter().map(|row| row.duplicate_of).collect();

    assert_eq!(duplicates, [None, None, Some(0), Some(0)]);
}

#[test]
fn only_the_first_500_notes_are_read() {
    let mut text = String::from("title,body\n");
    for index in 0..=MAX_IMPORT_ROWS {
        text.push_str(&format!("Note {index},Body\n"));
    }

    let over = read("notes.csv", &text);
    assert!(over.plan.truncated);
    assert_eq!(over.plan.rows.len(), MAX_IMPORT_ROWS);

    let exact_text = text
        .rsplit_once(&format!("Note {MAX_IMPORT_ROWS},"))
        .expect("last row")
        .0;
    let exact = read("notes.csv", exact_text);
    assert!(!exact.plan.truncated, "exactly 500 notes are all read");
    assert_eq!(exact.plan.rows.len(), MAX_IMPORT_ROWS);

    let json = format!(
        "[{}]",
        vec![r#"{"title":"t","body":"b"}"#; MAX_IMPORT_ROWS + 1].join(",")
    );
    let json = read("notes.json", &json);
    assert!(json.plan.truncated);
    assert_eq!(json.plan.rows.len(), MAX_IMPORT_ROWS);
}

#[test]
fn a_file_over_2_mib_or_not_text_is_refused() {
    let large = vec![b'a'; MAX_IMPORT_BYTES + 1];
    assert_eq!(refusal("notes.csv", &large), NotesImportFileProblem::TooLarge);

    assert_eq!(
        refusal("notes.csv", b"title,body\n\xff\xfe,b\n"),
        NotesImportFileProblem::NotUtf8
    );
}

#[test]
fn a_json_array_or_an_object_with_notes_is_read() {
    let array = read(
        "export.json",
        r#"[{"Title":"A","body":"b","kind":"fix","tags":["Perf","has space"],"scope":"global"},
            {"title":"B","body":"b","tags":"one, two"}]"#,
    );
    assert_eq!(array.plan.format, NotesImportFormat::Json);
    assert_eq!(titles(&array), ["A", "B"]);
    assert_eq!(array.plan.rows[0].tags, ["perf", "has", "space"]);
    assert_eq!(array.plan.rows[0].kind, NoteKind::Fix);
    assert_eq!(array.plan.rows[1].tags, ["one", "two"]);

    let object = read("export.txt", r#" {"notes":[{"title":"C","body":"b"}]}"#);
    assert_eq!(
        object.plan.format,
        NotesImportFormat::Json,
        "the content tells JSON from CSV"
    );
    assert_eq!(titles(&object), ["C"]);

    let csv = read("export.txt", "title,body\nD,b\n");
    assert_eq!(csv.plan.format, NotesImportFormat::Csv);
}

#[test]
fn json_values_of_the_wrong_type_are_problems() {
    let read = read(
        "notes.json",
        r#"[{"title":3,"body":"b","tags":[1]}, "a note", {"title":"T","body":null,"scope":true}]"#,
    );

    use NotesImportField::{Body, Scope, Tags, Title};
    use NotesImportProblemCode::{Missing, WrongType};
    assert_eq!(
        problems(&read.plan.rows[0]),
        [(WrongType, Some(Title)), (WrongType, Some(Tags))]
    );
    assert_eq!(
        problems(&read.plan.rows[1]),
        [(WrongType, None), (Missing, Some(Title)), (Missing, Some(Body))]
    );
    assert_eq!(
        problems(&read.plan.rows[2]),
        [(WrongType, Some(Scope)), (Missing, Some(Body))]
    );
}

#[test]
fn json_of_another_shape_is_refused() {
    let shape = NotesImportFileProblem::WrongJsonShape;

    assert_eq!(refusal("notes.json", br#"{"items":[]}"#), shape);
    assert_eq!(refusal("notes.json", b"42"), shape);
    assert_eq!(refusal("notes.json", b"[{"), NotesImportFileProblem::InvalidJson);
}

#[test]
fn a_selection_is_the_importable_rows_in_the_file_order() {
    let read = read("notes.csv", "title,body\nA,a\n,missing\nC,c\n");
    let indexes = |rows: Vec<&StoredRow>| rows.iter().map(|row| row.index).collect::<Vec<_>>();

    assert_eq!(
        indexes(selected_rows(&read.stored, &[2, 0, 2]).expect("selected")),
        [0, 2]
    );

    for refused in [&[][..], &[1], &[0, 7]] {
        let error = selected_rows(&read.stored, refused).expect_err("refused");
        assert!(
            matches!(error, BusinessError::InvalidInput(_)),
            "{refused:?}: {error:?}"
        );
    }
}

#[test]
fn a_kept_row_becomes_a_draft_with_its_scope_or_the_default() {
    let read = read("notes.csv", "title,body,scope\n  Padded  ,\"Body\n\",\nB,b,global\n");

    let padded = read.stored.rows[0].draft(ScriptScope::Repo);
    assert_eq!(padded.title, "Padded");
    assert_eq!(padded.body, "Body\n");
    assert_eq!(padded.scope, ScriptScope::Repo);

    let global = read.stored.rows[1].draft(ScriptScope::Repo);
    assert_eq!(global.scope, ScriptScope::Global);
}
