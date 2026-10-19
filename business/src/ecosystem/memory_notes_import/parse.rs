//! Reads the notes of an imported file as raw fields, before any check (see the parent module).
//!
//! - CSV: a UTF-8 byte order mark is dropped, the separator is the most frequent of `,`, `;` and
//!   tab on the header line (outside quotes; `,` when there is none), and fields follow RFC 4180
//!   (quotes, doubled quotes, line breaks inside quotes). The header names the columns `title`,
//!   `body`, `kind`, `tags` and `scope` in any case and order; `title` and `body` are required,
//!   other columns are ignored, and blank records are skipped.
//! - JSON: an array of objects, or an object with one under `notes`. Keys are read in any case;
//!   `tags` is an array of strings or one string.

use std::collections::HashMap;

use serde_json::Value;

use crate::model::{NotesImportField, NotesImportFileProblem, NotesImportFormat};

/// How many notes an import reads; the rest of the file is reported as `truncated`.
pub const MAX_IMPORT_ROWS: usize = 500;
/// The largest file an import reads, in bytes (2 MiB).
pub const MAX_IMPORT_BYTES: usize = 2 * 1024 * 1024;

/// One note of the file, its fields as they are written.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RawRow {
    pub title: Option<String>,
    pub body: Option<String>,
    pub kind: Option<String>,
    pub scope: Option<String>,
    /// The tag cells or strings, not split yet.
    pub tags: Vec<String>,
    /// The fields of the wrong JSON type, `None` for a row that is not an object.
    pub wrong_type: Vec<Option<NotesImportField>>,
}

/// The notes read from a file.
#[derive(Debug, PartialEq, Eq)]
pub struct RawFile {
    pub format: NotesImportFormat,
    pub rows: Vec<RawRow>,
    /// The file has more than [`MAX_IMPORT_ROWS`] notes.
    pub truncated: bool,
}

/// Why a whole file cannot be read as notes, and on which line for invalid CSV.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileRefusal {
    pub reason: NotesImportFileProblem,
    pub line: Option<u32>,
}

impl From<NotesImportFileProblem> for FileRefusal {
    fn from(reason: NotesImportFileProblem) -> Self {
        Self { reason, line: None }
    }
}

type Read<T> = std::result::Result<T, FileRefusal>;

const BOM: char = '\u{feff}';
const SEPARATORS: [u8; 3] = *b",;\t";

/// The notes of `bytes`, read as the format of `file_name`'s extension (`.csv`, `.tsv`,
/// `.json`), or of its content for any other name. Refused (see [`NotesImportFileProblem`]) for a
/// file over the size limit, one that is not UTF-8 text, a blank file or one without notes, a CSV
/// file without a `title` and a `body` column or with a broken record, and JSON that does not
/// parse or has another shape.
pub fn read_file(file_name: &str, bytes: &[u8]) -> Read<RawFile> {
    if bytes.len() > MAX_IMPORT_BYTES {
        return Err(NotesImportFileProblem::TooLarge.into());
    }

    let text = std::str::from_utf8(bytes).map_err(|_| FileRefusal::from(NotesImportFileProblem::NotUtf8))?;
    let text = text.strip_prefix(BOM).unwrap_or(text);

    if text.trim().is_empty() {
        return Err(NotesImportFileProblem::Empty.into());
    }

    let file = match format_of(file_name, text) {
        NotesImportFormat::Csv => read_csv(text)?,
        NotesImportFormat::Json => read_json(text)?,
    };

    if file.rows.is_empty() {
        return Err(NotesImportFileProblem::Empty.into());
    }

    Ok(file)
}

/// The format of the extension of `file_name`, otherwise JSON for text that starts with `[` or
/// `{` and CSV for the rest.
fn format_of(file_name: &str, text: &str) -> NotesImportFormat {
    let extension = file_name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase());

    match extension.as_deref() {
        Some("csv" | "tsv") => NotesImportFormat::Csv,
        Some("json") => NotesImportFormat::Json,
        _ if text.trim_start().starts_with(['[', '{']) => NotesImportFormat::Json,
        _ => NotesImportFormat::Csv,
    }
}

/// The separator of the header line: the most frequent of `,`, `;` and tab outside quotes, the
/// first of them on a tie, `,` when the line has none.
fn separator(text: &str) -> u8 {
    let header = text.split('\n').next().unwrap_or_default();
    let mut counts = [0usize; SEPARATORS.len()];
    let mut quoted = false;

    for byte in header.bytes() {
        if byte == b'"' {
            quoted = !quoted;
            continue;
        }

        if let Some(position) = SEPARATORS.iter().position(|separator| *separator == byte)
            && !quoted
        {
            counts[position] += 1;
        }
    }

    let mut best = 0;

    for (position, count) in counts.iter().enumerate() {
        if *count > counts[best] {
            best = position;
        }
    }

    SEPARATORS[best]
}

