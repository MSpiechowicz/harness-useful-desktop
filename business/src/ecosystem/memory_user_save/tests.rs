use pretty_assertions::assert_eq;

use super::*;

fn strings(args: Vec<OsString>) -> Vec<String> {
    args.into_iter().map(|arg| arg.into_string().unwrap()).collect()
}

#[test]
fn the_save_commands_name_the_app_and_the_project_when_there_is_one() {
    let project = Path::new("/work/repo");

    assert_eq!(
        strings(note_save_args(Some(project), None)),
        [
            "notes",
            "save",
            "--json",
            "--client",
            "harness-desktop",
            "--project",
            "/work/repo"
        ]
    );
    assert_eq!(
        strings(note_save_args(None, None)),
        ["notes", "save", "--json", "--client", "harness-desktop"]
    );
    assert_eq!(
        strings(note_update_args(12, Some(project), None)),
        [
            "notes",
            "update",
            "12",
            "--json",
            "--client",
            "harness-desktop",
            "--project",
            "/work/repo"
        ]
    );
    assert_eq!(
        strings(note_update_args(12, None, None)),
        ["notes", "update", "12", "--json", "--client", "harness-desktop"]
    );
    assert_eq!(
        strings(note_update_args(12, Some(project), Some(3))),
        [
            "notes",
            "update",
            "12",
            "--json",
            "--client",
            "harness-desktop",
            "--project",
            "/work/repo",
            "--expect-revision",
            "3"
        ]
    );
    assert_eq!(
        strings(note_save_args(Some(project), Some(MemorySavePlace::Local))),
        [
            "notes",
            "save",
            "--json",
            "--client",
            "harness-desktop",
            "--project",
            "/work/repo",
            "--place",
            "local"
        ]
    );
    assert_eq!(
        strings(script_save_args(None, Some(MemorySavePlace::Remote))),
        [
            "scripts",
            "save",
            "--json",
            "--client",
            "harness-desktop",
            "--place",
            "remote"
        ]
    );
    assert_eq!(
        strings(script_save_args(Some(project), None)),
        [
            "scripts",
            "save",
            "--json",
            "--client",
            "harness-desktop",
            "--project",
            "/work/repo"
        ]
    );
}

#[test]
fn a_refusal_is_read_from_its_code() {
    let gone = refusal(r#"{"error": "gone", "message": "note 4 is gone"}"#, "line".to_owned());
    assert!(matches!(gone.error, BusinessError::Conflict(ref text) if text == GONE_MESSAGE));

    let changed = refusal(
        r#"{"error": "changed", "message": "held save changed"}"#,
        "line".to_owned(),
    );
    assert!(matches!(changed.error, BusinessError::Changed(ref text) if text == CHANGED_MESSAGE));

    let moved = refusal(
        r#"{"error": "changed", "reason": "project", "message": "the folder this save was held for is gone, so nothing was saved"}"#,
        "line".to_owned(),
    );
    assert_eq!(moved.code.as_deref(), Some("changed"));
    assert!(matches!(moved.error, BusinessError::Process(ref text) if text.contains("is gone, so nothing was saved")));

    let missing = refusal(
        r#"{"error": "notFound", "message": ""}"#,
        "harness-memory: no held save".to_owned(),
    );
    assert!(matches!(missing.error, BusinessError::NotFound(ref text) if text == "harness-memory: no held save"));

    let invalid = refusal(
        r#"{"error": "invalid", "message": "unknown field `x`"}"#,
        "line".to_owned(),
    );
    assert!(matches!(invalid.error, BusinessError::InvalidInput(ref text) if text == "unknown field `x`"));

    let taken = refusal(
        r#"{"error": "refused", "message": "note 3 already has this title"}"#,
        "line".to_owned(),
    );
    assert_eq!(taken.code.as_deref(), Some("refused"));
    assert_eq!(taken_title(&taken.message), Some(3));

    let plain = refusal("not json", "harness-memory: database locked".to_owned());
    assert_eq!(plain.code, None);
    assert!(matches!(plain.error, BusinessError::Process(ref text) if text == "harness-memory: database locked"));
}

#[test]
fn only_a_server_with_save_guards_takes_the_user_commands() {
    assert_eq!(SaveRoute::of(FeaturesSupport::Unknown), SaveRoute::Mcp);
    assert_eq!(SaveRoute::of(FeaturesSupport::Unsupported), SaveRoute::Mcp);
}
