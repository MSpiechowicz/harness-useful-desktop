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
//!
//! Only "no such process" (or a zombie) counts as gone. Any other failure to read (a transient
//! `/proc` error, `hidepid`, another pid namespace, `ps` failing with output) leaves it unknown:
//! a recorded app is then taken as still running, so its children are never killed.

use std::ffi::OsString;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// How often [`ProcessIdentity::capture`] reads a command line that does not match.
const CAPTURE_ATTEMPTS: usize = 20;

/// The pause between those reads.
const CAPTURE_RETRY: Duration = Duration::from_millis(2);

/// How long [`ProcessIdentity::capture`] waits for a program that is still loading (no command
/// line at all yet). It ends as soon as the program is in place, a cold disk can take this long.
const CAPTURE_LOAD_LIMIT: Duration = Duration::from_secs(2);

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

/// What the OS reports about a pid.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    not(any(target_os = "linux", target_os = "macos", test)),
    expect(dead_code, reason = "elsewhere every pid is unknown")
)]
pub(crate) enum Observation {
    /// A process runs under the pid (not a zombie).
    Running(Observed),
    /// No process has the pid, or only a zombie.
    Gone,
    /// The OS could not tell (see the module docs).
    Unknown,
}

/// `ESRCH` ("no such process"), the same number on Linux and macOS.
#[cfg(any(target_os = "linux", test))]
const ESRCH: i32 = 3;

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
    /// A process just spawned shows an empty command line until the kernel has loaded its program
    /// (glibc's `posix_spawn` returns before that), so an empty one is read again for up to
    /// [`CAPTURE_LOAD_LIMIT`]. One that does not match is read a few more times only.
    pub fn capture(pid: u32, args: &[OsString]) -> Option<Self> {
        Self::capture_from(pid, args, || observe(pid))
    }

    /// [`ProcessIdentity::capture`] with the process read by `read` (tests script what it shows).
    fn capture_from(pid: u32, args: &[OsString], mut read: impl FnMut() -> Observation) -> Option<Self> {
        let args: Vec<String> = args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect();
        let began = Instant::now();
        let mut mismatches = 0;

        loop {
            let Observation::Running(observed) = read() else {
                return None;
            };

            if ends_with_args(&observed.command, &args) {
                return Some(Self {
                    pid,
                    started: observed.started,
                    args,
                });
            }

            let again = if observed.command.is_empty() {
                began.elapsed() < CAPTURE_LOAD_LIMIT
            } else {
                mismatches += 1;
                mismatches < CAPTURE_ATTEMPTS
            };

            if !again {
                tracing::debug!(pid, "a started process's command line does not end with its arguments");

                return None;
            }

            std::thread::sleep(CAPTURE_RETRY);
        }
    }

    /// The identity of the current process; `None` where identities are not supported.
    pub fn current() -> Option<Self> {
        let args: Vec<OsString> = std::env::args_os().skip(1).collect();

        Self::capture(std::process::id(), &args)
    }

    /// True while process [`ProcessIdentity::pid`] runs and is still this process, and when the OS
    /// cannot tell (see the module docs): false only when it is provably gone.
    pub fn is_alive(&self) -> bool {
        self.may_be_running(&observe(self.pid))
    }

    /// What the OS reports about [`ProcessIdentity::pid`], when that is provably still this
    /// process.
    pub(crate) fn observe_matching(&self) -> Option<Observed> {
        match observe(self.pid) {
            Observation::Running(observed) if self.is(&observed) => Some(observed),
            _ => None,
        }
    }

    /// False only when `observation` proves this process gone: no process, or another one.
    fn may_be_running(&self, observation: &Observation) -> bool {
        match observation {
            Observation::Running(observed) => self.is(observed),
            Observation::Gone => false,
            Observation::Unknown => true,
        }
    }

    /// True when `observed` started when this process did, with its arguments.
    fn is(&self, observed: &Observed) -> bool {
        observed.started == self.started && ends_with_args(&observed.command, &self.args)
    }
}

/// True unless no process runs under `pid` (see the module docs): a pid the OS cannot tell about,
/// or one it gave to another program since, counts as running. For a pid another program
/// recorded, where no [`ProcessIdentity`] is known.
pub(crate) fn pid_may_be_running(pid: u32) -> bool {
    !matches!(observe(pid), Observation::Gone)
}

