//! Team scripts beside the scripts saved on this computer, through `harness-memory scripts`,
//! against the stand-in of `fake_agent` (see `src/bin/fake_agent/memory_scripts_hidden.rs`):
//! read-only team scripts while scripts are routed `remote+local`, the hidden count and the local
//! counts of a page, the hidden team scripts (`--hidden`), hiding and showing one, discarding local
//! scripts (ids on stdin), deleting them all while team scripts stay, and saving a script beside a
//! read-only team script of its name.

use harness_business::BusinessError;
use harness_business::ecosystem::{ListWindow, ScriptsQuery, discard_local, hide_entry};
use harness_business::model::{
    LocalNoteIds, MemoryCopyKind, MemoryDeleteAll, MemoryScopeCounts, MemoryScriptsPage, NoteIdsWord,
    ScriptSaveOutcome, ScriptScope, ScriptSort,
};
use memory_stand_in::{CURRENT, Setup, Vars, script};
use pretty_assertions::assert_eq;

mod memory_stand_in;

/// Hiding and discarding take scripts.
const SCRIPTS: MemoryCopyKind = MemoryCopyKind::Scripts;

/// Scripts routed `remote+local`, as the server reads its environment.
const TEAM_AND_LOCAL: Vars = &[("HARNESS_MEMORY_SCRIPTS", "remote+local")];

/// Scripts routed to the team database alone.
const REMOTE: Vars = &[("HARNESS_MEMORY_SCRIPTS", "remote")];

fn query(hidden: bool) -> ScriptsQuery {
    let window = ListWindow::new(None, None, None, None).unwrap();

    ScriptsQuery {
        hidden,
        ..ScriptsQuery::new(window, None, None, None, Some(ScriptSort::Name)).unwrap()
    }
}

async fn first_page(setup: &Setup, hidden: bool) -> MemoryScriptsPage {
    let project = setup.project();

    setup
        .scripts
        .list(&setup.env, setup.runner(), Some(&project), &query(hidden))
        .await
}

