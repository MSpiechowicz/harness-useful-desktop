//! Case-insensitive fuzzy matching of a query against a project path, with the feel of
//! `ui/src/lib/utils/fuzzy.ts`: a contiguous match always beats a scattered one, a match at the start
//! of a word scores higher, and so do consecutive characters. Scores are integers in tenths of the
//! TypeScript points, so ranking never depends on float rounding.
//!
//! A match inside the file name beats any match that needs the folders ([`NAME_BONUS`]).

/// What a match inside the file name adds. Above the best score a match over the whole path reaches.
const NAME_BONUS: i32 = 2000;

/// A contiguous match scores this, plus the bonuses below, minus 10 per character it starts at.
const CONTIGUOUS: i32 = 1000;

/// The extra score of a contiguous match that starts a word.
const WORD_START: i32 = 400;

/// The extra score of a contiguous match that starts the text.
const PREFIX: i32 = 200;

/// Per matched character of a scattered match: right after the previous one.
const CONSECUTIVE: i32 = 80;

/// Per matched character of a scattered match: at the start of a word.
const WORD_START_CHAR: i32 = 60;

/// Per matched character of a scattered match, minus 1 per character skipped before it.
const MATCHED_CHAR: i32 = 10;

/// A scattered match never reaches a contiguous one.
const SCATTERED_CAP: i32 = CONTIGUOUS - 10;

/// A query ready to score many paths. Holds scratch space, so one matcher serves one search.
pub(super) struct Matcher {
    needle: Vec<char>,
    text: Vec<char>,
}

impl Matcher {
    /// `None` for a query that is empty once trimmed: it matches everything with no score.
    pub(super) fn new(query: &str) -> Option<Self> {
        let needle: Vec<char> = query
            .trim()
            .chars()
            .map(|character| if character == '\\' { '/' } else { fold(character) })
            .collect();

        (!needle.is_empty()).then_some(Self {
            needle,
            text: Vec::new(),
        })
    }

    /// The score of `path`, or `None` when the query does not match it.
    pub(super) fn score(&mut self, path: &str) -> Option<i32> {
        self.text.clear();
        self.text.extend(path.chars().map(fold));

        let name = self
            .text
            .iter()
            .rposition(|&character| character == '/')
            .map_or(0, |slash| slash + 1);

        if let Some(score) = match_in(&self.needle, &self.text[name..]) {
            return Some(score + NAME_BONUS);
        }

        if name == 0 {
            return None;
        }

        match_in(&self.needle, &self.text)
    }
}

/// The character the comparison sees: lowercase, one character for one.
pub(super) fn fold(character: char) -> char {
    if character.is_ascii() {
        return character.to_ascii_lowercase();
    }

    character.to_lowercase().next().unwrap_or(character)
}

fn match_in(needle: &[char], text: &[char]) -> Option<i32> {
    contiguous(needle, text).or_else(|| scattered(needle, text))
}

fn contiguous(needle: &[char], text: &[char]) -> Option<i32> {
    let start = text.windows(needle.len()).position(|window| window == needle)?;

    let boundary = if is_word_start(text, start) { WORD_START } else { 0 };
    let prefix = if start == 0 { PREFIX } else { 0 };

    Some(CONTIGUOUS + boundary + prefix - 10 * count(start))
}

fn scattered(needle: &[char], text: &[char]) -> Option<i32> {
    let mut score = 0;
    let mut cursor = 0;
    let mut previous: Option<usize> = None;

    for &wanted in needle {
        let found = cursor + text[cursor..].iter().position(|&character| character == wanted)?;

        if previous.is_some_and(|last| found == last + 1) {
            score += CONSECUTIVE;
        }

        if is_word_start(text, found) {
            score += WORD_START_CHAR;
        }

        score += MATCHED_CHAR - count(found - cursor);
        previous = Some(found);
        cursor = found + 1;
    }

    Some(score.min(SCATTERED_CAP))
}

/// True at the start of the text and right after a space, `-`, `_`, `/`, `.` or `:`.
fn is_word_start(text: &[char], index: usize) -> bool {
    index == 0 || text.get(index - 1).is_some_and(|&before| is_boundary(before))
}

fn is_boundary(character: char) -> bool {
    character.is_whitespace() || matches!(character, '-' | '_' | '/' | '.' | ':')
}

/// A length as a score term, kept small enough to multiply.
fn count(length: usize) -> i32 {
    i32::try_from(length).unwrap_or(i32::MAX / 100)
}
