//! The numbered steps of a prompt: how the Harness family hooks that are steps (`FamilyHook::step`)
//! are put in front of the message.
//!
//! [`arrange`] takes the parts the firings produced and returns them in the order the agent reads:
//! an intro, the steps by position numbered from 1, the memory settings notice, then every other
//! hook in run order. The order never depends on when a hook file was written. A step is found by
//! its file's `seed`, so an edited file still composes as its step. Pure: the shell passes in what
//! the app looked up for the message ([`Lookups`]).
//!
//! - Harness Dashboard and Git branch put the app's lookup before their text, and no step is made
//!   (nor numbered) without it.
//! - Harness Memory and Harness Knowledge put the notes or documents the app found after their
//!   text, or say that none matched or that the app did not look.
//! - One step per position: when two files hold the same step an edited copy wins over an untouched
//!   one, then the first in run order.
//! - An untouched file of a retired seed is skipped (the update deletes it), so nothing is said
//!   twice. An edited one stays an ordinary hook.
//! - Each lookup is cut to its cap, so all steps fit [`MAX_COMPOSED_BYTES`].

use super::compose::ResolvedPart;
use super::family::{FAMILY, FamilyHook, StepKind};
use crate::model::HookSeed;

/// The hook id of the intro element.
pub const STEPS_INTRO_ID: &str = "harness-desktop:steps";

/// The name of the intro element.
pub const STEPS_INTRO_NAME: &str = "Harness steps";

/// Most bytes of the notes the app found that go in the Harness Memory step: a whole
/// `harness-memory recall --hook` block, which is never larger, so it is never cut.
pub const MAX_MEMORY_LOOKUP_BYTES: usize = crate::pi_family::sessions::recall_block::RECALL_BUDGET;

/// Most bytes of the documents the app found that go in the Harness Knowledge step.
pub const MAX_KNOWLEDGE_LOOKUP_BYTES: usize = 2 * 1024;

/// Most bytes of the branch or limits lookup that go in their step.
pub const MAX_APP_LINE_BYTES: usize = 512;

/// What the app's lookup for a step produced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Lookup {
    /// The app did not look (it was not asked to, or the member is off).
    #[default]
    NotRun,
    /// The app looked and it did not work.
    Failed,
    /// The app looked and nothing matched.
    Empty,
    /// What matched.
    Found(String),
}

/// What the app looked up for one message.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lookups {
    /// The Harness Memory notes that match the message.
    pub memory: Lookup,
    /// The Harness Knowledge documents that match the message.
    pub knowledge: Lookup,
    /// How the branch stands against main, `None` when there is nothing to say.
    pub branch: Option<String>,
    /// The plan limits that are close, `None` when none is.
    pub limits: Option<String>,
}

/// One step found among the parts.
struct Step {
    part: ResolvedPart,
    position: u8,
    kind: StepKind,
    /// True when the file is not a catalog version word for word (the user changed it).
    edited: bool,
}

/// The context parts of a prompt in reading order: the intro and the steps in `parts` (see the
/// module docs), the memory settings `notice`, then the other parts in the order given (run
/// order).
pub fn arrange(notice: Option<ResolvedPart>, parts: Vec<ResolvedPart>, lookups: &Lookups) -> Vec<ResolvedPart> {
    let mut steps = Vec::new();
    let mut others = Vec::new();

    for part in parts {
        match step_of(&part) {
            Placement::Step(position, kind) => {
                let edited = is_edited(&part);

                steps.push(Step {
                    part,
                    position,
                    kind,
                    edited,
                });
            }
            Placement::Skipped => tracing::debug!(hook = part.hook_id, "retired family hook left out"),
            Placement::Other => others.push(part),
        }
    }

    let elements: Vec<ResolvedPart> = one_per_position(steps)
        .into_iter()
        .filter_map(|step| element(step, lookups))
        .collect();

    let mut arranged = Vec::new();

    if let Some(intro) = intro(&elements) {
        arranged.push(intro);
    }

    arranged.extend(numbered(elements));
    arranged.extend(notice);
    arranged.extend(others);
    arranged
}

/// Where a part goes.
enum Placement {
    Step(u8, StepKind),
    /// An untouched file of a retired seed.
    Skipped,
    Other,
}

fn step_of(part: &ResolvedPart) -> Placement {
    let Some(entry) = part.seed.and_then(family_entry) else {
        return Placement::Other;
    };

    if entry.retired {
        return if is_untouched(entry, part) {
            Placement::Skipped
        } else {
            Placement::Other
        };
    }

    match entry.step {
        Some(step) => Placement::Step(step.position, step.kind),
        None => Placement::Other,
    }
}

fn family_entry(seed: HookSeed) -> Option<&'static FamilyHook> {
    FAMILY.iter().find(|entry| entry.seed == seed)
}

