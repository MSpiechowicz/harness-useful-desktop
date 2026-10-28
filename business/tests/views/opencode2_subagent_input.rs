//! The preview of an OpenCode 2 subagent's tool call: it comes from the input once that input finished streaming, and a later
//! event without the input never blanks it.

use crate::opencode_subagent_support;
use crate::opencode2_mapper_support;
use crate::opencode2_subagent_mapper::{call, child, collect, created, progress, result};

use harness_business::model::SubagentStepStatus;
use opencode_subagent_support::steps;
use opencode2_mapper_support::{SESSION, mapper};
use pretty_assertions::assert_eq;
use serde_json::json;

const CHILD: &str = "ses_child1";

#[test]
fn a_call_named_only_by_its_streamed_input_keeps_that_preview_when_it_ends() {
    let mut mapper = mapper();

    let out = collect(
        &mut mapper,
        vec![
            call("call_s", "Evaluate", "general", "Work it out"),
            vec![created(CHILD, SESSION), progress("call_s", CHILD)],
            vec![
                child(
                    CHILD,
                    "session.tool.input.started",
                    json!({ "id": "call_e", "name": "eval" }),
                ),
                child(
                    CHILD,
                    "session.tool.input.ended",
                    json!({ "id": "call_e", "text": "{\"expression\":\"2 + 2\"}" }),
                ),
                // The call event of this tool brings no input.
                child(CHILD, "session.tool.called", json!({ "id": "call_e", "input": {} })),
                result(CHILD, "call_e", "4"),
            ],
        ],
    );

    let shown: Vec<(SubagentStepStatus, String)> = steps(&out, CHILD)
        .into_iter()
        .filter(|step| step.id == "call_e")
        .map(|step| (step.status, step.detail))
        .collect();

    assert_eq!(
        shown.last(),
        Some(&(SubagentStepStatus::Done, r#"{"expression":"2 + 2"}"#.to_owned()))
    );
    assert!(
        shown.iter().skip(1).all(|(_, detail)| !detail.is_empty()),
        "once known, the preview is never blank again: {shown:?}"
    );
}
