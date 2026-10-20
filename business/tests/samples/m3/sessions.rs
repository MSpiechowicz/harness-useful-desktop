//! Saved-session fixtures (`sessions/`): the OMP and PI session fixtures
//! (`tests/fixtures/sessions`) listed and read by the real adapters in a temporary home, with
//! paths rewritten to the demo home.

use harness_business::harness::Harness;
use harness_business::harnesses::{Omp, Pi};
use harness_business::model::{HarnessId, MAX_SESSION_PAGE};
use harness_business::pi_family::sessions::SessionCache;
use serde_json::Value;

use crate::common::exported;
use crate::session_support::{OMP_MAIN, SessionHome, session};

/// The `sessions/` fixtures.
pub fn fixtures() -> Vec<(&'static str, Value)> {
    let home = SessionHome::with_fixtures(&[(&Omp, "omp"), (&Pi, "pi")]);
    let (env, project) = (&home.env, home.project.as_path());
    let main = session(HarnessId::Omp, OMP_MAIN);
    let cache = SessionCache::default();

    let omp_list = Omp.list_sessions(env, project, &cache).expect("OMP sessions");
    let pi_list = Pi.list_sessions(env, project, &cache).expect("PI sessions");
    let tree = Omp.session_tree(env, project, &main).expect("OMP tree");
    let messages = Omp
        .session_messages(env, project, &main, None, MAX_SESSION_PAGE)
        .expect("OMP transcript");
    let touched = Omp.session_files_touched(env, project, &main).expect("touched files");
    let omp_totals = Omp.session_totals(env).expect("OMP totals");
    let pi_totals = Pi.session_totals(env).expect("PI totals");

    vec![
        (
            "sessions/sessions_list.omp.json",
            exported(omp_list, &home.home, project),
        ),
        ("sessions/sessions_list.pi.json", exported(pi_list, &home.home, project)),
        ("sessions/session_tree.omp.json", exported(tree, &home.home, project)),
        (
            "sessions/session_messages.omp.json",
            exported(messages, &home.home, project),
        ),
        (
            "sessions/session_files_touched.omp.json",
            exported(touched, &home.home, project),
        ),
        (
            "sessions/harness_session_totals.omp.json",
            exported(omp_totals, &home.home, project),
        ),
        (
            "sessions/harness_session_totals.pi.json",
            exported(pi_totals, &home.home, project),
        ),
    ]
}
