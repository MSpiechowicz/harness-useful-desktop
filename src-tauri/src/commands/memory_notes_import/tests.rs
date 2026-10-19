use harness_business::model::{IpcErrorCode, NotesImportFileProblem, NotesImportPlan};
use serde_json::json;
use tauri::async_runtime::block_on;

use super::*;
use crate::test_support::{ProjectHome, collecting_channel, path_arg, place_memory_server};

/// A new repository note, a repository note of the fixtures' title (note 3), a global note of
/// the fixtures' title (note 7) and a row without a title.
const NOTES: &str = "title,body,scope,kind\n\
                     Imported note,From a file.,,decision\n\
                     Watcher misses renames,Inotify drops them.,repo,problem\n\
                     Prefer bun over npm,Faster installs.,global,decision\n\
                     ,No title.,,\n";

/// Reviews `text` as `notes.csv` for `project`, as the preview command does once a file is picked.
fn review(state: &AppState, project: Option<String>, text: &str) -> NotesImportPlan {
    let root = block_on(project_root(state, project)).expect("a known project");
    let preview = block_on(preview(state, root, "notes.csv".to_owned(), text.as_bytes().to_vec())).expect("reviewed");

    match preview {
        NotesImportPreview::Plan { plan } => plan,
        refused @ NotesImportPreview::Refused { .. } => panic!("expected a plan, got {refused:?}"),
    }
}

fn request(rows: &[u32], default_scope: ScriptScope, replace: bool) -> NotesImportRequest {
    NotesImportRequest {
        rows: rows.to_vec(),
        default_scope,
        replace,
    }
}

fn import(
    state: &AppState,
    plan_id: &str,
    request: &NotesImportRequest,
    project: Option<String>,
) -> CommandResult<NotesImportResult> {
    let (channel, _) = collecting_channel();

    block_on(apply(state, plan_id, request, project, channel))
}

#[test]
fn the_installed_server_imports_skips_and_replaces_rows() {
    let setup = ProjectHome::new("notes-import");
    let (state, project) = (&setup.state, &setup.project);

    if place_memory_server(&setup.home).is_none() {
        return;
    }

    let plan = review(state, path_arg(project), NOTES);
    assert_eq!(plan.rows.len(), 4);
    assert!(!plan.rows[3].problems.is_empty(), "the row without a title");

    let error =
        import(state, &plan.plan_id, &request(&[0], ScriptScope::Repo, false), None).expect_err("another project");
    assert_eq!(
        error.code(),
        IpcErrorCode::InvalidInput,
        "and the review stays for its project"
    );

    let (channel, progress) = collecting_channel();
    let skipped = block_on(apply(
        state,
        &plan.plan_id,
        &request(&[2, 0, 1], ScriptScope::Repo, false),
        path_arg(project),
        channel,
    ))
    .expect("imported");

    assert_eq!(
        serde_json::to_value(&skipped).expect("json"),
        json!({
            "created": 1,
            "updated": 0,
            "replaced": 0,
            "exists": [{ "index": 1, "id": 3 }, { "index": 2, "id": 7 }],
            "failed": [],
        })
    );
    assert_eq!(
        *progress.lock().expect("progress"),
        [
            json!({ "index": 0, "done": 1, "total": 3 }),
            json!({ "index": 1, "done": 2, "total": 3 }),
            json!({ "index": 2, "done": 3, "total": 3 }),
        ],
        "one message per row, in the file's order"
    );

    let plan = review(state, path_arg(project), NOTES);
    let replaced = import(
        state,
        &plan.plan_id,
        &request(&[1, 2], ScriptScope::Repo, true),
        path_arg(project),
    )
    .expect("imported");
    assert_eq!(
        (replaced.replaced, replaced.exists.len()),
        (2, 0),
        "notes 3 and 7 are rewritten with the rows"
    );

    block_on(state.memory_scripts.shutdown());
}

