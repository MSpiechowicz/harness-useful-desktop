//! Retiring the old copy of the bundled memory server (`<local data>/bin/harness-memory` on macOS
//! and Windows): the app's own entries move to the installed server, the user's entries stay, and
//! the copy is deleted only once nothing runs it, never through a symlink and never on Linux.

use std::path::{Path, PathBuf};

use harness_business::app_dirs::legacy_memory_copy;
use harness_business::ecosystem::{LegacyCopy, LegacyMigration, migrate_legacy_memory};
use harness_business::env::{Env, OsKind};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tempfile::TempDir;

/// A macOS home with the old copy in place and an OMP user MCP file (plus the `.mcp.json` it also
/// reads).
struct Mac {
    dir: TempDir,
    env: Env,
}

impl Mac {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let env = Env::for_tests(dir.path()).with_os(OsKind::MacOs);
        let mac = Self { dir, env };

        write(&mac.legacy(), "old server");
        mac
    }

    fn legacy(&self) -> PathBuf {
        legacy_memory_copy(&self.env).expect("macOS has an old copy")
    }

    /// The installed server (never read or run here).
    fn server(&self) -> PathBuf {
        self.dir.path().join(".local/bin/harness-memory")
    }

    fn mcp(&self, name: &str) -> PathBuf {
        self.dir.path().join(".omp/agent").join(name)
    }

    fn targets(&self) -> Vec<(PathBuf, Vec<PathBuf>)> {
        vec![(self.mcp("mcp.json"), vec![self.mcp(".mcp.json")])]
    }

    fn migrate(&self, server: Option<&Path>) -> LegacyMigration {
        migrate_legacy_memory(&self.env, &self.targets(), server)
    }

    fn migrate_installed(&self) -> LegacyMigration {
        self.migrate(Some(&self.server()))
    }

    fn legacy_command(&self) -> String {
        self.legacy().to_string_lossy().into_owned()
    }
}

/// Writes `text` to `path`, creating its folders.
fn write(path: &Path, text: impl AsRef<[u8]>) {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).unwrap();
    }

    std::fs::write(path, text).unwrap();
}

