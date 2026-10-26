//! The team document keys of Harness Memory's `config.toml`, for hooks (`hooks = "remote"`,
//! `trust_team_hooks = true`) and routines (`routines = "remote"`, `trust_team_routines = true`).
//! Each kind's keys are written only while the installed server keeps that kind's team documents
//! (an older server refuses a file with keys it does not know), trust only while the kind is
//! shared, a null choice keeps the saved one, and both are left as saved while the server cannot
//! be asked. `HARNESS_MEMORY_CONFIG` names the file, as for the server.

use crate::memory_remote_support;

use harness_business::ecosystem::{
    MemoryRemoteChange, MemorySupport, ScriptsSupport, TeamDocsConfig, TeamDocsSupport, memory_config_path,
    memory_remote_get, memory_remote_set,
};
use harness_business::model::MemoryRemote;
use memory_remote_support::{Home, URL, postgres_change};
use pretty_assertions::assert_eq;

const SUPPORTED: TeamDocsSupport = TeamDocsSupport::Supported(TeamDocsConfig {
    remote: false,
    trust: false,
});

/// One kind of team documents: its config keys and its fields in a change and in the settings.
struct Kind {
    key: &'static str,
    trust_key: &'static str,
    /// Sets the kind's choices on a change.
    choose: fn(&mut MemoryRemoteChange, Option<bool>, Option<bool>),
    /// The support where only this kind is `support` (the other one cannot be asked).
    support: fn(TeamDocsSupport) -> MemorySupport,
    /// `(supported, shared, trusted)` of the settings.
    read: fn(&MemoryRemote) -> (bool, bool, bool),
}

const HOOKS: Kind = Kind {
    key: "hooks",
    trust_key: "trust_team_hooks",
    choose: |change, share, trust| {
        change.share_hooks = share;
        change.trust_team_hooks = trust;
    },
    support: |hooks| MemorySupport {
        hooks,
        ..MemorySupport::from(ScriptsSupport::Approvals)
    },
    read: |remote| (remote.hooks_supported, remote.share_hooks, remote.trust_team_hooks),
};

const ROUTINES: Kind = Kind {
    key: "routines",
    trust_key: "trust_team_routines",
    choose: |change, share, trust| {
        change.share_routines = share;
        change.trust_team_routines = trust;
    },
    support: |routines| MemorySupport {
        routines,
        ..MemorySupport::from(ScriptsSupport::Approvals)
    },
    read: |remote| {
        (
            remote.routines_supported,
            remote.share_routines,
            remote.trust_team_routines,
        )
    },
};

const KINDS: [Kind; 2] = [HOOKS, ROUTINES];

impl Kind {
    /// The file's text after saving the team database with these choices for this kind.
    fn save(&self, home: &Home, share: Option<bool>, trust: Option<bool>, support: TeamDocsSupport) -> String {
        let mut change = postgres_change();
        (self.choose)(&mut change, share, trust);

        memory_remote_set(&home.env, change, (self.support)(support)).expect("saved");
        std::fs::read_to_string(home.config()).expect("written")
    }

    fn get(&self, home: &Home, support: TeamDocsSupport) -> (bool, bool, bool) {
        (self.read)(&memory_remote_get(&home.env, (self.support)(support)))
    }

    /// The lines of the text that hold this kind's keys.
    fn lines<'a>(&self, text: &'a str) -> Vec<&'a str> {
        text.lines()
            .filter(|line| {
                let key = line.split(" = ").next().unwrap_or_default();
                key == self.key || key == self.trust_key
            })
            .collect()
    }

    fn shared_line(&self) -> String {
        format!("{} = \"remote\"", self.key)
    }

    fn trusted_line(&self) -> String {
        format!("{} = true", self.trust_key)
    }
}

#[test]
fn sharing_and_trust_round_trip_while_the_server_keeps_the_kind() {
    for kind in KINDS {
        let home = Home::new();

        let text = kind.save(&home, Some(true), Some(true), SUPPORTED);
        assert_eq!(kind.lines(&text), [kind.shared_line(), kind.trusted_line()]);
        assert_eq!(kind.get(&home, SUPPORTED), (true, true, true));

        // The file the app wrote is read back without a problem, and saved again unchanged.
        let remote = memory_remote_get(&home.env, (kind.support)(SUPPORTED));
        assert_eq!(remote.problem, None);
        assert_eq!(kind.save(&home, None, None, SUPPORTED), text);
    }
}