/// True when `part` is a catalog version of `entry` word for word, with the catalog's shape.
fn is_untouched(entry: &FamilyHook, part: &ResolvedPart) -> bool {
    part.plain && entry.is_version(&part.hook_name, &part.text)
}

/// True when `part` is a family file the user changed.
fn is_edited(part: &ResolvedPart) -> bool {
    part.seed
        .and_then(family_entry)
        .is_some_and(|entry| !is_untouched(entry, part))
}

/// `steps` by position, then edited copies first, then run order, with one step per position: the
/// first wins.
fn one_per_position(mut steps: Vec<Step>) -> Vec<Step> {
    steps.sort_by(|left, right| {
        left.position
            .cmp(&right.position)
            .then_with(|| right.edited.cmp(&left.edited))
            .then_with(|| left.part.order.cmp(&right.part.order))
    });

    steps.dedup_by(|later, earlier| {
        let same = later.position == earlier.position;

        if same {
            tracing::warn!(hook = later.part.hook_id, "family step left out: another hook holds it");
        }

        same
    });

    steps
}

/// The element of `step`, or `None` when it has nothing to say without its lookup.
fn element(step: Step, lookups: &Lookups) -> Option<ResolvedPart> {
    let text = match step.kind {
        StepKind::Rule => step.part.text.clone(),
        StepKind::Limits => app_line(lookups.limits.as_deref()?, &step.part.text),
        StepKind::Branch => app_line(lookups.branch.as_deref()?, &step.part.text),
        StepKind::Memory => with_data(
            &step.part.text,
            &lookups.memory,
            MAX_MEMORY_LOOKUP_BYTES,
            (
                "No note matched this message.",
                "The app did not look up notes for this message: search with note_search.",
            ),
        ),
        StepKind::Knowledge => with_data(
            &step.part.text,
            &lookups.knowledge,
            MAX_KNOWLEDGE_LOOKUP_BYTES,
            (
                "No document matched this message.",
                "The app did not look up documents for this message: search with knowledge_search.",
            ),
        ),
    };

    Some(ResolvedPart { text, ..step.part })
}

/// The app's `lookup`, then the hook's `text` on the next line.
fn app_line(lookup: &str, text: &str) -> String {
    format!("{}\n{text}", clipped(lookup, MAX_APP_LINE_BYTES))
}

/// `text`, then on a new line what `lookup` found, or (`empty`, `not_run`) when it found nothing or
/// did not run.
fn with_data(text: &str, lookup: &Lookup, cap: usize, (empty, not_run): (&str, &str)) -> String {
    let data = match lookup {
        Lookup::Found(block) => clipped(block, cap),
        Lookup::Empty => empty,
        Lookup::NotRun | Lookup::Failed => not_run,
    };

    format!("{text}\n{data}")
}

/// `text`, cut to at most `cap` bytes at a line break, or at a character when a line is that long.
fn clipped(text: &str, cap: usize) -> &str {
    if text.len() <= cap {
        return text;
    }

    let mut end = cap;

    while !text.is_char_boundary(end) {
        end -= 1;
    }

    match text[..end].rfind('\n') {
        Some(line) if line > 0 => &text[..line],
        _ => &text[..end],
    }
}

/// The intro element of `elements` (the steps), when there are any.
fn intro(elements: &[ResolvedPart]) -> Option<ResolvedPart> {
    if elements.is_empty() {
        return None;
    }

    let skills = elements.iter().any(|part| part.seed == Some(HookSeed::UsefulSkills));

    let mut text = format!(
        "Work through these {} steps in order, then do the request below. Write the status each step asks for \
         once, after you did the steps, at the start of your final answer and never before your first tool call: \
         a Markdown table with the header \"| Step | Status |\" and one row per status line in step order, the label \
         without its colon in the first column (for example \"| Complexity | small, a one-line fix |\"), also for a \
         small request. Never put a | inside a cell. Then leave a blank line and answer. Keep your prose short and \
         direct: no preamble or filler, and code, paths, commands, numbers and caveats stay exact.",
        elements.len()
    );

    if skills {
        text.push_str(" The steps after Harness Skills feed the complexity you pick.");
    }

    text.push_str(
        " Harness tools may be listed under a prefix (for example mcp__harness_memory_note_search) or as \
         deferred tools: load them (for example with ToolSearch) before use, and say once if one is missing. \
         When you run them from a script, print the text of each reply and not the whole result (print(result[\"text\"])), \
         because long output lines get cut, and when a script makes several calls print a line \"RESULT n:\" before each reply.",
    );

    Some(ResolvedPart::new(STEPS_INTRO_ID, STEPS_INTRO_NAME, text))
}

/// `elements` with "N. " in front of their texts, N counting from 1.
fn numbered(elements: Vec<ResolvedPart>) -> impl Iterator<Item = ResolvedPart> {
    elements.into_iter().enumerate().map(|(index, part)| ResolvedPart {
        text: format!("{}. {}", index + 1, part.text),
        ..part
    })
}