/// A failed read about a process: gone when it does not exist, else unknown.
#[cfg(any(target_os = "linux", test))]
fn unreadable(error: &std::io::Error) -> Observation {
    let missing = error.kind() == std::io::ErrorKind::NotFound || error.raw_os_error() == Some(ESRCH);

    if missing {
        Observation::Gone
    } else {
        Observation::Unknown
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
pub(crate) fn observe(pid: u32) -> Observation {
    observe_proc(pid, boot_id(), |path| std::fs::read(path))
}

/// [`observe`] with the files read by `read` (tests inject read errors).
#[cfg(target_os = "linux")]
fn observe_proc(pid: u32, boot_id: &str, read: impl Fn(&str) -> std::io::Result<Vec<u8>>) -> Observation {
    let stat = match read(&format!("/proc/{pid}/stat")) {
        Ok(stat) => String::from_utf8_lossy(&stat).into_owned(),
        Err(error) => return unreadable(&error),
    };

    let Some((state, group, ticks)) = parse_proc_stat(&stat) else {
        return Observation::Unknown;
    };

    if state == "Z" || state == "X" {
        return Observation::Gone;
    }

    let cmdline = match read(&format!("/proc/{pid}/cmdline")) {
        Ok(cmdline) => cmdline,
        Err(error) => return unreadable(&error),
    };

    let command = cmdline
        .split(|byte| *byte == 0)
        .filter(|arg| !arg.is_empty())
        .map(|arg| String::from_utf8_lossy(arg).into_owned())
        .collect();

    Observation::Running(Observed {
        started: format!("{boot_id}:{ticks}"),
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
pub(crate) fn observe(pid: u32) -> Observation {
    use std::process::{Command, Stdio};

    let output = Command::new("/bin/ps")
        .args([
            "-ww", "-o", "pgid=", "-o", "stat=", "-o", "lstart=", "-o", "command=", "-p",
        ])
        .arg(pid.to_string())
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output();

    match output {
        Ok(output) => ps_observation(
            output.status.code(),
            &String::from_utf8_lossy(&output.stdout),
            &String::from_utf8_lossy(&output.stderr),
            pid,
        ),
        Err(_) => Observation::Unknown,
    }
}

/// What a `ps` run says about `pid`. Only the exact "no such process" result is gone: `ps -p`
/// exits 1 and prints nothing on either stream then. Any other failure (a signal, another exit
/// code, an error message from a sandbox or a broken spawn) is unknown, so a live owner's
/// servers are never killed over a `ps` that could not look.
#[cfg(any(target_os = "macos", test))]
fn ps_observation(code: Option<i32>, stdout: &str, stderr: &str, pid: u32) -> Observation {
    if code == Some(0) {
        return parse_ps_line(stdout, pid);
    }

    let silent = stdout.trim().is_empty() && stderr.trim().is_empty();

    if code == Some(1) && silent {
        Observation::Gone
    } else {
        Observation::Unknown
    }
}

/// Parses `ps -o pgid= -o stat= -o lstart= -o command=` output for `pid`: the group, the state
/// (`Z…` is a zombie, gone), the five words of `lstart` in the C locale (`Tue Oct 21 10:00:00
/// 2026`) and the command line split into words. Output that does not read so is unknown.
#[cfg(any(target_os = "macos", test))]
fn parse_ps_line(output: &str, pid: u32) -> Observation {
    let words: Vec<&str> = output
        .lines()
        .next()
        .map(|line| line.split_whitespace().collect())
        .unwrap_or_default();

    let (Some(group), Some(state)) = (words.first().and_then(|group| group.parse::<u32>().ok()), words.get(1)) else {
        return Observation::Unknown;
    };

    if state.starts_with('Z') {
        return Observation::Gone;
    }

    if words.len() < 8 {
        return Observation::Unknown;
    }

    Observation::Running(Observed {
        started: words[2..7].join(" "),
        command: words[7..].iter().map(|word| (*word).to_owned()).collect(),
        group_leader: group == pid,
    })
}

/// Other systems report nothing, so no recorded process is ever killed there.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(crate) fn observe(_pid: u32) -> Observation {
    Observation::Unknown
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
mod tests;
