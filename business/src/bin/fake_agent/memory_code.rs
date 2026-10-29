//! `harness-memory code …` stand-in (the code index switch). The switch is kept between runs in
//! `.fake-code-switch` of the working directory.
//!
//! - `code status --json` prints `{"enabled": …, "source": …}`: `file` with what the last
//!   `enable|disable` of that working directory said, else `env` with `FAKE_CODE_INDEX` (`off` is
//!   off, anything else on), else `default` (on).
//! - `code enable|disable --json` remember the switch and print the state they left, or exit 1
//!   with the server's line when `FAKE_CODE_REFUSE` is set.
//! - Any other command or option is a usage error (exit 2).

use serde_json::json;

/// Where `code enable|disable` remember the switch, in the working directory.
const SWITCH_FILE: &str = ".fake-code-switch";

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
        _ => {
            eprintln!("usage: harness-memory code status|enable|disable --json");
            std::process::exit(2);
        }
    }
}

fn print_state() -> ! {
    let (enabled, source) = match std::fs::read_to_string(SWITCH_FILE).as_deref() {
        Ok(saved) => (saved == "enable", "file"),
        Err(_) => match std::env::var("FAKE_CODE_INDEX") {
            Ok(value) => (value != "off", "env"),
            Err(_) => (true, "default"),
        },
    };

    println!("{}", json!({ "enabled": enabled, "source": source }));
    std::process::exit(0);
}
