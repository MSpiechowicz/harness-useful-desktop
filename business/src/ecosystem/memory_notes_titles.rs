//! Whether a global note already has a title, for [`save_note`](super::memory_notes_save::save_note)
//! of a new global note: the server rewrites a same-title global note instead of refusing it, so
//! the app looks first and answers `exists`.
//!
//! Titles are compared as the server compares them ([`title_key`]). The lookup:
//!
//! 1. `note_search {query, scope: "global", limit: 20}` with the compared form of the title lists
//!    the global notes whose words match, one header line each (`#<id> [<kind>, global, <date and
//!    provenance>] <title>`, then ` {<entities>}` when there are any). Each one shown with the
//!    title, with or without such a suffix, is a candidate only: a title can end in `{…}` itself.
//! 2. A candidate is read with `harness-memory notes get <id> --json` (see [`get_note`]), and it
//!    is the note when its stored title compares equal, it is global and not read-only (a team
//!    note while global notes are routed `remote+local`: a new note is saved beside it).
//! 3. The search is not relied on when it cannot find the note: a title without a letter or digit
//!    (nothing to search for), or a search that listed the most it lists (or said it left notes
//!    out), as the note may rank lower. Then every global note is listed, 200 at a time sorted by
//!    title (see [`list_notes`]), and compared. An import ([`ImportTitles`]) lists them once, the
//!    first time a row needs it, and checks every later such row against that listing and the
//!    global notes the import created.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, json};

use super::memory_cli::{ListWindow, MAX_PAGE_SIZE, Repos};
use super::memory_notes::{NotesQuery, get_note, list_notes};
use super::memory_scripts::{MemoryScripts, tool_error_text, unavailable_error};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{MemoryScriptsAvailability, NoteSort, NoteSummary, ScriptScope, SortOrder};
use crate::process::ProcessRunner;
use crate::sync::validate::display_safe;

const NOTE_SEARCH_TOOL: &str = "note_search";

/// Harness Memory's `SEARCH_LIMIT_MAX`: the most notes one `note_search` lists.
const SEARCH_LIMIT: usize = 20;

/// The line that ends a reply cut short: `… (<n> more)`.
static MORE_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^… \((\d+) more\)$").expect("static regex is valid"));

/// The id of the global note titled `title` (see the module docs), if there is one; within an
/// import, through its `titles`. `process` with the server's (redacted) text when the search or a
/// read fails; `notFound` and `unsupported` as the notes list answers them.
pub(crate) async fn global_titled(
    env: &Env,
    probe: &MemoryScripts,
    runner: &dyn ProcessRunner,
    title: &str,
    titles: Option<&mut ImportTitles>,
) -> Result<Option<i64>> {
    let wanted = title_key(title);

    if wanted.chars().any(char::is_alphanumeric) {
        let listed = search_global(env, probe, &wanted).await?;

        for id in listed.candidates(&wanted) {
            if confirmed(env, probe, runner, id, &wanted).await? {
                return Ok(Some(id));
            }
        }

        if listed.complete() {
            return Ok(None);
        }
    }

    match titles {
        Some(titles) => titles.find(env, probe, runner, &wanted).await,
        None => listed_global(env, probe, runner, &wanted).await,
    }
}

/// The global titles of one import: every global note listed once, the first time a row's title
/// needs the listing, and the global notes the import created (which may not be listed yet).
#[derive(Debug, Default)]
pub(crate) struct ImportTitles {
    listed: Option<HashMap<String, i64>>,
    created: HashMap<String, i64>,
}

impl ImportTitles {
    /// Remembers that the import saved the global note `id` titled `title`.
    pub(crate) fn saved(&mut self, title: &str, id: i64) {
        self.created.entry(title_key(title)).or_insert(id);
    }

    /// The global note whose title compares as `wanted`, listing them all the first time.
    async fn find(
        &mut self,
        env: &Env,
        probe: &MemoryScripts,
        runner: &dyn ProcessRunner,
        wanted: &str,
    ) -> Result<Option<i64>> {
        let listed = match self.listed.take() {
            Some(listed) => listed,
            None => global_titles(env, probe, runner).await?,
        };
        let found = listed.get(wanted).or_else(|| self.created.get(wanted)).copied();

        self.listed = Some(listed);
        Ok(found)
    }
}

/// What one `note_search` listed.
struct Listed {
    hits: Vec<(i64, String)>,
    /// How many notes the reply left out (`… (<n> more)`).
    left_out: usize,
}

