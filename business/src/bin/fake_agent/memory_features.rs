//! `harness-memory features`, answered from a fixed list (see [`DEFAULT_FEATURES`]).
//!
//! `FAKE_MEMORY_FEATURES=none` is a server that predates the command (usage error, exit 2) and
//! `FAKE_MEMORY_FEATURES=<a,b>` lists those instead. Without the variable, the same text in
//! `.fake-memory-features` of the working directory does the same (for a copy of the stand-in that
//! cannot have its own environment).

use super::usage;

/// `harness-memory features`.
pub fn features() -> ! {
    match features_setting().as_deref() {
        Some("none") => usage(),
        Some(listed) => println!("{}", listed.replace(',', "\n")),
        None => println!("{}", DEFAULT_FEATURES.join("\n")),
    }

    std::process::exit(0);
}

/// The features of a stand-in without `FAKE_MEMORY_FEATURES`.
const DEFAULT_FEATURES: [&str; 22] = [
    "notes-remote-local",
    "notes-hide",
    "push-json",
    "scripts-remote-local",
    "scripts-hide",
    "scripts-push-json",
    "embedder",
    "settings",
    "notes-lang",
    "notes-useful",
    "notes-authors",
    "notes-author-filter",
    "offline-update-queue",
    "note-expect-revision",
    "sync-command",
    "offline-fallback",
    "push-remote-local",
    "push-selection",
    "pull-json",
    "scripts-pull-json",
    "save-place",
    "remote-local-saves",
];

/// Whether this stand-in lists `feature` in `features`: the commands of a feature it does not list
/// are usage errors, as an older server answers.
pub fn lists(feature: &str) -> bool {
    match features_setting().as_deref() {
        Some("none") => false,
        Some(listed) => listed.split(',').any(|listed| listed.trim() == feature),
        None => DEFAULT_FEATURES.contains(&feature),
    }
}

/// Where an agent's new note of `scope` is saved, when the stand-in lists `remote-local-saves` and
/// the scope is routed `remote+local`: `local` when `HARNESS_MEMORY_REMOTE_LOCAL_SAVES` says so
/// (the config key `remote_local_saves`), else `remote`. `None` otherwise: the server does not say.
pub fn agent_note_place(scope: &str) -> Option<&'static str> {
    agent_place(super::routing(scope))
}

/// [`agent_note_place`] for a new script, routed by `HARNESS_MEMORY_SCRIPTS`.
pub fn agent_script_place() -> Option<&'static str> {
    agent_place(std::env::var("HARNESS_MEMORY_SCRIPTS").ok())
}

fn agent_place(routed: Option<String>) -> Option<&'static str> {
    if routed.as_deref() != Some("remote+local") || !lists("remote-local-saves") {
        return None;
    }

    match std::env::var("HARNESS_MEMORY_REMOTE_LOCAL_SAVES").as_deref() {
        Ok("local") => Some("local"),
        _ => Some("remote"),
    }
}

/// `FAKE_MEMORY_FEATURES`, else the text of `.fake-memory-features` in the working directory.
fn features_setting() -> Option<String> {
    std::env::var("FAKE_MEMORY_FEATURES").ok().or_else(|| {
        std::fs::read_to_string(".fake-memory-features")
            .ok()
            .map(|text| text.trim().to_owned())
    })
}
