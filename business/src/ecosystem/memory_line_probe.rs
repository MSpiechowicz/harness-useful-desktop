//! A probe of the installed Harness Memory that runs one command printing one word per line
//! (`harness-memory kinds`, `harness-memory features`) and keeps the answer per binary, shared by
//! [`memory_kinds`](super::memory_kinds) and [`memory_features`](super::memory_features).
//!
//! [`LineProbe::ask`] runs the command once per binary: the answer is cached until the binary at
//! that path changes (its modification time, length, or on Unix its device and inode). A usage
//! error (exit 2, or a line naming an unknown command) is cached as an older server, any other
//! failure answers [`LineAnswer::Unsupported`] without being cached, and a server that does not
//! start or answer in time is [`LineAnswer::Unknown`] (not cached either).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use super::memory_binary::resolve_memory;
use super::memory_cli::{CliAnswer, CliCall, USAGE_EXIT, run_memory_cli};
use crate::env::Env;
use crate::process::ProcessRunner;

/// How long the command may take.
pub const LINE_PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// The output kept of the probe (the answer is a few short lines).
const PROBE_OUTPUT_BYTES: usize = 16 * 1024;

/// What one probe found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LineAnswer {
    /// It answered the command: its lines, trimmed and lowercased, blank ones dropped.
    Listed(Vec<String>),
    /// It predates the command (or failed it).
    Unsupported,
    /// Harness Memory is not installed.
    NotInstalled,
    /// The server could not be asked (it did not start or answer in time).
    Unknown,
}

impl LineAnswer {
    /// Whether the server listed `line` (compared lowercase).
    pub(crate) fn lists(&self, line: &str) -> bool {
        match self {
            Self::Listed(lines) => lines.iter().any(|listed| listed == line),
            _ => false,
        }
    }
}

/// The cached probe of one command. See the module docs.
#[derive(Debug)]
pub(crate) struct LineProbe {
    command: &'static str,
    probe: Mutex<Option<Probe>>,
}

/// A cached answer of one binary, as it was when probed.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Probe {
    path: PathBuf,
    stamp: Stamp,
    answer: LineAnswer,
}

/// What tells one binary at a path from another: a replaced file gets a new inode (Unix) even when
/// its time and length match the old one's.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    modified: SystemTime,
    len: u64,
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
}

impl Stamp {
    /// The stamp of the file at `path`; `None` when it cannot be read.
    fn of(path: &Path) -> Option<Self> {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;

        let metadata = std::fs::metadata(path).ok()?;

        Some(Self {
            modified: metadata.modified().ok()?,
            len: metadata.len(),
            #[cfg(unix)]
            dev: metadata.dev(),
            #[cfg(unix)]
            ino: metadata.ino(),
        })
    }
}

/// What one run of the command says, and whether it may be cached.
struct Answer {
    lines: LineAnswer,
    cache: bool,
}

impl LineProbe {
    /// No probe yet of `harness-memory <command>`.
    pub(crate) const fn new(command: &'static str) -> Self {
        Self {
            command,
            probe: Mutex::new(None),
        }
    }

    /// What the installed server answers (see the module docs).
    pub(crate) async fn ask(&self, env: &Env, runner: &dyn ProcessRunner) -> LineAnswer {
        let Some(binary) = resolve_memory(env) else {
            return LineAnswer::NotInstalled;
        };

        let stamp = Stamp::of(&binary.path);

        if let Some(stamp) = &stamp
            && let Some(answer) = self.cached(&binary.path, stamp)
        {
            return answer;
        }

        let answer = self.run(env, runner, &binary.path).await;

        if answer.cache
            && let Some(stamp) = stamp
        {
            self.remember(Probe {
                path: binary.path,
                stamp,
                answer: answer.lines.clone(),
            });
        }

        answer.lines
    }

    fn cached(&self, path: &Path, stamp: &Stamp) -> Option<LineAnswer> {
        let probe = self.probe.lock().unwrap_or_else(PoisonError::into_inner);

        probe
            .as_ref()
            .filter(|probe| probe.path == path && probe.stamp == *stamp)
            .map(|probe| probe.answer.clone())
    }

    fn remember(&self, probe: Probe) {
        *self.probe.lock().unwrap_or_else(PoisonError::into_inner) = Some(probe);
    }

    /// The command on `server`.
    async fn run(&self, env: &Env, runner: &dyn ProcessRunner, server: &Path) -> Answer {
        let command = self.command;
        let call = CliCall {
            name: command,
            args: vec![OsString::from(command)],
            timeout: LINE_PROBE_TIMEOUT,
            output_bytes: PROBE_OUTPUT_BYTES,
            stdin: None,
        };

        match run_memory_cli(env, runner, server, call).await {
            Ok(CliAnswer::Done(stdout)) => {
                let lines = stdout
                    .lines()
                    .map(|line| line.trim().to_lowercase())
                    .filter(|line| !line.is_empty())
                    .collect();

                Answer {
                    lines: LineAnswer::Listed(lines),
                    cache: true,
                }
            }
            Ok(CliAnswer::Failed {
                status: Some(code),
                message,
                ..
            }) => {
                let older = code == USAGE_EXIT || names_unknown_command(&message);

                if !older {
                    tracing::warn!(code, command, "a harness-memory probe failed");
                }

                Answer {
                    lines: LineAnswer::Unsupported,
                    cache: older,
                }
            }
            Ok(CliAnswer::Failed { status: None, .. }) => {
                tracing::warn!(command, "a harness-memory probe was stopped");
                unknown()
            }
            Err(error) => {
                tracing::warn!(command, code = ?error.code(), "a harness-memory probe did not run");
                unknown()
            }
        }
    }
}

fn unknown() -> Answer {
    Answer {
        lines: LineAnswer::Unknown,
        cache: false,
    }
}

/// True when the server's line says it does not know the command (a server whose usage errors do
/// not exit 2).
fn names_unknown_command(message: &str) -> bool {
    let message = message.to_lowercase();

    ["unrecognized subcommand", "unknown subcommand", "unknown command"]
        .iter()
        .any(|marker| message.contains(marker))
}
