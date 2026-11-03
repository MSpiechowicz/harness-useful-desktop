//! Which file holds a step when more than one could, and what a chat's harness changing under it
//! does: an edited copy beats an untouched one, a catalog file the user changed is never taken for
//! an untouched one, and Harness Skills installed or removed apply to the chat's next message.

use harness_business::hooks::{HookSession, STEPS_INTRO_NAME, SendOrigin};
use harness_business::model::HookSeed;
use pretty_assertions::assert_eq;

use crate::hook_support::{chat, prompt, set};
use crate::hooks_steps::hooks_steps::{arranged, family, lookups, names, resolved};

#[test]
fn installing_or_removing_useful_skills_applies_to_an_open_chat_on_its_next_message() {
    let hooks = set(vec![
        family(HookSeed::UsefulSkills, 1),
        family(HookSeed::MemoryFirst, 2),
    ]);
    let mut session = chat(false, false);

    let steps = |session: &mut HookSession| {
        let mut plan = session.on_send(&hooks, &prompt("Go"), SendOrigin::User);
        let names: Vec<String> = resolved(&plan).into_iter().map(|part| part.hook_name).collect();

        session.sending(&mut plan);
        names
    };

    session.set_useful_skills(false);
    assert_eq!(steps(&mut session), ["Harness Memory"]);

    session.set_useful_skills(true);
    assert_eq!(steps(&mut session), ["Harness Skills", "Harness Memory"]);

    session.set_useful_skills(false);
    assert_eq!(steps(&mut session), ["Harness Memory"]);
}

#[test]
fn two_untouched_files_for_one_step_make_one_element_and_the_first_in_run_order_wins() {
    let mut later = family(HookSeed::CodeGraphFirst, 1);
    later.created_at = "2030-01-01T00:00:00Z".to_owned();

    let first = family(HookSeed::CodeGraphFirst, 2);
    let first_id = first.id.clone();

    let hooks = set(vec![later, first]);
    let parts = arranged(&mut chat(false, false), &hooks, &lookups());

    assert_eq!(names(&parts), [STEPS_INTRO_NAME, "Harness Code"]);
    assert_eq!(parts[1].hook_id, first_id);
}

#[test]
fn an_edited_copy_of_a_step_wins_over_an_untouched_one_whatever_the_run_order() {
    let untouched = family(HookSeed::CodeGraphFirst, 1);

    let mut renamed = family(HookSeed::CodeGraphFirst, 2);
    renamed.name = "My code rule".to_owned();
    renamed.created_at = "2030-01-01T00:00:00Z".to_owned();

    // Edited only by the user's own scope, matcher or project: same name and text.
    let mut matcher = family(HookSeed::CodeGraphFirst, 3);
    matcher.matcher = Some("Bash".to_owned());
    matcher.created_at = "2031-01-01T00:00:00Z".to_owned();
    let matcher_id = matcher.id.clone();

    let parts = arranged(
        &mut chat(false, false),
        &set(vec![untouched.clone(), renamed]),
        &lookups(),
    );
    assert_eq!(names(&parts), [STEPS_INTRO_NAME, "My code rule"]);

    let parts = arranged(&mut chat(false, false), &set(vec![untouched, matcher]), &lookups());
    assert_eq!(names(&parts), [STEPS_INTRO_NAME, "Harness Code"]);
    assert_eq!(parts[1].hook_id, matcher_id);
}

#[test]
fn a_retired_file_with_a_matcher_is_not_skipped_as_untouched() {
    let mut changed = family(HookSeed::MemorySearch, 1);
    changed.matcher = Some("Bash".to_owned());

    let hooks = set(vec![changed, family(HookSeed::MemoryFirst, 2)]);
    let parts = arranged(&mut chat(true, false), &hooks, &lookups());

    assert_eq!(
        names(&parts),
        [STEPS_INTRO_NAME, "Harness Memory", "Search Harness Memory first"]
    );
}