#[test]
fn trust_is_written_only_while_the_kind_is_shared() {
    for kind in KINDS {
        let home = Home::new();

        let text = kind.save(&home, Some(false), Some(true), SUPPORTED);
        assert!(kind.lines(&text).is_empty(), "{text}");
        assert_eq!(kind.get(&home, SUPPORTED), (true, false, false));

        kind.save(&home, Some(true), Some(true), SUPPORTED);
        let text = kind.save(&home, None, Some(false), SUPPORTED);
        assert_eq!(kind.lines(&text), [kind.shared_line()]);
    }
}

#[test]
fn an_older_server_gets_neither_key() {
    for kind in KINDS {
        let home = Home::new();
        kind.save(&home, Some(true), Some(true), SUPPORTED);

        let text = kind.save(&home, Some(true), Some(true), TeamDocsSupport::Unsupported);
        assert!(kind.lines(&text).is_empty(), "{text}");

        let (supported, shared, _) = kind.get(&home, TeamDocsSupport::Unsupported);
        assert!(!supported && !shared);
    }
}

#[test]
fn the_saved_keys_stay_while_the_server_cannot_be_asked() {
    for kind in KINDS {
        for unknown in [TeamDocsSupport::Unknown, TeamDocsSupport::NotInstalled] {
            let home = Home::new();
            kind.save(&home, Some(true), Some(true), SUPPORTED);

            let text = kind.save(&home, Some(false), Some(false), unknown);
            assert_eq!(kind.lines(&text), [kind.shared_line(), kind.trusted_line()]);

            let remote = memory_remote_get(&home.env, ScriptsSupport::Unknown);
            assert_eq!((kind.read)(&remote), (false, true, true));
        }
    }
}

#[test]
fn each_kind_follows_its_own_support() {
    let home = Home::new();
    let support = MemorySupport {
        hooks: TeamDocsSupport::Unsupported,
        routines: SUPPORTED,
        ..MemorySupport::from(ScriptsSupport::Approvals)
    };

    let change = MemoryRemoteChange {
        share_hooks: Some(true),
        trust_team_hooks: Some(true),
        share_routines: Some(true),
        ..postgres_change()
    };

    let remote = memory_remote_set(&home.env, change, support).expect("saved");
    let text = std::fs::read_to_string(home.config()).expect("written");

    assert!(HOOKS.lines(&text).is_empty(), "{text}");
    assert_eq!(ROUTINES.lines(&text), [ROUTINES.shared_line()]);
    assert_eq!((HOOKS.read)(&remote), (false, false, false));
    assert_eq!((ROUTINES.read)(&remote), (true, true, false));
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
fn a_hand_written_file_with_the_kinds_keys_is_not_blocked() {
    for kind in KINDS {
        let home = Home::new();
        write_private(
            &home,
            &format!(
                "database_url = \"{URL}\"\n{} = \"local\"\n{} = false\n",
                kind.key, kind.trust_key
            ),
        );

        let remote = memory_remote_get(&home.env, (kind.support)(SUPPORTED));
        assert_eq!(remote.problem, None);
        assert!(remote.enabled);
        assert_eq!((kind.read)(&remote), (true, false, false));

        write_private(&home, &format!("database_url = \"{URL}\"\n{} = \"team\"\n", kind.key));
        let wrong = memory_remote_get(&home.env, (kind.support)(SUPPORTED));
        assert_eq!(
            wrong.problem,
            Some(format!("{} must be \"remote\" or \"local\"", kind.key))
        );
    }
}

#[test]
fn harness_memory_config_names_the_file_read_and_written() {
    let mut home = Home::new();
    let custom = home.env.home.join("team").join("memory.toml");
    home.env = home.env.clone().with_var("HARNESS_MEMORY_CONFIG", "~/team/memory.toml");

    assert_eq!(memory_config_path(&home.env), Some(custom.clone()));

    HOOKS.save(&home, Some(true), None, SUPPORTED);
    let text = std::fs::read_to_string(&custom).expect("written where the server reads");
    assert_eq!(HOOKS.lines(&text), [HOOKS.shared_line()]);
    assert!(
        !home.env.home.join(".config/harness-memory/config.toml").exists(),
        "the default file is left alone"
    );

    let remote = memory_remote_get(&home.env, (HOOKS.support)(SUPPORTED));
    assert!(remote.enabled && remote.share_hooks);
    assert_eq!(remote.config_path, "~/team/memory.toml");

    // A relative value is refused by the server, so the app neither reads nor writes a file.
    let relative = home.env.clone().with_var("HARNESS_MEMORY_CONFIG", "memory.toml");
    assert_eq!(memory_config_path(&relative), None);

    let remote = memory_remote_get(&relative, (HOOKS.support)(SUPPORTED));
    assert!(!remote.enabled);
    assert!(remote.problem.unwrap().contains("HARNESS_MEMORY_CONFIG"));
}