fn write_json(path: &Path, value: &Value) {
    write(path, serde_json::to_string_pretty(value).unwrap());
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn the_apps_entry_moves_to_the_installed_server_and_the_old_copy_goes() {
    let mac = Mac::new();
    let original = json!({
        "mcpServers": {
            "github": { "command": "github-mcp" },
            "harness-memory": { "command": mac.legacy_command(), "args": ["mcp"], "env": { "A": "1" } }
        },
        "disabledServers": []
    });
    write_json(&mac.mcp("mcp.json"), &original);

    let migration = mac.migrate_installed();

    assert_eq!(
        migration,
        LegacyMigration {
            repointed: 1,
            copy: LegacyCopy::Removed
        }
    );
    let mut expected = original;
    expected["mcpServers"]["harness-memory"]["command"] = json!(mac.server().to_string_lossy());
    assert_eq!(read_json(&mac.mcp("mcp.json")), expected);
    assert!(!mac.legacy().exists());
    assert!(!mac.legacy().parent().unwrap().exists(), "the emptied folder goes too");

    assert_eq!(
        mac.migrate_installed(),
        LegacyMigration {
            repointed: 0,
            copy: LegacyCopy::Absent
        },
        "a second run finds nothing to do"
    );
}

#[test]
fn without_an_installed_server_nothing_changes() {
    let mac = Mac::new();
    let original = json!({ "mcpServers": { "harness-memory": { "command": mac.legacy_command(), "args": ["mcp"] } } });
    write_json(&mac.mcp("mcp.json"), &original);

    for server in [None, Some(mac.legacy())] {
        let migration = mac.migrate(server.as_deref());

        assert_eq!(migration.copy, LegacyCopy::NotInstalled);
        assert_eq!(migration.repointed, 0);
        assert_eq!(read_json(&mac.mcp("mcp.json")), original);
        assert!(mac.legacy().exists());
    }
}

#[test]
fn the_users_own_entries_stay_and_keep_the_old_copy() {
    let mac = Mac::new();
    // Not the app's: another name, and the right name with other arguments.
    let target = json!({ "mcpServers": { "memory": { "command": mac.legacy_command(), "args": ["mcp"] } } });
    let also_read =
        json!({ "mcpServers": { "harness-memory": { "command": mac.legacy_command(), "args": ["mcp", "-v"] } } });
    write_json(&mac.mcp("mcp.json"), &target);
    write_json(&mac.mcp(".mcp.json"), &also_read);

    let migration = mac.migrate_installed();

    assert_eq!(
        migration,
        LegacyMigration {
            repointed: 0,
            copy: LegacyCopy::InUse
        }
    );
    assert_eq!(read_json(&mac.mcp("mcp.json")), target);
    assert_eq!(read_json(&mac.mcp(".mcp.json")), also_read);
    assert!(mac.legacy().exists());
}

#[test]
fn an_entry_only_in_a_file_the_harness_also_reads_is_never_changed() {
    let mac = Mac::new();
    let also_read = json!({ "mcpServers": { "harness-memory": { "command": mac.legacy_command(), "args": ["mcp"] } } });
    write_json(&mac.mcp(".mcp.json"), &also_read);

    let migration = mac.migrate_installed();

    assert_eq!(migration.repointed, 0);
    assert_eq!(migration.copy, LegacyCopy::InUse);
    assert_eq!(read_json(&mac.mcp(".mcp.json")), also_read);
}

#[test]
fn an_entry_that_runs_another_server_is_left_alone() {
    let mac = Mac::new();
    let original =
        json!({ "mcpServers": { "harness-memory": { "command": "/opt/custom/harness-memory", "args": ["mcp"] } } });
    write_json(&mac.mcp("mcp.json"), &original);

    let migration = mac.migrate_installed();

    assert_eq!(migration.repointed, 0);
    assert_eq!(migration.copy, LegacyCopy::Removed);
    assert_eq!(read_json(&mac.mcp("mcp.json")), original);
}

#[test]
fn an_unreadable_mcp_file_keeps_the_old_copy() {
    let mac = Mac::new();
    write(&mac.mcp("mcp.json"), "{ not json");

    assert_eq!(mac.migrate_installed().copy, LegacyCopy::InUse);
    assert!(mac.legacy().exists());
}

#[test]
fn only_the_old_copy_is_deleted() {
    let mac = Mac::new();
    let bin = mac.legacy().parent().unwrap().to_path_buf();
    write(&bin.join("unrelated"), "someone's file");
    write(&mac.server(), "installed server");

    assert_eq!(mac.migrate_installed().copy, LegacyCopy::Removed);

    assert!(!mac.legacy().exists());
    assert!(bin.join("unrelated").exists());
    assert!(mac.server().exists());
}

#[cfg(unix)]
#[test]
fn a_symlinked_old_copy_is_left_alone() {
    let mac = Mac::new();
    let real = mac.dir.path().join("elsewhere/harness-memory");
    write(&real, "someone's server");
    std::fs::remove_file(mac.legacy()).unwrap();
    std::os::unix::fs::symlink(&real, mac.legacy()).unwrap();

    assert_eq!(mac.migrate_installed().copy, LegacyCopy::Refused);

    assert!(mac.legacy().symlink_metadata().unwrap().file_type().is_symlink());
    assert_eq!(std::fs::read_to_string(&real).unwrap(), "someone's server");
}

#[test]
fn linux_has_no_old_copy_to_retire() {
    let mac = Mac::new();
    let linux = Env::for_tests(mac.dir.path()).with_os(OsKind::Linux);
    // On Linux the old copy was ~/.local/bin/harness-memory, where the installer puts the server.
    write(&mac.server(), "installed server");
    let original =
        json!({ "mcpServers": { "harness-memory": { "command": mac.server().to_string_lossy(), "args": ["mcp"] } } });
    write_json(&mac.mcp("mcp.json"), &original);

    let migration = migrate_legacy_memory(&linux, &mac.targets(), Some(&mac.server()));

    assert_eq!(
        migration,
        LegacyMigration {
            repointed: 0,
            copy: LegacyCopy::Absent
        }
    );
    assert!(mac.server().exists());
    assert_eq!(read_json(&mac.mcp("mcp.json")), original);
}

/// Replaces the old copy's `bin` folder with a symlink to `~/.local/bin`, where the installed
/// server is, so the copy's path leads to the server.
#[cfg(unix)]
fn link_bin_to_the_installers_folder(mac: &Mac) {
    let bin = mac.legacy().parent().unwrap().to_path_buf();
    std::fs::remove_dir_all(&bin).unwrap();
    write(&mac.server(), "installed server");
    std::os::unix::fs::symlink(mac.server().parent().unwrap(), &bin).unwrap();
}

#[cfg(unix)]
#[test]
fn a_copy_path_that_leads_to_the_installed_server_never_deletes_it() {
    let mac = Mac::new();
    link_bin_to_the_installers_folder(&mac);
    let original = json!({ "mcpServers": { "harness-memory": { "command": mac.legacy_command(), "args": ["mcp"] } } });
    write_json(&mac.mcp("mcp.json"), &original);

    let migration = mac.migrate_installed();

    assert_eq!(
        migration,
        LegacyMigration {
            repointed: 0,
            copy: LegacyCopy::InUse
        }
    );
    assert_eq!(std::fs::read_to_string(mac.server()).unwrap(), "installed server");
    assert_eq!(read_json(&mac.mcp("mcp.json")), original);
}

#[cfg(unix)]
#[test]
fn a_copy_in_a_symlinked_folder_is_left_alone() {
    let mac = Mac::new();
    // The copy's folder leads elsewhere, and the server resolves under another path.
    let bin = mac.legacy().parent().unwrap().to_path_buf();
    let elsewhere = mac.dir.path().join("elsewhere");
    write(&elsewhere.join("harness-memory"), "someone's file");
    std::fs::remove_dir_all(&bin).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &bin).unwrap();
    write(&mac.server(), "installed server");

    assert_eq!(mac.migrate_installed().copy, LegacyCopy::Refused);
    assert_eq!(
        std::fs::read_to_string(elsewhere.join("harness-memory")).unwrap(),
        "someone's file"
    );
}

#[cfg(unix)]
#[test]
fn a_copy_under_a_symlinked_data_folder_is_left_alone() {
    let mac = Mac::new();
    let data = mac.legacy().parent().unwrap().parent().unwrap().to_path_buf();
    let moved = mac.dir.path().join("moved-data");
    std::fs::rename(&data, &moved).unwrap();
    std::os::unix::fs::symlink(&moved, &data).unwrap();

    assert_eq!(mac.migrate_installed().copy, LegacyCopy::Refused);
    assert!(moved.join("bin/harness-memory").exists());
}