#[test]
fn a_row_the_app_refuses_fails_alone_and_a_missing_server_ends_the_import() {
    let setup = ProjectHome::new("notes-import-failed");
    let state = &setup.state;

    let plan = review(state, None, NOTES);
    let error =
        import(state, &plan.plan_id, &request(&[2], ScriptScope::Global, false), None).expect_err("not installed");
    assert_eq!(error.code(), IpcErrorCode::NotFound);

    if place_memory_server(&setup.home).is_none() {
        return;
    }

    let plan = review(state, None, NOTES);
    let result = import(state, &plan.plan_id, &request(&[0, 2], ScriptScope::Repo, false), None).expect("imported");

    assert_eq!(result.updated, 0);
    assert_eq!(
        serde_json::to_value(&result.exists).expect("json"),
        json!([{ "index": 2, "id": 7 }]),
        "the global row, whose title note 7 has"
    );
    assert_eq!(result.failed.len(), 1);
    assert_eq!(result.failed[0].index, 0, "a repository row without a project");
    assert!(
        result.failed[0].message.contains("needs a project"),
        "{:?}",
        result.failed[0]
    );

    block_on(state.memory_scripts.shutdown());
}

#[test]
fn a_review_is_used_once_and_only_for_its_project() {
    let setup = ProjectHome::new("notes-import-plan");
    let (state, project) = (&setup.state, &setup.project);
    let rows = request(&[3], ScriptScope::Repo, false);

    let plan = review(state, path_arg(project), NOTES);

    let error = import(state, &plan.plan_id, &rows, None).expect_err("another project");
    assert_eq!(error.code(), IpcErrorCode::InvalidInput);

    let error = import(state, "another-plan", &rows, path_arg(project)).expect_err("another plan");
    assert_eq!(error.code(), IpcErrorCode::InvalidInput);

    let error = import(state, &plan.plan_id, &rows, path_arg(project)).expect_err("a row with problems");
    assert_eq!(
        error.code(),
        IpcErrorCode::InvalidInput,
        "the review was still there, and is now used up"
    );

    let error = import(
        state,
        &plan.plan_id,
        &request(&[0], ScriptScope::Repo, false),
        path_arg(project),
    )
    .expect_err("used up");
    assert_eq!(error.code(), IpcErrorCode::InvalidInput);

    let first = review(state, path_arg(project), NOTES);
    let second = review(state, path_arg(project), NOTES);
    assert_ne!(first.plan_id, second.plan_id);

    let error = import(state, &first.plan_id, &rows, path_arg(project)).expect_err("replaced by a newer review");
    assert_eq!(error.code(), IpcErrorCode::InvalidInput);

    let error = import(state, &second.plan_id, &rows, path_arg(&setup.stranger)).expect_err("unknown project");
    assert_eq!(error.code(), IpcErrorCode::Forbidden);
}

#[test]
fn an_import_is_refused_while_another_runs_and_the_review_stays() {
    let setup = ProjectHome::new("notes-import-busy");
    let (state, project) = (&setup.state, &setup.project);
    let plan = review(state, path_arg(project), NOTES);

    let running = block_on(state.notes_import_apply.lock());
    let error = import(
        state,
        &plan.plan_id,
        &request(&[0], ScriptScope::Repo, false),
        path_arg(project),
    )
    .expect_err("another import runs");
    drop(running);

    assert_eq!(error.code(), IpcErrorCode::InvalidInput);
    assert!(
        state.notes_import.lock().expect("plan").is_some(),
        "the review is not used up"
    );
}

#[test]
fn a_file_that_is_not_notes_is_refused_with_a_reason_and_drops_the_review() {
    let setup = ProjectHome::new("notes-import-refused");
    let state = &setup.state;
    let file = setup.home.join("large.csv");
    std::fs::write(&file, vec![b'a'; MAX_IMPORT_BYTES + 10]).expect("write");

    let bytes = read_capped(&file).expect("read");
    assert_eq!(bytes.len(), MAX_IMPORT_BYTES + 1, "read one byte past the limit");

    let plan = review(state, None, NOTES);
    let refused = block_on(preview(state, None, "large.csv".to_owned(), bytes)).expect("answered");
    assert_eq!(
        refused,
        NotesImportPreview::Refused {
            file_name: "large.csv".to_owned(),
            reason: NotesImportFileProblem::TooLarge,
            line: None,
        }
    );

    let error = import(state, &plan.plan_id, &request(&[0], ScriptScope::Global, false), None)
        .expect_err("the earlier review is gone");
    assert_eq!(error.code(), IpcErrorCode::InvalidInput);

    let columns = block_on(preview(
        state,
        None,
        "notes.csv".to_owned(),
        b"name,text\nA,b\n".to_vec(),
    ))
    .expect("answered");
    assert!(
        matches!(
            columns,
            NotesImportPreview::Refused {
                reason: NotesImportFileProblem::MissingColumns,
                ..
            }
        ),
        "{columns:?}"
    );
}
