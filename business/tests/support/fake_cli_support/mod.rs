//! A fake harness CLI for the extension action tests: a shell script in a temporary directory (the
//! real harness binaries never run here) that records what it is asked to into files beside it.
#![cfg(unix)]
#![allow(dead_code)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use harness_business::process::ResolvedBinary;
use tempfile::TempDir;

/// Copies `from` to `to` in a `cp` process, so this process never holds `to` open for writing.
/// A child another test thread forks meanwhile inherits such a descriptor until it execs, and
/// running `to` then fails with "Text file busy" (ETXTBSY).
fn written_by_a_child(from: &Path, to: &Path) {
    let copied = Command::new("cp").arg(from).arg(to).status().expect("run cp");
    assert!(copied.success(), "cp {} failed", from.display());
}

/// An executable `<dir>/bin/<name>`, with a home directory at `<dir>/home`.
pub struct FakeCli {
    dir: TempDir,
    pub binary: ResolvedBinary,
}

impl FakeCli {
    /// A script that runs `body` with `d` set to its directory (where it records into files).
    pub fn script(name: &str, body: &str) -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).expect("create bin");

        let shebang = "#!/bin/sh"; // guard-allow: installer-shell
        let path = bin.join(name);
        let draft = dir.path().join(format!("{name}.draft"));
        std::fs::write(&draft, format!("{shebang}\nd=$(dirname \"$0\")\n{body}")).expect("write script");
        written_by_a_child(&draft, &path);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");

        Self {
            dir,
            binary: ResolvedBinary {
                path,
                is_batch_script: false,
            },
        }
    }

    /// Writes `text` to `file` beside the script.
    pub fn place(&self, file: &str, text: &str) {
        std::fs::write(self.dir.path().join("bin").join(file), text).expect("write file");
    }

    /// What the script recorded in `file`, when it ran.
    pub fn recorded(&self, file: &str) -> Option<String> {
        std::fs::read_to_string(self.dir.path().join("bin").join(file)).ok()
    }

    pub fn home(&self) -> PathBuf {
        let home = self.dir.path().join("home");
        std::fs::create_dir_all(&home).expect("create home");
        home
    }
}
