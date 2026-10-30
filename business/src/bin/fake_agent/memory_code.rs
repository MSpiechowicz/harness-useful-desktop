//! `harness-memory code …` stand-in (the code index switch). The switch is kept between runs in
//! `.fake-code-switch` of the working directory.
//!
//! - `code status --json` prints `{"enabled": …, "source": …}`: `file` with what the last
//!   `enable|disable` of that working directory said, else `env` with `FAKE_CODE_INDEX` (`off` is
//!   off, anything else on), else `default` (on).
//! - `code enable|disable --json` remember the switch and print the state they left, or exit 1
//!   with the server's line when `FAKE_CODE_REFUSE` is set.
//! - `code build --project P --json` (only while `code-prepare` is listed) prints
//!   `{"built": true, "files": 3}` the first time in a working directory and `{"built": false,
//!   "reason": "indexed"}` after that (`.fake-code-built` remembers it), and `reason` `off` while
//!   the switch is off. `FAKE_CODE_BUILD` scripts other answers: `error` (reason `error` with a
//!   line), `garbled` (not JSON) and `incomplete` (`built` false without a `reason`). The project
//!   must be an absolute path, else it is a usage error.
//! - Any other command or option is a usage error (exit 2).

use serde_json::json;

use crate::memory_push;

/// Where `code enable|disable` remember the switch, in the working directory.
const SWITCH_FILE: &str = ".fake-code-switch";

/// Where the first `code build` remembers that it ran, in the working directory.
const BUILT_FILE: &str = ".fake-code-built";

/// `harness-memory code <args>`.
pub fn code(args: &[String]) -> ! {
    match args {
        [command, json] if command == "status" && json == "--json" => print_state(),
        [command, json] if (command == "enable" || command == "disable") && json == "--json" => {
            if std::env::var_os("FAKE_CODE_REFUSE").is_some() {
                eprintln!("harness-memory: cannot write code.toml: permission denied");
                std::process::exit(1);
            }

            std::fs::write(SWITCH_FILE, command).ok();
            print_state()
        }
        [command, option, project, json]
            if command == "build"
                && option == "--project"
                && json == "--json"
                && memory_push::lists("code-prepare") =>
        {
            build(project)
        }
        _ => {
            eprintln!("usage: harness-memory code status|enable|disable --json");
            std::process::exit(2);
        }
    }
}

/// `code build --project <project> --json`.
fn build(project: &str) -> ! {
    if !std::path::Path::new(project).is_absolute() {
        eprintln!("harness-memory: --project must be an absolute path");
        std::process::exit(2);
    }

    match std::env::var("FAKE_CODE_BUILD").as_deref() {
        Ok("garbled") => {
            println!("indexing {project}");
            std::process::exit(0);
        }
        Ok("incomplete") => answer(json!({ "built": false })),
        Ok("error") => answer(json!({ "built": false, "reason": "error", "error": "disk full" })),
        _ => {}
    }

    if !switch().0 {
        answer(json!({ "built": false, "reason": "off" }));
    }

    if std::path::Path::new(BUILT_FILE).exists() {
        answer(json!({ "built": false, "reason": "indexed" }));
    }

    std::fs::write(BUILT_FILE, project).ok();
    answer(json!({ "built": true, "files": 3 }))
}

fn answer(value: serde_json::Value) -> ! {
    println!("{value}");
    std::process::exit(0);
}

/// Whether the switch is on, and where that comes from.
fn switch() -> (bool, &'static str) {
    match std::fs::read_to_string(SWITCH_FILE).as_deref() {
        Ok(saved) => (saved == "enable", "file"),
        Err(_) => match std::env::var("FAKE_CODE_INDEX") {
            Ok(value) => (value != "off", "env"),
            Err(_) => (true, "default"),
        },
    }
}

fn print_state() -> ! {
    let (enabled, source) = switch();

    println!("{}", json!({ "enabled": enabled, "source": source }));
    std::process::exit(0);
}
