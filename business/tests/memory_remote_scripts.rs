//! The `scripts` routing key of Harness Memory's `config.toml`: read like the note routing, and
//! written only as `scripts = "local"` while the installed server keeps scripts (an older server
//! refuses a file with a key it does not know), and left as saved while the server cannot be
//! asked. `trust_team_scripts` follows the same rules, written only as `true` and only for a
//! server that asks before running a script, while scripts go to the team database.

use harness_business::ecosystem::{MemoryRemoteChange, ScriptsSupport, memory_remote_get, memory_remote_set};
use harness_business::env::{Env, OsKind};
use harness_business::model::{MemoryNotesTarget, MemoryRemoteKind};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

const URL: &str = "postgres://team@db.example.com/memory";

struct Home {
    _dir: TempDir,
    env: Env,
}

impl Home {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let env = Env::for_tests(dir.path()).with_os(OsKind::Linux);

        Self { _dir: dir, env }
    }

    fn config(&self) -> std::path::PathBuf {
        self.env.home.join(".config/harness-memory/config.toml")
    }

    /// The file's text after saving the team database with scripts routed to `scripts`.
    fn save(&self, scripts: MemoryNotesTarget, support: ScriptsSupport) -> String {
        self.save_routing(MemoryNotesTarget::Local, scripts, support)
    }

    /// The file's text after saving with repository notes routed to `repository_notes` too.
    fn save_routing(
        &self,
        repository_notes: MemoryNotesTarget,
        scripts: MemoryNotesTarget,
        support: ScriptsSupport,
    ) -> String {
        self.save_all(repository_notes, scripts, false, support)
    }

    /// The file's text after saving with `trust_team_scripts` too.
    fn save_trust(&self, trust_team_scripts: bool, support: ScriptsSupport) -> String {
        self.save_all(
            MemoryNotesTarget::Remote,
            MemoryNotesTarget::Remote,
            trust_team_scripts,
            support,
        )
    }

    fn save_all(
        &self,
        repository_notes: MemoryNotesTarget,
        scripts: MemoryNotesTarget,
        trust_team_scripts: bool,
        support: ScriptsSupport,
    ) -> String {
        let change = MemoryRemoteChange {
            enabled: true,
            kind: Some(MemoryRemoteKind::Postgres),
            url: Some(URL.to_owned()),
            token: None,
            repository_notes,
            global_notes: MemoryNotesTarget::Remote,
            scripts,
            trust_team_scripts,
        };

        memory_remote_set(&self.env, change, support).expect("saved");
        std::fs::read_to_string(self.config()).expect("written")
    }
}

fn scripts_lines(text: &str) -> Vec<&str> {
    text.lines().filter(|line| line.starts_with("scripts")).collect()
}

fn trust_lines(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| line.starts_with("trust_team_scripts"))
        .collect()
}

#[test]
fn local_scripts_round_trip_while_the_server_supports_them() {
    let home = Home::new();

    let text = home.save(MemoryNotesTarget::Local, ScriptsSupport::Supported);
    assert_eq!(scripts_lines(&text), ["scripts = \"local\""]);

    let remote = memory_remote_get(&home.env, ScriptsSupport::Supported);
    assert_eq!(remote.scripts, MemoryNotesTarget::Local);
    assert!(remote.scripts_supported);
    assert_eq!(remote.repository_notes, MemoryNotesTarget::Local);
    assert_eq!(remote.problem, None, "scripts is a known key");

    // Back to the team database: unset means remote, so the key goes away.
    let text = home.save(MemoryNotesTarget::Remote, ScriptsSupport::Supported);
    assert_eq!(scripts_lines(&text), Vec::<&str>::new());
    assert_eq!(
        memory_remote_get(&home.env, ScriptsSupport::Supported).scripts,
        MemoryNotesTarget::Remote
    );
}

#[test]
fn the_key_is_never_written_for_a_server_without_scripts() {
    let home = Home::new();

    let text = home.save(MemoryNotesTarget::Local, ScriptsSupport::Unsupported);
    assert_eq!(scripts_lines(&text), Vec::<&str>::new());

    let remote = memory_remote_get(&home.env, ScriptsSupport::Unsupported);
    assert_eq!(remote.scripts, MemoryNotesTarget::Remote);
    assert!(!remote.scripts_supported);

    // A saved `scripts = "local"` is dropped on the next save once the server no longer has it.
    home.save(MemoryNotesTarget::Local, ScriptsSupport::Supported);
    let text = home.save(MemoryNotesTarget::Local, ScriptsSupport::Unsupported);
    assert_eq!(scripts_lines(&text), Vec::<&str>::new());
}

#[test]
fn a_scripts_value_of_the_wrong_kind_is_reported_and_read_as_remote() {
    let home = Home::new();
    home.save(MemoryNotesTarget::Remote, ScriptsSupport::Supported);

    let path = home.config();
    let text = std::fs::read_to_string(&path).unwrap() + "scripts = 5\n";
    std::fs::write(&path, text).unwrap();

    let remote = memory_remote_get(&home.env, ScriptsSupport::Supported);
    assert_eq!(remote.scripts, MemoryNotesTarget::Remote);
    assert_eq!(
        remote.problem.as_deref(),
        Some("scripts must be \"remote\" or \"local\"")
    );
}

