//! Telling whether a process id still names the process the app started, so a recorded pid that
//! the OS gave to another process since is never signalled.
//!
//! A [`ProcessIdentity`] is a pid plus the start time the OS reports for it and the arguments the
//! process was started with. A process matches it while it runs (not a zombie), started at the same
//! time and its command line still ends with those arguments.
//!
//! - Linux: `/proc/<pid>/stat` (start time in clock ticks since boot, process group, state), the
//!   boot id (`/proc/sys/kernel/random/boot_id`, so a pid and tick count from an earlier boot never
//!   match) and `/proc/<pid>/cmdline`.
//! - macOS: `/bin/ps -ww -o pgid= -o stat= -o lstart= -o command= -p <pid>` in the C locale (the
//!   start time to the second).
//! - Elsewhere (Windows): nothing is known, so nothing matches and nothing is ever killed.

use std::ffi::OsString;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// How often [`ProcessIdentity::capture`] reads a command line that does not match yet.
const CAPTURE_ATTEMPTS: usize = 20;

/// The pause between those reads (at most 40 ms in all).
const CAPTURE_RETRY: Duration = Duration::from_millis(2);

/// A process the app started, as recorded when it started (see the module docs).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessIdentity {
    /// The OS process id.
    pub pid: u32,
    /// When the process started, as the OS reports it (Linux: boot id and clock ticks since boot,
    /// macOS: `ps`'s `lstart`).
    pub started: String,
    /// The arguments after the program, which its command line must still end with.
    pub args: Vec<String>,
}

/// What the OS reports about a running process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Observed {
    /// See [`ProcessIdentity::started`].
    pub started: String,
    /// The whole command line (program and arguments).
    pub command: Vec<String>,
    /// True when the process leads its process group (the app's children are spawned that way).
    pub group_leader: bool,
}

impl ProcessIdentity {
    /// The identity of running process `pid`, started with `args` after its program; `None` when
    /// the process is gone, its command line does not end with `args` or the OS is not supported.
    ///
    /// A process just spawned may not show its command line yet (glibc's `posix_spawn` returns
    /// once the new program replaced the old one, before its arguments are in place), so a
    /// mismatch is read again a few times, [`CAPTURE_RETRY`] apart.
    pub fn capture(pid: u32, args: &[OsString]) -> Option<Self> {
        let args: Vec<String> = args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect();

        for _ in 0..CAPTURE_ATTEMPTS {
            let observed = observe(pid)?;

            if ends_with_args(&observed.command, &args) {
                return Some(Self {
                    pid,
                    started: observed.started,
                    args,
                });
            }

            std::thread::sleep(CAPTURE_RETRY);
        }

        tracing::debug!(pid, "a started process's command line does not end with its arguments");

        None
    }

    /// The identity of the current process; `None` where identities are not supported.
    pub fn current() -> Option<Self> {
        let args: Vec<OsString> = std::env::args_os().skip(1).collect();

        Self::capture(std::process::id(), &args)
    }

    /// True while process [`ProcessIdentity::pid`] runs and is still this process.
    pub fn is_alive(&self) -> bool {
        self.observe_matching().is_some()
    }

    /// What the OS reports about [`ProcessIdentity::pid`], when that is still this process.
    pub(crate) fn observe_matching(&self) -> Option<Observed> {
        let observed = observe(self.pid)?;

        let same = observed.started == self.started && ends_with_args(&observed.command, &self.args);

        same.then_some(observed)
    }
}

/// True when `command` ends with `args`, compared word by word (`ps` joins the command line with
/// spaces, so an argument containing one cannot be told apart from two).
fn ends_with_args(command: &[String], args: &[String]) -> bool {
    let words = |items: &[String]| -> Vec<String> {
        items
            .iter()
            .flat_map(|item| item.split_whitespace())
            .map(str::to_owned)
            .collect()
    };

    words(command).ends_with(&words(args))
}

/// Linux: reads `/proc` (see the module docs). A zombie counts as gone.
#[cfg(target_os = "linux")]
pub(crate) fn observe(pid: u32) -> Option<Observed> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (state, group, ticks) = parse_proc_stat(&stat)?;

    if state == "Z" || state == "X" {
        return None;
    }

    let cmdline = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let command = cmdline
        .split(|byte| *byte == 0)
        .filter(|arg| !arg.is_empty())
        .map(|arg| String::from_utf8_lossy(arg).into_owned())
        .collect();

    Some(Observed {
        started: format!("{}:{ticks}", boot_id()),
        command,
        group_leader: group == pid,
    })
}