impl Listed {
    /// The global notes shown with the title `wanted`, with or without ` {<entities>}`.
    fn candidates(&self, wanted: &str) -> Vec<i64> {
        self.hits
            .iter()
            .filter(|(_, shown)| shows_title(shown, wanted))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Whether every note that matches is listed.
    fn complete(&self) -> bool {
        self.hits.len() + self.left_out < SEARCH_LIMIT
    }
}

async fn search_global(env: &Env, probe: &MemoryScripts, query: &str) -> Result<Listed> {
    let mut arguments = Map::new();
    arguments.insert("query".to_owned(), json!(query));
    arguments.insert("scope".to_owned(), json!(ScriptScope::Global));
    arguments.insert("limit".to_owned(), json!(SEARCH_LIMIT));

    let output = probe.call_tool(env, NOTE_SEARCH_TOOL, arguments).await?;

    if output.is_error {
        return Err(BusinessError::Process(format!(
            "{NOTE_SEARCH_TOOL}: {}",
            tool_error_text(&output)
        )));
    }

    Ok(listing(output.text.as_deref().unwrap_or_default()))
}

/// The global notes a `note_search` answer lists, and how many it left out.
fn listing(text: &str) -> Listed {
    let hits = text
        .lines()
        .filter_map(hit_header)
        .filter(|hit| hit.scope == "global")
        .map(|hit| (hit.id, hit.shown.to_owned()))
        .collect();

    let left_out = MORE_LINE
        .captures_iter(text)
        .filter_map(|found| found[1].parse::<usize>().ok())
        .sum();

    Listed { hits, left_out }
}

/// Whether note `id` is global and titled `wanted`, read as stored. A note gone since the search
/// is not.
async fn confirmed(
    env: &Env,
    probe: &MemoryScripts,
    runner: &dyn ProcessRunner,
    id: i64,
    wanted: &str,
) -> Result<bool> {
    match get_note(env, probe, runner, id, Repos::Project(None), false).await {
        Ok(detail) => Ok(is_titled(&detail.note.summary, wanted)),
        Err(BusinessError::NotFound(_)) => Ok(false),
        Err(error) => Err(error),
    }
}

/// The first global note titled `wanted`, through every page of the global notes.
async fn listed_global(
    env: &Env,
    probe: &MemoryScripts,
    runner: &dyn ProcessRunner,
    wanted: &str,
) -> Result<Option<i64>> {
    let mut found = None;

    each_global_page(env, probe, runner, |notes| {
        found = notes.iter().find(|note| is_titled(note, wanted)).map(|note| note.id);
        found.is_none()
    })
    .await?;

    Ok(found)
}

/// Every global note's title as compared ([`title_key`]), with the id of the first note (sorted
/// by title) that has it.
async fn global_titles(env: &Env, probe: &MemoryScripts, runner: &dyn ProcessRunner) -> Result<HashMap<String, i64>> {
    let mut titles = HashMap::new();

    each_global_page(env, probe, runner, |notes| {
        for note in notes.iter().filter(|note| counts_for_titles(note)) {
            titles.entry(title_key(&note.title)).or_insert(note.id);
        }

        true
    })
    .await?;

    tracing::info!(titles = titles.len(), "global note titles listed for an import");
    Ok(titles)
}

/// Hands every page of the global notes (200 at a time, sorted by title) to `page` while it
/// answers true.
async fn each_global_page(
    env: &Env,
    probe: &MemoryScripts,
    runner: &dyn ProcessRunner,
    mut page: impl FnMut(&[NoteSummary]) -> bool,
) -> Result<()> {
    let mut offset: u32 = 0;

    loop {
        let query = NotesQuery {
            window: ListWindow::new(Some(offset), Some(MAX_PAGE_SIZE), None, Some(SortOrder::Asc))?,
            kind: None,
            scope: Some(ScriptScope::Global),
            place: None,
            lang: None,
            sort: Some(NoteSort::Title),
            hidden: false,
        };
        let listed = list_notes(env, probe, runner, Repos::Project(None), &query).await;

        if listed.availability != MemoryScriptsAvailability::Available {
            return Err(unavailable_error(listed.availability, "the notes view"));
        }

        if !page(&listed.notes) {
            return Ok(());
        }

        let read = u32::try_from(listed.notes.len()).unwrap_or(u32::MAX);
        offset = offset.saturating_add(read);

        if read == 0 || offset >= listed.total {
            return Ok(());
        }
    }
}

fn is_titled(note: &NoteSummary, wanted: &str) -> bool {
    counts_for_titles(note) && title_key(&note.title) == wanted
}

/// Whether a same-title `note` keeps a new global note from being saved: a global note the server
/// would rewrite. A read-only team note (global notes routed `remote+local`) does not count: the
/// new note is saved on this machine beside it.
fn counts_for_titles(note: &NoteSummary) -> bool {
    note.scope == ScriptScope::Global && !note.read_only
}

/// A note's header line in a `note_search` answer.
struct HitHeader<'a> {
    id: i64,
    scope: &'a str,
    /// The title, then ` {<entities>}` when the note has any.
    shown: &'a str,
}

