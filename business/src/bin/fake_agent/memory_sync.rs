//! `harness-memory sync --json`, the copy of what waits for the team database, answered from fixed
//! fixtures (nothing is kept between runs).
//!
//! It is a usage error (exit 2) unless the stand-in lists `sync-command`. Otherwise it prints, by
//! `FAKE_MEMORY_SYNC`:
//!
//! - unset: `{"online": true, "waiting": {"notes": 0, "scripts": 0, "updates": 0}}`, with a field the
//!   app does not read besides;
//! - `offline`: the team database is unreachable, so `{"online": false, "waiting": {"notes": 2,
//!   "scripts": 1, "updates": 3}}`;
//! - `failing`: nothing on stdout and a line on stderr, exit 1;
//! - `garbled`: a line that is not JSON, exit 0.

use serde_json::json;

use crate::memory_push::lists;

/// `harness-memory sync <args>`.
pub fn sync(args: &[String]) -> ! {
    if args != ["--json"] || !lists("sync-command") {
        eprintln!("usage: harness-memory sync --json");
        std::process::exit(2);
    }

    match std::env::var("FAKE_MEMORY_SYNC").as_deref() {
        Ok("offline") => println!(
            "{}",
            json!({ "online": false, "waiting": { "notes": 2, "scripts": 1, "updates": 3 } })
        ),
        Ok("failing") => {
            eprintln!("harness-memory: the notes store cannot be opened");
            std::process::exit(1);
        }
        Ok("garbled") => println!("not json at all"),
        _ => println!(
            "{}",
            json!({ "online": true, "waiting": { "notes": 0, "scripts": 0, "updates": 0 }, "checkedAt": 1 })
        ),
    }

    std::process::exit(0);
}