/// The state, process group and start time (clock ticks since boot) in a `/proc/<pid>/stat` line.
/// The command name in parentheses may hold spaces and parentheses, so fields count from the last
/// `)`.
#[cfg(any(target_os = "linux", test))]
fn parse_proc_stat(stat: &str) -> Option<(String, u32, u64)> {
    let rest = &stat[stat.rfind(')')? + 1..];
    let fields: Vec<&str> = rest.split_whitespace().collect();

    // Field 3 (state) is the first after the name; field 5 is the group, field 22 the start time.
    let state = (*fields.first()?).to_owned();
    let group = fields.get(2)?.parse().ok()?;
    let ticks = fields.get(19)?.parse().ok()?;

    Some((state, group, ticks))
}

/// This boot's id, read once; empty when unreadable.
#[cfg(target_os = "linux")]
fn boot_id() -> &'static str {
    static BOOT_ID: std::sync::OnceLock<String> = std::sync::OnceLock::new();

    BOOT_ID.get_or_init(|| {
        std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .map(|id| id.trim().to_owned())
            .unwrap_or_default()
    })
}

/// macOS: asks `ps` (see the module docs). A zombie counts as gone.
#[cfg(target_os = "macos")]
pub(crate) fn observe(pid: u32) -> Option<Observed> {
    use std::process::{Command, Stdio};

    let output = Command::new("/bin/ps")
        .args([
            "-ww", "-o", "pgid=", "-o", "stat=", "-o", "lstart=", "-o", "command=", "-p",
        ])
        .arg(pid.to_string())
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    parse_ps_line(&String::from_utf8_lossy(&output.stdout), pid)
}

/// Parses `ps -o pgid= -o stat= -o lstart= -o command=` output for `pid`: the group, the state
/// (`Z…` is a zombie), the five words of `lstart` in the C locale (`Tue Oct 21 10:00:00 2026`) and
/// the command line split into words.
#[cfg(any(target_os = "macos", test))]
fn parse_ps_line(output: &str, pid: u32) -> Option<Observed> {
    let words: Vec<&str> = output.lines().next()?.split_whitespace().collect();
    let group: u32 = words.first()?.parse().ok()?;
    let state = words.get(1)?;

    if state.starts_with('Z') || words.len() < 8 {
        return None;
    }

    Some(Observed {
        started: words[2..7].join(" "),
        command: words[7..].iter().map(|word| (*word).to_owned()).collect(),
        group_leader: group == pid,
    })
}

/// Other systems report nothing, so no recorded process is ever killed there.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(crate) fn observe(_pid: u32) -> Option<Observed> {
    None
}

/// Sends SIGKILL to the process group `observed` leads, or to the process alone when it does not
/// lead one.
#[cfg(unix)]
pub(crate) fn kill_observed(pid: u32, observed: &Observed) {
    if observed.group_leader {
        super::kill_group(pid);
        return;
    }

    let Ok(raw) = i32::try_from(pid) else {
        return;
    };

    if let Err(error) = nix::sys::signal::kill(nix::unistd::Pid::from_raw(raw), nix::sys::signal::Signal::SIGKILL)
        && error != nix::errno::Errno::ESRCH
    {
        tracing::debug!(%error, "kill failed");
    }
}

/// Nothing is ever observed elsewhere, so nothing is killed.
#[cfg(not(unix))]
pub(crate) fn kill_observed(_pid: u32, _observed: &Observed) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proc_stat_fields_count_from_the_last_parenthesis() {
        let stat = "4242 (open (code) x) S 1 4242 4242 0 -1 4194560 100 0 0 0 5 3 0 0 20 0 12 0 987654 \
                    1000 200 18446744073709551615";

        assert_eq!(parse_proc_stat(stat), Some(("S".to_owned(), 4242, 987_654)));
        assert_eq!(parse_proc_stat("garbage"), None);
    }

    #[test]
    fn ps_output_gives_group_start_and_command() {
        let line = "  501 Ss   Tue Oct  7 09:05:01 2026     /Users/u/.opencode/bin/opencode serve --port 0\n";
        let observed = parse_ps_line(line, 501).unwrap();

        assert_eq!(observed.started, "Tue Oct 7 09:05:01 2026");
        assert_eq!(observed.command.last().map(String::as_str), Some("0"));
        assert!(observed.group_leader);

        assert!(!parse_ps_line(line, 777).unwrap().group_leader);
        assert_eq!(parse_ps_line("  501 Z    Tue Oct  7 09:05:01 2026 x\n", 501), None);
        assert_eq!(parse_ps_line("", 501), None);
    }

    #[test]
    fn the_command_line_must_end_with_the_recorded_arguments() {
        let command: Vec<String> = ["/usr/bin/env", "bun", "/x/opencode", "serve", "--port", "0"]
            .map(str::to_owned)
            .to_vec();

        assert!(ends_with_args(&command, &["serve".to_owned(), "--port 0".to_owned()]));
        assert!(ends_with_args(&command, &[]));
        assert!(!ends_with_args(&command, &["serve".to_owned()]));
    }
}