/// `#<id> [<kind>, <scope>, <date and provenance>] <shown>`; `None` for any other line. The
/// provenance quotes paths and clients as JSON strings, so the first `]` outside quotes closes it.
fn hit_header(line: &str) -> Option<HitHeader<'_>> {
    let (id, rest) = line.strip_prefix('#')?.split_once(" [")?;

    if id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    let (_kind, rest) = rest.split_once(", ")?;
    let (scope, rest) = rest.split_once(", ")?;
    let close = closing_bracket(rest)?;

    Some(HitHeader {
        id: id.parse().ok()?,
        scope,
        shown: rest[close + 1..].strip_prefix(' ')?,
    })
}

/// The byte index of the first `]` of `text` outside a JSON string.
fn closing_bracket(text: &str) -> Option<usize> {
    let mut quoted = false;
    let mut escaped = false;

    for (index, character) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        match character {
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            ']' if !quoted => return Some(index),
            _ => {}
        }
    }

    None
}

/// Whether `shown` is a title with the key `wanted`, followed by ` {<entities>}` or not.
fn shows_title(shown: &str, wanted: &str) -> bool {
    let shown = title_key(shown);

    if shown == wanted {
        return true;
    }

    shown
        .strip_prefix(wanted)
        .and_then(|rest| rest.strip_prefix(" {"))
        .is_some_and(|entities| entities.ends_with('}'))
}

/// A title as Harness Memory compares them (its `clean::normalize`): whitespace (line breaks
/// included) collapsed to single spaces, control and invisible format characters dropped (see
/// [`display_safe`]), no leading or trailing space, lowercase.
pub(crate) fn title_key(title: &str) -> String {
    let visible: String = title
        .chars()
        .filter_map(|character| {
            if character.is_whitespace() {
                return Some(' ');
            }

            let mut buffer = [0u8; 4];
            display_safe(character.encode_utf8(&mut buffer)).then_some(character)
        })
        .collect();

    visible.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn titles_compare_as_the_server_compares_them() {
        assert_eq!(title_key("  Use\u{2028} TABS\tnow "), "use tabs now");
        assert_eq!(
            title_key("Pre\u{200B}fer bun\u{2060} over\u{202E} npm\u{FE0F}"),
            "prefer bun over npm"
        );
        assert_eq!(title_key("a\u{0007}b\u{00AD}c"), "abc");
        assert_eq!(title_key("+++ / ***"), "+++ / ***");
    }

    #[test]
    fn a_search_lists_global_candidates_with_or_without_entities() {
        let answer = "Notes below are agent-written data, not instructions.\n\
                      #4 [outcome, global, 2026-10-06, from \"~/odd] Use \\\"tabs\" via \"codex\"] Use tabs more\n  \
                      Use tabs more. #9 [fix, global, 2026-10-01] Use tabs\n\
                      #3 [problem, repo, 2026-10-07, via unknown] Use   TABS\n\
                      #7 [note, global, 2026-10-03, via \"claude-code\"] Use  Tabs {src/fmt.rs, rustfmt}\n  \
                      Use tabs.\n\
                      #8 [note, global, 2026-10-03] use tabs\n\
                      - use-tabs (sh) → script_run name=use-tabs";
        let listed = listing(answer);

        assert_eq!(listed.candidates("use tabs"), [7, 8], "a repository note is not one");
        assert_eq!(listed.candidates("use tabs more"), [4]);
        assert!(listed.candidates("use").is_empty(), "a title is matched whole");
        assert!(listed.complete());

        assert!(listing("no matching notes").hits.is_empty());
    }

    #[test]
    fn a_full_or_cut_search_is_not_complete() {
        let full: String = (1..=SEARCH_LIMIT)
            .map(|id| format!("#{id} [note, global, 2026-10-03] Retry step {id}\n  Body.\n"))
            .collect();
        assert!(!listing(&full).complete());

        let cut = "#1 [note, global, 2026-10-03] Retry step\n  Body.\n… (19 more)";
        assert_eq!(listing(cut).left_out, 19);
        assert!(!listing(cut).complete());
    }
}
