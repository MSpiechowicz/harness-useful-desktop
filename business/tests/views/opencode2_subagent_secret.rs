//! The OpenCode server password and the subagent panel of an OpenCode 2 chat: the password is
//! masked before any text is cut to its limit, so a cut never leaves a piece of it. Also the size
//! of the ids and labels the server and the models choose.

use crate::opencode_subagent_support;
use crate::opencode2_mapper_support;
use crate::opencode2_subagent_mapper::{
    answered, call, child, collect, created, progress, result, started, text_ended,
};

use harness_business::model::{
    AgentEvent, MAX_STEP_DETAIL_CHARS, MAX_STEP_OUTPUT_BYTES, MAX_STEP_TEXT_CHARS, MAX_SUBAGENT_TEXT_CHARS,
};
use opencode_subagent_support::{fake_password, latest, leaked, shown, steps, straddling, straddling_tail};
use opencode2_mapper_support::{SESSION, mapper};
use pretty_assertions::assert_eq;
use serde_json::json;

const CHILD: &str = "ses_child1";

#[test]
fn a_password_cut_by_any_limit_leaves_no_piece_of_it_in_the_panel() {
    let password = fake_password();
    let mut mapper = mapper();
    mapper.set_secret(Some(&password));

    let label = straddling(MAX_STEP_DETAIL_CHARS, &password);
    let out = collect(
        &mut mapper,
        vec![
            call(
                "call_s",
                &label,
                &label,
                &straddling(MAX_SUBAGENT_TEXT_CHARS, &password),
            ),
            vec![
                created(CHILD, SESSION),
                progress("call_s", CHILD),
                child(
                    CHILD,
                    "session.step.started",
                    json!({ "agent": label, "model": { "id": label, "providerID": label } }),
                ),
            ],
            started(CHILD, "call_a", "shell", json!({ "command": label })),
            vec![
                result(CHILD, "call_a", &straddling_tail(MAX_STEP_OUTPUT_BYTES, &password)),
                text_ended(CHILD, 0, &straddling(MAX_STEP_TEXT_CHARS, &password)),
                answered("call_s", &straddling(MAX_SUBAGENT_TEXT_CHARS, &password)),
            ],
        ],
    );

    assert_eq!(
        leaked(&shown(&out), &password),
        None,
        "no panel event shows a piece of the password"
    );

    let all = steps(&out, CHILD);
    assert!(
        all.iter()
            .any(|step| step.detail.chars().count() == MAX_STEP_DETAIL_CHARS)
    );
    assert!(
        all.iter()
            .any(|step| step.detail.chars().count() == MAX_STEP_TEXT_CHARS)
    );
    assert!(all.iter().any(|step| step.output.is_some()));
    assert_eq!(
        latest(&out, CHILD).report.map(|report| report.chars().count()),
        Some(MAX_SUBAGENT_TEXT_CHARS)
    );
}

#[test]
fn ids_agents_and_models_from_the_server_are_bounded() {
    let long = "l".repeat(1000);
    let mut mapper = mapper();

    let out = collect(
        &mut mapper,
        vec![
            call("call_s", "Check", "general", "Look"),
            vec![
                created(&"s".repeat(1000), SESSION),
                created(CHILD, SESSION),
                progress("call_s", CHILD),
                child(
                    CHILD,
                    "session.step.started",
                    json!({ "agent": long, "model": { "id": long, "providerID": long } }),
                ),
            ],
            started(CHILD, &long, "shell", json!({ "command": "ls" })),
        ],
    );

    let snapshot = latest(&out, CHILD);

    assert!(snapshot.agent.chars().count() <= MAX_STEP_DETAIL_CHARS);
    assert!(
        snapshot
            .model
            .as_deref()
            .is_some_and(|model| model.chars().count() <= 2 * MAX_STEP_DETAIL_CHARS + 1)
    );
    assert!(
        steps(&out, CHILD)
            .iter()
            .all(|step| step.id.chars().count() <= MAX_STEP_DETAIL_CHARS)
    );
    assert!(
        out.iter().all(|event| match event {
            AgentEvent::Subagent { subagent } => subagent.id.chars().count() <= MAX_STEP_DETAIL_CHARS,
            _ => true,
        }),
        "a child session with an absurd id is not tracked"
    );
}
