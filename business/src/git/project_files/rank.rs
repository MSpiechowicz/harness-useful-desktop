//! The matches in rank order, sorted only as far as the caller reads.
//!
//! A search over a repository of 200,000 files matches most of them on a short query. Sorting them all
//! on every keystroke is waste when the caller takes the first 50, so [`Ranked`] selects the best
//! few, sorts those, and goes back for the next batch only when the caller read past them (it does,
//! when the best files turn out to be denied).

use std::cmp::Ordering;

use super::entries::Entry;
use super::score::fold;

/// What a changed file adds to its score. A clear boost, still below the gap between a match at the
/// start of a file name and one in the middle of it.
pub(super) const CHANGED_BOOST: i32 = 500;

/// Matches sorted in the first batch, and the floor of every later one.
const FIRST_BATCH: usize = 128;

/// One match: its score and where its entry is.
pub(super) struct Match {
    pub(super) score: i32,
    pub(super) index: usize,
}

/// Matches in rank order: the higher score first; then, with a query, the shorter path, and
/// without one, the shallower path; then the alphabetically first (without regard to case); at last
/// the path's bytes, so the order never depends on how the list was built.
pub(super) struct Ranked<'a> {
    entries: &'a [Entry],
    matches: Vec<Match>,
    by_depth: bool,
    sorted: usize,
    next: usize,
    batch: usize,
}

impl<'a> Ranked<'a> {
    pub(super) fn new(entries: &'a [Entry], matches: Vec<Match>, by_depth: bool, wanted: usize) -> Self {
        Self {
            entries,
            matches,
            by_depth,
            sorted: 0,
            next: 0,
            batch: FIRST_BATCH.max(wanted.saturating_mul(4)),
        }
    }

    /// Sorts the next `batch` best of the matches that are still unsorted.
    fn sort_next_batch(&mut self) {
        let end = (self.sorted + self.batch).min(self.matches.len());
        let (entries, by_depth) = (self.entries, self.by_depth);
        let order = |left: &Match, right: &Match| compare(entries, by_depth, left, right);

        let rest = &mut self.matches[self.sorted..];
        let batch = end - self.sorted;
        if batch < rest.len() {
            rest.select_nth_unstable_by(batch - 1, order);
        }

        rest[..batch].sort_unstable_by(order);

        self.sorted = end;
        self.batch = self.batch.saturating_mul(4);
    }
}

impl<'a> Iterator for Ranked<'a> {
    type Item = &'a Entry;

    fn next(&mut self) -> Option<&'a Entry> {
        if self.next >= self.matches.len() {
            return None;
        }

        if self.next >= self.sorted {
            self.sort_next_batch();
        }

        let found = &self.matches[self.next];
        self.next += 1;

        self.entries.get(found.index)
    }
}

fn compare(entries: &[Entry], by_depth: bool, left: &Match, right: &Match) -> Ordering {
    let (first, second) = (&entries[left.index], &entries[right.index]);

    right.score.cmp(&left.score).then_with(|| {
        let shape = if by_depth {
            first.depth.cmp(&second.depth)
        } else {
            first.path.len().cmp(&second.path.len())
        };

        shape
            .then_with(|| first.path.chars().map(fold).cmp(second.path.chars().map(fold)))
            .then_with(|| first.path.cmp(&second.path))
    })
}
