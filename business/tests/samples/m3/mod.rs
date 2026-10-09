//! M3 fixtures for the sessions, changes and review views
//! (`ui/src/lib/ipc/mock/fixtures/{sessions,git,review}/`), produced by the real business code:
//! the session adapters over the session fixtures, git over a temporary repository with fixed
//! identities and dates, and the review composer. Paths are rewritten to the demo project
//! (`/home/demo/Projects/demo`). They hold the cases the views must render: a session tree with
//! two branch points, a diff with modified, added, renamed, binary and untracked files, and a
//! composed review prompt.

mod git;
mod review;
mod sessions;

use serde::Serialize;
use serde_json::Value;

fn to_json(value: impl Serialize) -> Value {
    serde_json::to_value(value).expect("DTOs always serialize")
}

/// Every M3 fixture path (relative to the fixtures directory) with its content.
pub fn m3_fixtures() -> Vec<(&'static str, Value)> {
    let mut fixtures = sessions::fixtures();
    fixtures.extend(git::fixtures());
    fixtures.extend(review::fixtures());
    fixtures
}
