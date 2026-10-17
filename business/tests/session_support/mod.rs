//! Temporary homes with the saved-session fixtures (`tests/fixtures/sessions/{omp,pi}`) copied
//! into the harnesses' session folders for the project `<home>/Projects/demo`.
//!
//! The temp root of the environment is moved below the home, so the project counts as
//! home-relative for OMP. File times are fixed (newest first: draft, fork, main, release), so
//! listings and exported fixtures are deterministic.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use harness_business::env::Env;
use harness_business::harness::Harness;
use harness_business::harnesses::Pi;
use harness_business::model::{HarnessId, SessionRef};
use harness_business::pi_family::sessions::{SessionCache, epoch_millis, read_tree};
use tempfile::TempDir;

/// The main OMP session (two branch points, edits, a label, hidden entries).
pub const OMP_MAIN: &str = "0199c1f2-5d3e-7a10-9b2c-4e6f8a1d3c57";

/// An untitled OMP fork of [`OMP_MAIN`].
pub const OMP_FORK: &str = "0199c3a8-0b1e-7c42-8d5f-1a2b3c4d5e6f";

/// An older OMP session with a user title.
pub const OMP_RELEASE: &str = "01999e40-7d21-7f03-a4b6-c8d9e0f1a2b3";

/// The named PI session (branches, `session_info`, malformed, CRLF and U+2028 lines).
pub const PI_NAMED: &str = "8c2d4e6f-1a3b-4c5d-8e7f-9a0b1c2d3e4f";

/// An untitled PI session.
pub const PI_UNTITLED: &str = "5b7c9d1e-3f4a-4b6c-8d0e-2f4a6b8c0d1e";

/// Session file modification times by id fragment.
const FILE_TIMES: [(&str, &str); 6] = [
    ("0199d000", "2026-10-09T12:00:30Z"),
    (OMP_FORK, "2026-10-09T11:20:05Z"),
    (OMP_MAIN, "2026-10-09T10:17:30Z"),
    (OMP_RELEASE, "2026-10-07T16:05:48Z"),
    (PI_NAMED, "2026-10-08T09:47:31Z"),
    (PI_UNTITLED, "2026-10-06T18:41:10Z"),
];

/// A temporary home with a demo project.
pub struct SessionHome {
    /// Keeps the directory alive.
    pub dir: TempDir,
    /// The canonical home.
    pub home: PathBuf,
    /// The canonical project (`<home>/Projects/demo`, with a `src/` directory).
    pub project: PathBuf,
    /// The environment (home, temp root below the home).
    pub env: Env,
}

/// A home without sessions.
pub fn session_home() -> SessionHome {
    let dir = tempfile::tempdir().expect("temp home");
    let home = dunce::canonicalize(dir.path()).expect("canonical home");
    let project = home.join("Projects/demo");
    fs::create_dir_all(project.join("src")).expect("project");
    fs::create_dir_all(home.join("tmp")).expect("temp root");

    let env = Env::for_tests(&home).with_var("TMPDIR", home.join("tmp").to_string_lossy());

    SessionHome {
        dir,
        home,
        project,
        env,
    }
}

impl SessionHome {
    /// Copies the `fixture` (`omp` or `pi`) sessions into `harness`'s folder for the project.
    pub fn install(&self, harness: &dyn Harness, fixture: &str) -> PathBuf {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sessions")
            .join(fixture);
        let target = harness.sessions_dir(&self.env, &self.project);

        copy_tree(&source, &target);
        target
    }

    /// A home with both fixtures installed.
    pub fn with_fixtures(harnesses: &[(&dyn Harness, &str)]) -> Self {
        let home = session_home();
        for (harness, fixture) in harnesses {
            home.install(*harness, fixture);
        }
        home
    }
}

/// A session reference.
pub fn session(harness: HarnessId, id: &str) -> SessionRef {
    SessionRef {
        harness,
        id: id.to_owned(),
    }
}

/// Replaces `Which edition`, the start of the first user message of the untitled PI fixture, with
/// `sent` (written as is, so JSON-escape it) and checks that the session list's first prompt and
/// the tree preview still show only the user's own text.
pub fn assert_pi_previews_skip(sent: &str) {
    let home = SessionHome::with_fixtures(&[(&Pi, "pi")]);
    let reference = session(HarnessId::Pi, PI_UNTITLED);
    let path = Pi.resolve_session(&home.env, &home.project, &reference).unwrap();

    let original = fs::read_to_string(&path).unwrap();
    fs::write(&path, original.replace("Which edition", sent)).unwrap();

    let list = Pi
        .list_sessions(&home.env, &home.project, &SessionCache::default())
        .unwrap();
    let untitled = list.iter().find(|summary| summary.session.id == PI_UNTITLED).unwrap();
    assert_eq!(
        untitled.first_prompt.as_deref(),
        Some("Which edition does the crate use?")
    );

    let tree = read_tree(&path, &reference).unwrap().tree;
    let question = tree.nodes.iter().find(|node| node.id == "q01").unwrap();
    assert_eq!(question.preview.as_deref(), Some("Which edition does the crate use?"));
}

/// Sets the modification time of `path` (RFC 3339).
pub fn set_mtime(path: &Path, time: &str) {
    let millis = epoch_millis(time).expect("valid time");
    let when = UNIX_EPOCH + Duration::from_millis(u64::try_from(millis).expect("after 1970"));
    set_modified(path, when);
}

fn set_modified(path: &Path, when: SystemTime) {
    fs::File::options()
        .write(true)
        .open(path)
        .and_then(|file| file.set_modified(when))
        .expect("set file time");
}

fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir_all(target).expect("target folder");

    for entry in fs::read_dir(source).expect("fixture folder").flatten() {
        let from = entry.path();
        let to = target.join(entry.file_name());

        if from.is_dir() {
            copy_tree(&from, &to);
            continue;
        }

        fs::copy(&from, &to).expect("copy fixture");

        let name = entry.file_name().to_string_lossy().into_owned();
        let time = FILE_TIMES
            .iter()
            .find(|(fragment, _)| name.contains(fragment))
            .map_or("2026-10-01T00:00:00Z", |(_, time)| time);
        set_mtime(&to, time);
    }
}
