//! `harness-memory embedder …` and `harness-memory settings …` stand-ins (vector search and the
//! English notes setting). Nothing is kept between runs: the answers come from variables.
//!
//! - `embedder run` plays `FAKE_EMBEDDER_SCRIPT`, one step per line: `emit <json>` prints a line,
//!   `sleep <ms>`, `exit <code>`, `exit-first <code>` (exits only in the first `embedder run` of
//!   its working directory, remembered in a file there), `hang` (ignores stdin EOF and SIGTERM
//!   until killed). When the
//!   script ends the process waits for stdin EOF and exits 0, as the real embedder does. Without a
//!   script it prints `ready` and waits.
//! - `embedder enable|disable` print nothing, exit 0 and remember the switch in `.fake-embedder-switch` of
//!   their working directory. `embedder status --json` prints
//!   `{"enabled": …, "running": false, "model_present": …, …}` (`enabled` is what the last
//!   `enable|disable` of that working directory said, else `FAKE_EMBEDDER_ENABLED == "true"`)
//!   (`model_present` is true when `FAKE_EMBEDDER_MODEL=present`, left out when it is `unknown`,
//!   else false). `embedder remove-model`
//!   exits 1 with the server's line when `FAKE_EMBEDDER_REMOVE=refuse`, else 0.
//! - `settings get --json` prints `{"english_notes": FAKE_ENGLISH_NOTES != "false"}`; `settings set
//!   english_notes=<bool> --json` prints that value back.
//! - Any other command or option is a usage error (exit 2).

use std::io::Read;
use std::time::Duration;

use serde_json::{Value, json};

/// The ready line of a stand-in without a script.
const READY: &str = r#"{"event":"ready","model":"fake-model","dim":384}"#;

/// Where `embedder enable|disable` remember the switch, in the working directory.
const SWITCH_FILE: &str = ".fake-embedder-switch";

/// `harness-memory embedder <args>`.
pub fn embedder(args: &[String]) -> ! {
    match args.first().map(String::as_str) {
        None => run(),
        Some("run") if args.len() == 1 => run(),
        Some(switch @ ("enable" | "disable")) if args.len() == 1 => {
            std::fs::write(SWITCH_FILE, switch).ok();
            std::process::exit(0);
        }
        Some("status") if args.get(1..) == Some(&["--json".to_owned()][..]) => status(),
        Some("remove-model") if args.len() == 1 => remove_model(),
        _ => usage(),
    }
}

/// `harness-memory settings <args>`.
pub fn settings(args: &[String]) -> ! {
    let known = std::env::var("FAKE_ENGLISH_NOTES").map_or(true, |value| value != "false");

    match args {
        [get, json] if get == "get" && json == "--json" => print(&json!({ "english_notes": known })),
        [set, assignment, json] if set == "set" && json == "--json" => {
            match assignment.strip_prefix("english_notes=") {
                Some("true") => print(&json!({ "english_notes": true })),
                Some("false") => print(&json!({ "english_notes": false })),
                _ => usage(),
            }
        }
        _ => usage(),
    }
}

fn status() -> ! {
    let enabled = match std::fs::read_to_string(SWITCH_FILE).as_deref() {
        Ok("enable") => true,
        Ok(_) => false,
        Err(_) => std::env::var("FAKE_EMBEDDER_ENABLED").is_ok_and(|value| value == "true"),
    };

    let mut status = json!({
        "enabled": enabled,
        "running": false,
        "active_model": null,
        "pending_model": null,
        "vectors": 0,
    });

    match std::env::var("FAKE_EMBEDDER_MODEL").as_deref() {
        Ok("unknown") => {}
        Ok("present") => status["model_present"] = json!(true),
        _ => status["model_present"] = json!(false),
    }

    print(&status)
}

fn remove_model() -> ! {
    if std::env::var("FAKE_EMBEDDER_REMOVE").is_ok_and(|value| value == "refuse") {
        eprintln!("harness-memory: the embedder is running; stop it first");
        std::process::exit(1);
    }

    std::process::exit(0);
}

/// `embedder run`: plays the script (see the module docs).
fn run() -> ! {
    let script = std::env::var("FAKE_EMBEDDER_SCRIPT").unwrap_or_else(|_| format!("emit {READY}"));

    for step in script.lines().map(str::trim).filter(|step| !step.is_empty()) {
        let (name, rest) = step.split_once(' ').unwrap_or((step, ""));

        match name {
            "emit" => {
                println!("{rest}");
                flush();
            }
            "sleep" => std::thread::sleep(Duration::from_millis(rest.parse().unwrap_or(0))),
            "exit" => std::process::exit(rest.parse().unwrap_or(0)),
            "exit-first" if first_run() => std::process::exit(rest.parse().unwrap_or(0)),
            "exit-first" => {}
            "hang" => loop {
                std::thread::sleep(Duration::from_secs(60));
            },
            _ => usage(),
        }
    }

    // The lifeline: the real embedder exits when its stdin closes.
    let mut sink = Vec::new();
    std::io::stdin().read_to_end(&mut sink).ok();
    std::process::exit(0);
}

/// True the first time it is asked in this working directory.
fn first_run() -> bool {
    let marker = std::path::Path::new(".fake-embedder-ran");

    if marker.exists() {
        return false;
    }

    std::fs::write(marker, "").ok();
    true
}

fn print(value: &Value) -> ! {
    println!("{value}");
    flush();
    std::process::exit(0);
}

fn flush() {
    use std::io::Write;

    std::io::stdout().flush().ok();
}

fn usage() -> ! {
    eprintln!("usage: harness-memory [embedder run|enable|disable|status|remove-model | settings get|set]");
    std::process::exit(2);
}