fn read_csv(text: &str) -> Read<RawFile> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(separator(text))
        .flexible(true)
        .has_headers(true)
        .from_reader(text.as_bytes());

    let headers = reader.headers().map_err(csv_refusal)?;
    let columns = Columns::of(headers);

    let (Some(title), Some(body)) = (columns.title, columns.body) else {
        return Err(NotesImportFileProblem::MissingColumns.into());
    };

    let mut rows = Vec::new();
    let mut truncated = false;

    for record in reader.records() {
        let record = record.map_err(csv_refusal)?;

        if record.iter().all(|cell| cell.trim().is_empty()) {
            continue;
        }

        if rows.len() == MAX_IMPORT_ROWS {
            truncated = true;
            break;
        }

        let cell = |column: Option<usize>| column.and_then(|column| record.get(column)).map(str::to_owned);

        rows.push(RawRow {
            title: cell(Some(title)),
            body: cell(Some(body)),
            kind: cell(columns.kind),
            scope: cell(columns.scope),
            tags: cell(columns.tags).into_iter().collect(),
            wrong_type: Vec::new(),
        });
    }

    Ok(RawFile {
        format: NotesImportFormat::Csv,
        rows,
        truncated,
    })
}

/// An unreadable CSV record, with its line when the reader knows it.
fn csv_refusal(error: csv::Error) -> FileRefusal {
    let line = error
        .position()
        .and_then(|position| u32::try_from(position.line()).ok());

    FileRefusal {
        reason: NotesImportFileProblem::InvalidCsv,
        line,
    }
}

/// Where each known column is in a CSV header (the first of a repeated name).
#[derive(Default)]
struct Columns {
    title: Option<usize>,
    body: Option<usize>,
    kind: Option<usize>,
    tags: Option<usize>,
    scope: Option<usize>,
}

impl Columns {
    fn of(headers: &csv::StringRecord) -> Self {
        let mut columns = Self::default();

        for (position, name) in headers.iter().enumerate() {
            let slot = match name.trim().to_lowercase().as_str() {
                "title" => &mut columns.title,
                "body" => &mut columns.body,
                "kind" => &mut columns.kind,
                "tags" => &mut columns.tags,
                "scope" => &mut columns.scope,
                _ => continue,
            };

            slot.get_or_insert(position);
        }

        columns
    }
}

fn read_json(text: &str) -> Read<RawFile> {
    let value: Value =
        serde_json::from_str(text).map_err(|_| FileRefusal::from(NotesImportFileProblem::InvalidJson))?;

    let notes = match value {
        Value::Array(notes) => notes,
        Value::Object(mut object) => match object.remove("notes") {
            Some(Value::Array(notes)) => notes,
            _ => {
                return Err(NotesImportFileProblem::WrongJsonShape.into());
            }
        },
        _ => {
            return Err(NotesImportFileProblem::WrongJsonShape.into());
        }
    };

    let truncated = notes.len() > MAX_IMPORT_ROWS;
    let rows = notes.iter().take(MAX_IMPORT_ROWS).map(json_row).collect();

    Ok(RawFile {
        format: NotesImportFormat::Json,
        rows,
        truncated,
    })
}

fn json_row(note: &Value) -> RawRow {
    let Value::Object(object) = note else {
        return RawRow {
            wrong_type: vec![None],
            ..RawRow::default()
        };
    };

    let mut fields: HashMap<String, &Value> = HashMap::new();

    for (key, value) in object {
        fields.entry(key.to_lowercase()).or_insert(value);
    }

    let mut row = RawRow::default();
    let mut text = |name: &str, field: NotesImportField| match fields.get(name) {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => Some(text.clone()),
        Some(_) => {
            row.wrong_type.push(Some(field));
            None
        }
    };

    let title = text("title", NotesImportField::Title);
    let body = text("body", NotesImportField::Body);
    let kind = text("kind", NotesImportField::Kind);
    let scope = text("scope", NotesImportField::Scope);

    row.title = title;
    row.body = body;
    row.kind = kind;
    row.scope = scope;

    match fields.get("tags") {
        None | Some(Value::Null) => {}
        Some(Value::String(tags)) => row.tags.push(tags.clone()),
        Some(Value::Array(tags)) if tags.iter().all(Value::is_string) => {
            row.tags = tags.iter().filter_map(Value::as_str).map(str::to_owned).collect();
        }
        Some(_) => row.wrong_type.push(Some(NotesImportField::Tags)),
    }

    row
}
