use harness_business::model::{
    IpcErrorCode, MemoryScriptsAvailability, ScriptApproval, ScriptLanguage, ScriptSaveOutcome,
};
use tauri::async_runtime::block_on;

use super::*;
use crate::test_support::{ProjectHome, path_arg, place_memory_server};

fn draft(name: &str, scope: ScriptScope) -> ScriptDraft {
    ScriptDraft {
        name: name.to_owned(),
        description: "Written in the app".to_owned(),
        language: ScriptLanguage::Bash,
        body: "echo app\n".to_owned(),
        timeout_secs: 30,
        scope,
    }
}

fn by_name() -> ScriptsQuery {
    let window = ListWindow::new(None, None, None, None).expect("the first page");

    ScriptsQuery::new(window, None, None, None, Some(ScriptSort::Name)).expect("no filters")
}

#[test]
fn a_project_that_is_not_open_or_recent_is_refused() {
    let setup = ProjectHome::new("scripts-stranger");

    let error = block_on(list(&setup.state, path_arg(&setup.stranger), None, &by_name())).expect_err("unknown project");
    assert_eq!(error.code(), IpcErrorCode::Forbidden);

    let error = block_on(delete_all(&setup.state, path_arg(&setup.stranger), None, None)).expect_err("unknown project");
    assert_eq!(error.code(), IpcErrorCode::Forbidden);

    let error = block_on(hide(&setup.state, 30, path_arg(&setup.stranger), None, true)).expect_err("unknown project");
    assert_eq!(error.code(), IpcErrorCode::Forbidden);

    let error = block_on(save(
        &setup.state,
        path_arg(&setup.stranger),
        &draft("open-prs", ScriptScope::Repo),
        false,
        None,
        None,
    ))
    .expect_err("unknown project");
    assert_eq!(error.code(), IpcErrorCode::Forbidden);
}

#[test]
fn an_author_name_the_servers_refuse_is_invalid_input_not_an_older_server() {
    let setup = ProjectHome::new("scripts-author");

    for name in ["", " Ada", "Ada\nLovelace"] {
        let query = ScriptsQuery {
            author: Some(AuthorFilter::Name { name: name.to_owned() }),
            ..by_name()
        };

        let error = block_on(list(&setup.state, None, None, &query)).expect_err("refused");
        assert_eq!(error.code(), IpcErrorCode::InvalidInput, "{name:?}");
    }

    let query = ScriptsQuery {
        author: Some(AuthorFilter::Unknown),
        ..by_name()
    };
    let page = block_on(list(&setup.state, None, None, &query)).expect("a valid filter is asked");
    assert_eq!(page.availability, MemoryScriptsAvailability::NotInstalled);
}

#[test]
fn trusting_checks_the_project_and_needs_the_server() {
    let setup = ProjectHome::new("scripts-trust");
    let state = &setup.state;

    let item = || {
        vec![ScriptTrustItem {
            id: 12,
            sha256: "ab".repeat(32),
        }]
    };

    let error = block_on(trust(state, item(), path_arg(&setup.stranger), None)).expect_err("unknown project");
    assert_eq!(error.code(), IpcErrorCode::Forbidden);

    let error = block_on(trust(state, item(), path_arg(&setup.project), None)).expect_err("not installed");
    assert_eq!(error.code(), IpcErrorCode::NotFound);

    let error = block_on(trust(state, Vec::new(), None, None)).expect_err("no ids");
    assert_eq!(error.code(), IpcErrorCode::InvalidInput);
}

#[test]
fn the_installed_server_lists_the_project_scripts_and_deletes_them_all() {
    let setup = ProjectHome::new("scripts-list");
    let state = &setup.state;

    let page = block_on(list(state, None, None, &by_name())).expect("listed");
    assert_eq!(page.availability, MemoryScriptsAvailability::NotInstalled);

    if place_memory_server(&setup.home).is_none() {
        return;
    }

    let page = block_on(list(state, path_arg(&setup.project), None, &by_name())).expect("listed");
    assert_eq!(page.availability, MemoryScriptsAvailability::Available);

    let names: Vec<&str> = page.scripts.iter().map(|script| script.name.as_str()).collect();
    assert_eq!(names, ["open-prs", "release-notes"]);

    let deleted = block_on(delete_all(state, path_arg(&setup.project), None, None)).expect("deleted them all");
    assert_eq!(
        deleted,
        MemoryDeleteAll {
            deleted: 3,
            team: Some(0),
            kept: None,
        }
    );

    block_on(state.memory_scripts.shutdown());
}

#[test]
fn the_installed_server_saves_a_script_and_replaces_one_only_when_asked() {
    let setup = ProjectHome::new("scripts-save");
    let (state, project) = (&setup.state, &setup.project);

    let error = block_on(save(
        state,
        path_arg(project),
        &draft("app-script", ScriptScope::Repo),
        false,
        None,
        None,
    ))
    .expect_err("not installed");
    assert_eq!(error.code(), IpcErrorCode::NotFound);

    if place_memory_server(&setup.home).is_none() {
        return;
    }

    let created = block_on(save(
        state,
        path_arg(project),
        &draft("app-script", ScriptScope::Repo),
        false,
        None,
        None,
    ))
    .expect("created");
    assert_eq!(created.outcome, ScriptSaveOutcome::Created);
    let script = created.script.expect("the saved script");
    assert_eq!((script.id, script.approval), (created.id, ScriptApproval::Local));

    let exists = block_on(save(
        state,
        path_arg(project),
        &draft("open-prs", ScriptScope::Repo),
        false,
        None,
        None,
    ))
    .expect("refused");
    assert_eq!(
        (exists.outcome, exists.id, exists.script),
        (ScriptSaveOutcome::Exists, 12, None)
    );

    let global = block_on(save(
        state,
        path_arg(project),
        &draft("open-prs", ScriptScope::Global),
        false,
        None,
        None,
    ))
    .expect("another scope");
    assert_eq!(
        global.outcome,
        ScriptSaveOutcome::Created,
        "the name is taken in the repository only"
    );

    let replaced = block_on(save(
        state,
        path_arg(project),
        &draft("open-prs", ScriptScope::Repo),
        true,
        None,
        None,
    ))
    .expect("replaced");
    assert_eq!((replaced.outcome, replaced.id), (ScriptSaveOutcome::Replaced, 12));
    assert_eq!(replaced.script.expect("the saved script").timeout_secs, 30);

    let waiting = block_on(save(
        state,
        path_arg(project),
        &draft("release-notes", ScriptScope::Global),
        true,
        None,
        None,
    ))
    .expect("replaced");
    assert_eq!(
        (waiting.id, waiting.script.expect("the saved script").approval),
        (7, ScriptApproval::Local),
        "other code over a script waiting for approval is the user's, and the server trusts it as saved here"
    );

    let error = block_on(save(
        state,
        None,
        &draft("app-script", ScriptScope::Repo),
        false,
        None,
        None,
    ))
    .expect_err("a repository script needs a project");
    assert_eq!(error.code(), IpcErrorCode::InvalidInput);

    block_on(state.memory_scripts.shutdown());
}