#[tokio::test]
async fn team_scripts_beside_local_saves_are_read_only_and_the_page_counts_the_rest() {
    let setup = Setup::installed(CURRENT, TEAM_AND_LOCAL);

    let page = first_page(&setup, false).await;
    let listed: Vec<(i64, bool)> = page
        .scripts
        .iter()
        .map(|script| (script.id, script.read_only))
        .collect();
    assert_eq!(
        listed,
        [(31, true), (12, false), (30, true), (7, false)],
        "both scripts named open-prs are listed"
    );
    assert!(page.scripts.iter().all(|script| !script.hidden));
    assert_eq!(page.hidden_count, Some(1));
    assert_eq!(page.local_counts, Some(MemoryScopeCounts { repo: 1, global: 1 }));

    let wire = serde_json::to_value(&page).unwrap();
    assert_eq!(wire["hiddenCount"], 1);
    assert_eq!(wire["scripts"][0]["readOnly"], true);
    setup.scripts.shutdown().await;

    let setup = Setup::installed(CURRENT, REMOTE);
    let page = first_page(&setup, false).await;
    assert!(
        page.scripts.iter().all(|script| !script.read_only),
        "team scripts are editable while scripts are kept in the team database alone"
    );
    setup.scripts.shutdown().await;

    let setup = Setup::installed(CURRENT, &[]);
    let page = first_page(&setup, false).await;
    assert_eq!((page.hidden_count, page.local_counts), (None, None), "no team database");
    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn hidden_team_scripts_are_listed_and_read_only_when_asked_for() {
    let setup = Setup::installed(CURRENT, TEAM_AND_LOCAL);
    let project = setup.project();

    let page = first_page(&setup, true).await;
    let ids: Vec<i64> = page.scripts.iter().map(|script| script.id).collect();
    assert_eq!(ids, [41]);
    assert!(page.scripts[0].hidden);
    assert_eq!(page.total, 1);
    assert_eq!(setup.run_args(0).last().map(String::as_str), Some("--hidden"));

    let hidden = setup
        .scripts
        .get_hidden(&setup.env, setup.runner(), 41, None)
        .await
        .expect("the hidden script");
    assert!(hidden.summary.hidden);
    assert_eq!(hidden.body, "make deploy\n");
    assert_eq!(setup.run_args(1), ["scripts", "get", "41", "--json", "--hidden"]);

    let not_hidden = setup
        .scripts
        .get_hidden(&setup.env, setup.runner(), 31, Some(&project))
        .await;
    assert!(
        matches!(&not_hidden, Err(BusinessError::NotFound(message)) if message == "script 31"),
        "{not_hidden:?}"
    );

    let unseen = setup.scripts.get(&setup.env, 41, None).await;
    assert!(
        matches!(unseen, Err(BusinessError::NotFound(_))),
        "a hidden script is not read as a visible one: {unseen:?}"
    );

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_team_script_is_hidden_and_shown_again_but_a_local_one_is_refused() {
    let setup = Setup::installed(CURRENT, TEAM_AND_LOCAL);
    let project = setup.project();
    let hide =
        |id, project, hidden| hide_entry(&setup.env, &setup.scripts, setup.runner(), SCRIPTS, id, project, hidden);

    hide(30, Some(project.as_path()), true).await.expect("hidden");
    hide(41, None, false).await.expect("shown again");
    assert_eq!(
        setup.run_args(0),
        [
            "scripts",
            "hide",
            "30",
            "--json",
            "--project",
            &project.to_string_lossy()
        ]
    );
    assert_eq!(setup.run_args(1), ["scripts", "unhide", "41", "--json"]);

    let local = hide(12, Some(project.as_path()), true).await;
    assert!(
        matches!(&local, Err(BusinessError::Process(message)) if message.ends_with("delete it instead")),
        "{local:?}"
    );

    let gone = hide(30, None, true).await;
    assert!(
        matches!(&gone, Err(BusinessError::NotFound(message)) if message == "script 30"),
        "a repository script without its project: {gone:?}"
    );
    setup.scripts.shutdown().await;

    let setup = Setup::installed(CURRENT, &[]);
    let alone = hide_entry(&setup.env, &setup.scripts, setup.runner(), SCRIPTS, 31, None, true).await;
    assert!(
        matches!(&alone, Err(BusinessError::Process(message)) if message.contains("no team database")),
        "{alone:?}"
    );
    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn local_scripts_are_discarded_all_or_by_id_on_stdin_while_scripts_are_remote() {
    let setup = Setup::installed(CURRENT, REMOTE);
    let discard = |ids, scope| discard_local(&setup.env, &setup.scripts, setup.runner(), SCRIPTS, ids, scope);

    let all = LocalNoteIds::Word(NoteIdsWord::All);
    let deleted = discard(&all, Some(ScriptScope::Global)).await.expect("discarded");
    assert_eq!(deleted, 1);
    assert_eq!(
        setup.run_args(0),
        [
            "scripts",
            "discard-local",
            "--all",
            "--scope",
            "global",
            "--yes",
            "--json"
        ]
    );
    assert_eq!(setup.runs()[0].stdin, None);

    let ids = LocalNoteIds::Ids(vec![12, 7]);
    let deleted = discard(&ids, None).await.expect("discarded");
    assert_eq!(deleted, 2);
    assert_eq!(setup.run_args(1), ["scripts", "discard-local", "-", "--yes", "--json"]);
    assert_eq!(setup.runs()[1].stdin.as_deref(), Some(b"12,7\n".as_slice()));

    for refused in [
        LocalNoteIds::Word(NoteIdsWord::None),
        LocalNoteIds::Ids(Vec::new()),
        LocalNoteIds::Ids(vec![12, -1]),
    ] {
        let result = discard_local(&setup.env, &setup.scripts, setup.runner(), SCRIPTS, &refused, None).await;
        assert!(
            matches!(result, Err(BusinessError::InvalidInput(_))),
            "{refused:?}: {result:?}"
        );
    }
    assert_eq!(setup.runs().len(), 2, "nothing runs for a refused selection");
    setup.scripts.shutdown().await;

    let setup = Setup::installed(CURRENT, TEAM_AND_LOCAL);
    let read = discard_local(&setup.env, &setup.scripts, setup.runner(), SCRIPTS, &all, None).await;
    assert!(
        matches!(&read, Err(BusinessError::Process(message)) if message.contains("still read")),
        "local scripts are still read beside the team's: {read:?}"
    );
    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn deleting_them_all_keeps_the_read_only_team_scripts() {
    let setup = Setup::installed(CURRENT, TEAM_AND_LOCAL);

    let deleted = setup
        .scripts
        .delete_all(&setup.env, setup.runner(), None)
        .await
        .expect("deleted");
    assert_eq!(
        deleted,
        MemoryDeleteAll {
            deleted: 2,
            team: Some(0),
            kept: Some(1),
        }
    );

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_script_is_saved_beside_a_read_only_team_script_of_its_name() {
    let draft = || harness_business::model::ScriptDraft {
        scope: ScriptScope::Global,
        ..script("deploy")
    };

    let setup = Setup::installed(CURRENT, TEAM_AND_LOCAL);
    let saved = setup
        .scripts
        .save(&setup.env, setup.runner(), None, &draft(), false, None)
        .await
        .expect("saved");
    assert_eq!(
        saved.outcome,
        ScriptSaveOutcome::Created,
        "the team script does not count"
    );
    assert_ne!(saved.id, 31);
    setup.scripts.shutdown().await;

    let setup = Setup::installed(CURRENT, REMOTE);
    let saved = setup
        .scripts
        .save(&setup.env, setup.runner(), None, &draft(), false, None)
        .await
        .expect("refused");
    assert_eq!(
        (saved.outcome, saved.id),
        (ScriptSaveOutcome::Exists, 31),
        "an editable team script does"
    );
    setup.scripts.shutdown().await;
}
