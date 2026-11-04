//! Helpers shared by the tests of drafts and team approval.

use harness_business::model::KnowledgeId;
use harness_business::process::ExecOutput;

use super::cli;
use crate::team_support::Cli;

/// The id of the pending row (or draft) the review tests act on.
pub const ID: i64 = 21;

pub fn doc_id() -> KnowledgeId {
    ID.into()
}

/// A server that lists `features` and answers the commands starting with `prefix` with `output`.
pub fn docs_answering(features: &str, prefix: &[&str], output: ExecOutput) -> Cli {
    cli(features).starting(prefix, output)
}
