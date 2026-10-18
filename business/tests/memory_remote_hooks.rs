//! The team hooks keys of Harness Memory's `config.toml`: `hooks = "remote"` (share hooks with the
//! team) and `trust_team_hooks = true`. Written only while the installed server keeps team hooks
//! (an older server refuses a file with keys it does not know), trust only while hooks are shared,
//! a null choice keeps the saved one, and both are left as saved while the server cannot be asked.
//! `HARNESS_MEMORY_CONFIG` names the file, as for the server.

mod memory_remote_support;

use harness_business::ecosystem::{
    HooksConfig, HooksSupport, MemoryRemoteChange, MemorySupport, ScriptsSupport, memory_config_path,
    memory_remote_get, memory_remote_set,
};
use harness_business::model::{MemoryNotesTarget, MemoryRemoteKind};
use memory_remote_support::{Home, URL};
use pretty_assertions::assert_eq;

const SUPPORTED: HooksSupport = HooksSupport::Supported(HooksConfig {
    remote: false,
    trust: false,
});

impl Home {
    /// The file's text after saving the team database with these team hooks choices.
    fn save(&self, share: Option<bool>, trust: Option<bool>, hooks: HooksSupport) -> String {
        let change = MemoryRemoteChange {
            enabled: true,
            kind: Some(MemoryRemoteKind::Postgres),
            url: Some(URL.to_owned()),
            token: None,
            repository_notes: MemoryNotesTarget::Remote,
            global_notes: MemoryNotesTarget::Remote,
            scripts: MemoryNotesTarget::Remote,
            trust_team_scripts: false,
            share_hooks: share,
            trust_team_hooks: trust,
        };

        memory_remote_set(&self.env, change, support(hooks)).expect("saved");
        std::fs::read_to_string(self.config()).expect("written")
    }
}

fn support(hooks: HooksSupport) -> MemorySupport {
    MemorySupport {
        scripts: ScriptsSupport::Approvals,
        hooks,
    }
}

fn hooks_lines(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| line.starts_with("hooks") || line.starts_with("trust_team_hooks"))
        .collect()
}

#[test]
fn sharing_and_trust_round_trip_while_the_server_keeps_team_hooks() {
    let home = Home::new();

    let text = home.save(Some(true), Some(true), SUPPORTED);
    assert_eq!(hooks_lines(&text), ["hooks = \"remote\"", "trust_team_hooks = true"]);

    let remote = memory_remote_get(&home.env, support(SUPPORTED));
    assert!(remote.hooks_supported && remote.share_hooks && remote.trust_team_hooks);

    // The file the app wrote is read back without a problem, and saved again unchanged.
    assert_eq!(remote.problem, None);
    assert_eq!(home.save(None, None, SUPPORTED), text);
}

#[test]
fn trust_is_written_only_while_hooks_are_shared() {
    let home = Home::new();

    let text = home.save(Some(false), Some(true), SUPPORTED);
    assert!(hooks_lines(&text).is_empty(), "{text}");

    let remote = memory_remote_get(&home.env, support(SUPPORTED));
    assert!(!remote.share_hooks && !remote.trust_team_hooks);

    home.save(Some(true), Some(true), SUPPORTED);
    let text = home.save(None, Some(false), SUPPORTED);
    assert_eq!(hooks_lines(&text), ["hooks = \"remote\""]);
}

#[test]
fn an_older_server_gets_neither_key() {
    let home = Home::new();
    home.save(Some(true), Some(true), SUPPORTED);

    let text = home.save(Some(true), Some(true), HooksSupport::Unsupported);
    assert!(hooks_lines(&text).is_empty(), "{text}");

    let remote = memory_remote_get(&home.env, support(HooksSupport::Unsupported));
    assert!(!remote.hooks_supported && !remote.share_hooks);
}

#[test]
fn the_saved_keys_stay_while_the_server_cannot_be_asked() {
    for unknown in [HooksSupport::Unknown, HooksSupport::NotInstalled] {
        let home = Home::new();
        home.save(Some(true), Some(true), SUPPORTED);

        let text = home.save(Some(false), Some(false), unknown);
        assert_eq!(hooks_lines(&text), ["hooks = \"remote\"", "trust_team_hooks = true"]);

        let remote = memory_remote_get(&home.env, ScriptsSupport::Unknown);
        assert!(!remote.hooks_supported && remote.share_hooks && remote.trust_team_hooks);
    }
}

/// Writes `text` as the config file, private as the server wants it.
fn write_private(home: &Home, text: &str) {
    std::fs::create_dir_all(home.config().parent().unwrap()).unwrap();
    std::fs::write(home.config(), text).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(home.config(), std::fs::Permissions::from_mode(0o600)).unwrap();
    }
}

#[test]
fn a_hand_written_file_with_the_hooks_keys_is_not_blocked() {
    let home = Home::new();
    write_private(
        &home,
        &format!("database_url = \"{URL}\"\nhooks = \"local\"\ntrust_team_hooks = false\n"),
    );

    let remote = memory_remote_get(&home.env, support(SUPPORTED));
    assert_eq!(remote.problem, None);
    assert!(remote.enabled && !remote.share_hooks && !remote.trust_team_hooks);

    write_private(&home, &format!("database_url = \"{URL}\"\nhooks = \"team\"\n"));
    let wrong = memory_remote_get(&home.env, support(SUPPORTED));
    assert_eq!(wrong.problem.as_deref(), Some("hooks must be \"remote\" or \"local\""));
}

#[test]
fn harness_memory_config_names_the_file_read_and_written() {
    let mut home = Home::new();
    let custom = home.env.home.join("team").join("memory.toml");
    home.env = home.env.clone().with_var("HARNESS_MEMORY_CONFIG", "~/team/memory.toml");

    assert_eq!(memory_config_path(&home.env), Some(custom.clone()));

    home.save(Some(true), None, SUPPORTED);
    let text = std::fs::read_to_string(&custom).expect("written where the server reads");
    assert_eq!(hooks_lines(&text), ["hooks = \"remote\""]);
    assert!(
        !home.env.home.join(".config/harness-memory/config.toml").exists(),
        "the default file is left alone"
    );

    let remote = memory_remote_get(&home.env, support(SUPPORTED));
    assert!(remote.enabled && remote.share_hooks);
    assert_eq!(remote.config_path, "~/team/memory.toml");

    // A relative value is refused by the server, so the app neither reads nor writes a file.
    let relative = home.env.clone().with_var("HARNESS_MEMORY_CONFIG", "memory.toml");
    assert_eq!(memory_config_path(&relative), None);

    let remote = memory_remote_get(&relative, support(SUPPORTED));
    assert!(!remote.enabled);
    assert!(remote.problem.unwrap().contains("HARNESS_MEMORY_CONFIG"));
}