#[test]
fn a_server_that_cannot_be_asked_leaves_the_scripts_key_as_saved() {
    let home = Home::new();
    home.save(MemoryNotesTarget::Local, ScriptsSupport::Supported);

    // The probe failed: another change is saved, and the stored routing survives it.
    let text = home.save_routing(
        MemoryNotesTarget::Remote,
        MemoryNotesTarget::Remote,
        ScriptsSupport::Unknown,
    );
    assert_eq!(scripts_lines(&text), ["scripts = \"local\""]);

    let remote = memory_remote_get(&home.env, ScriptsSupport::Unknown);
    assert_eq!(
        remote.repository_notes,
        MemoryNotesTarget::Remote,
        "the other change is saved"
    );
    assert_eq!(
        remote.scripts,
        MemoryNotesTarget::Local,
        "the stored value is still shown"
    );
    assert!(!remote.scripts_supported);

    // Nor is the key added to a file without it.
    home.save(MemoryNotesTarget::Remote, ScriptsSupport::Supported);
    let text = home.save(MemoryNotesTarget::Local, ScriptsSupport::Unknown);
    assert_eq!(scripts_lines(&text), Vec::<&str>::new());
}

#[test]
fn a_server_that_asks_first_round_trips_trust_team_scripts() {
    let home = Home::new();

    let text = home.save_trust(true, ScriptsSupport::Approvals);
    assert_eq!(trust_lines(&text), ["trust_team_scripts = true"]);

    let remote = memory_remote_get(&home.env, ScriptsSupport::Approvals);
    assert!(remote.trust_team_scripts);
    assert!(remote.trust_team_scripts_supported);
    assert!(remote.scripts_supported, "a server that asks first keeps scripts");
    assert_eq!(remote.problem, None, "trust_team_scripts is a known key");

    // Off is the default: the key goes away rather than being written as false.
    let text = home.save_trust(false, ScriptsSupport::Approvals);
    assert_eq!(trust_lines(&text), Vec::<&str>::new());
    assert!(!memory_remote_get(&home.env, ScriptsSupport::Approvals).trust_team_scripts);

    // The scripts key is written for such a server as well.
    let text = home.save(MemoryNotesTarget::Local, ScriptsSupport::Approvals);
    assert_eq!(scripts_lines(&text), ["scripts = \"local\""]);
}

#[test]
fn trust_team_scripts_is_dropped_while_scripts_stay_local() {
    let home = Home::new();
    home.save_trust(true, ScriptsSupport::Approvals);

    // Keeping scripts local leaves no team scripts to trust: the key goes, so switching back to
    // the team later does not trust them unasked.
    let text = home.save_all(
        MemoryNotesTarget::Remote,
        MemoryNotesTarget::Local,
        true,
        ScriptsSupport::Approvals,
    );
    assert_eq!(trust_lines(&text), Vec::<&str>::new());
    assert_eq!(scripts_lines(&text), ["scripts = \"local\""]);
    assert!(!memory_remote_get(&home.env, ScriptsSupport::Approvals).trust_team_scripts);

    // While the server cannot be asked the saved key stays, whatever the routing.
    home.save_trust(true, ScriptsSupport::Approvals);
    let text = home.save_all(
        MemoryNotesTarget::Remote,
        MemoryNotesTarget::Local,
        true,
        ScriptsSupport::Unknown,
    );
    assert_eq!(trust_lines(&text), ["trust_team_scripts = true"]);
}

#[test]
fn trust_team_scripts_is_never_written_for_an_older_server() {
    let home = Home::new();

    for support in [ScriptsSupport::Supported, ScriptsSupport::Unsupported] {
        let text = home.save_trust(true, support);
        assert_eq!(trust_lines(&text), Vec::<&str>::new(), "{support:?}");

        let remote = memory_remote_get(&home.env, support);
        assert!(!remote.trust_team_scripts_supported, "{support:?}");
        assert!(!remote.trust_team_scripts, "{support:?}");
    }

    // A saved key is dropped on the next save once the server no longer knows it.
    home.save_trust(true, ScriptsSupport::Approvals);
    let text = home.save_trust(true, ScriptsSupport::Supported);
    assert_eq!(trust_lines(&text), Vec::<&str>::new());
}

#[test]
fn a_server_that_cannot_be_asked_leaves_trust_team_scripts_as_saved() {
    let home = Home::new();
    home.save_trust(true, ScriptsSupport::Approvals);

    let text = home.save_trust(false, ScriptsSupport::Unknown);
    assert_eq!(trust_lines(&text), ["trust_team_scripts = true"]);

    let remote = memory_remote_get(&home.env, ScriptsSupport::Unknown);
    assert!(remote.trust_team_scripts, "the stored value is still shown");
    assert!(!remote.trust_team_scripts_supported);

    // Nor is the key added to a file without it.
    home.save_trust(false, ScriptsSupport::Approvals);
    let text = home.save_trust(true, ScriptsSupport::Unknown);
    assert_eq!(trust_lines(&text), Vec::<&str>::new());
}

#[test]
fn a_trust_team_scripts_value_of_the_wrong_kind_is_reported_and_read_as_off() {
    let home = Home::new();
    home.save_trust(false, ScriptsSupport::Approvals);

    let path = home.config();
    let text = std::fs::read_to_string(&path).unwrap() + "trust_team_scripts = \"yes\"\n";
    std::fs::write(&path, text).unwrap();

    let remote = memory_remote_get(&home.env, ScriptsSupport::Approvals);
    assert!(!remote.trust_team_scripts);
    assert_eq!(
        remote.problem.as_deref(),
        Some("trust_team_scripts must be true or false")
    );
}
