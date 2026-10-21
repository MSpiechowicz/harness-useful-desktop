//! `scripts = "remote+local"` in Harness Memory's `config.toml`: read the team database and this
//! computer, save new scripts here. It is saved only while the installed server lists the three
//! script features in `harness-memory features` (an older server would refuse the file), a gate
//! apart from the one for notes. `trust_team_scripts` still applies, since team scripts are read.

mod memory_remote_support;

use harness_business::BusinessError;
use harness_business::ecosystem::{
    FeaturesSupport, MemoryRemoteChange, ScriptsSupport, memory_remote_get, memory_remote_set,
};
use harness_business::model::MemoryNotesRouting;
use memory_remote_support::{EVERY_FEATURE, Home, postgres_change, with_features};
use pretty_assertions::assert_eq;

/// The note features only.
const NOTES_ONLY: FeaturesSupport = FeaturesSupport::Listed {
    notes_remote_local: true,
    notes_hide: true,
    push_json: true,
    scripts_remote_local: false,
    scripts_hide: false,
    scripts_push_json: false,
    save_guards: false,
};

/// The script features only.
const SCRIPTS_ONLY: FeaturesSupport = FeaturesSupport::Listed {
    notes_remote_local: false,
    notes_hide: false,
    push_json: false,
    scripts_remote_local: true,
    scripts_hide: true,
    scripts_push_json: true,
    save_guards: false,
};

fn scripts_routed(scripts: MemoryNotesRouting, trust_team_scripts: bool) -> MemoryRemoteChange {
    MemoryRemoteChange {
        scripts,
        trust_team_scripts,
        ..postgres_change()
    }
}

fn text(home: &Home) -> String {
    std::fs::read_to_string(home.config()).unwrap()
}

#[test]
fn team_scripts_with_local_saves_are_written_and_read_back_with_their_trust() {
    let home = Home::new();
    let support = with_features(ScriptsSupport::Approvals, EVERY_FEATURE);

    let change = scripts_routed(MemoryNotesRouting::RemoteLocal, true);
    let saved = memory_remote_set(&home.env, change, support).expect("saved");
    assert_eq!(saved.scripts, MemoryNotesRouting::RemoteLocal);
    assert!(
        saved.trust_team_scripts,
        "team scripts are still read, so they may be trusted"
    );
    assert!(saved.scripts_team_and_local_supported);
    assert_eq!(saved.problem, None);

    let text = text(&home);
    assert!(text.contains("scripts = \"remote+local\"\n"), "{text}");
    assert!(text.contains("trust_team_scripts = true\n"), "{text}");

    let wire = serde_json::to_value(memory_remote_get(&home.env, support)).unwrap();
    assert_eq!(wire["scripts"], "remoteLocal");
    assert_eq!(wire["scriptsTeamAndLocalSupported"], true);

    // Back to the team database alone: unset means remote, so the key goes away.
    let change = scripts_routed(MemoryNotesRouting::Remote, false);
    memory_remote_set(&home.env, change, support).expect("saved");
    assert!(!text_has_scripts_key(&home));
    assert_eq!(
        memory_remote_get(&home.env, support).scripts,
        MemoryNotesRouting::Remote
    );
}

fn text_has_scripts_key(home: &Home) -> bool {
    text(home).lines().any(|line| line.starts_with("scripts ="))
}

#[test]
fn a_hand_written_remote_local_scripts_key_is_read_without_a_problem() {
    let home = Home::new();
    memory_remote_set(&home.env, postgres_change(), ScriptsSupport::Approvals).expect("saved");
    let written = text(&home) + "scripts = \"remote+local\"\n";
    std::fs::write(home.config(), written).unwrap();

    // Read whatever the server lists: the saved value is shown either way.
    let remote = memory_remote_get(&home.env, ScriptsSupport::Approvals);
    assert_eq!(remote.scripts, MemoryNotesRouting::RemoteLocal);
    assert!(!remote.scripts_team_and_local_supported, "features unknown");
    assert_eq!(remote.problem, None);
}

#[test]
fn a_server_without_the_script_features_never_gets_remote_local_scripts_written() {
    let home = Home::new();
    let before_support = with_features(ScriptsSupport::Approvals, EVERY_FEATURE);
    memory_remote_set(&home.env, postgres_change(), before_support).expect("saved");
    let before = text(&home);

    let partial = FeaturesSupport::Listed {
        notes_remote_local: true,
        notes_hide: true,
        push_json: true,
        scripts_remote_local: true,
        scripts_hide: false,
        scripts_push_json: true,
        save_guards: false,
    };

    for (features, wording) in [
        (partial, "Update Harness Memory"),
        (NOTES_ONLY, "Update Harness Memory"),
        (FeaturesSupport::Unsupported, "Update Harness Memory"),
        (FeaturesSupport::NotInstalled, "Install Harness Memory"),
        (FeaturesSupport::Unknown, "could not say"),
    ] {
        let support = with_features(ScriptsSupport::Approvals, features);
        let change = scripts_routed(MemoryNotesRouting::RemoteLocal, false);
        let refused = memory_remote_set(&home.env, change, support);

        assert!(
            matches!(&refused, Err(BusinessError::Unsupported(reason)) if reason.contains(wording) && reason.contains("scripts")),
            "{features:?}: {refused:?}"
        );
        assert_eq!(text(&home), before, "nothing is written");
        assert!(!memory_remote_get(&home.env, support).scripts_team_and_local_supported);

        for two_valued in [MemoryNotesRouting::Local, MemoryNotesRouting::Remote] {
            memory_remote_set(&home.env, scripts_routed(two_valued, false), support)
                .expect("remote and local stay possible");
        }
    }
}

#[test]
fn the_script_gate_is_apart_from_the_note_gate() {
    let home = Home::new();

    // Notes may go to both while scripts may not.
    let notes_only = with_features(ScriptsSupport::Approvals, NOTES_ONLY);
    let notes = MemoryRemoteChange {
        repository_notes: MemoryNotesRouting::RemoteLocal,
        ..postgres_change()
    };
    let saved = memory_remote_set(&home.env, notes.clone(), notes_only).expect("notes are saved");
    assert_eq!(saved.repository_notes, MemoryNotesRouting::RemoteLocal);
    assert!(saved.team_and_local_supported);
    assert!(!saved.scripts_team_and_local_supported);

    let both = MemoryRemoteChange {
        scripts: MemoryNotesRouting::RemoteLocal,
        ..notes.clone()
    };
    assert!(matches!(
        memory_remote_set(&home.env, both.clone(), notes_only),
        Err(BusinessError::Unsupported(_))
    ));

    // Scripts may go to both while notes may not.
    let scripts_only = with_features(ScriptsSupport::Approvals, SCRIPTS_ONLY);
    let saved = memory_remote_set(
        &home.env,
        scripts_routed(MemoryNotesRouting::RemoteLocal, false),
        scripts_only,
    )
    .expect("scripts are saved");
    assert_eq!(saved.scripts, MemoryNotesRouting::RemoteLocal);
    assert!(saved.scripts_team_and_local_supported);
    assert!(!saved.team_and_local_supported);

    assert!(matches!(
        memory_remote_set(&home.env, both, scripts_only),
        Err(BusinessError::Unsupported(reason)) if reason.contains("notes")
    ));
    assert_eq!(
        memory_remote_get(&home.env, scripts_only).scripts,
        MemoryNotesRouting::RemoteLocal,
        "a refused save leaves the file as it was"
    );
}
