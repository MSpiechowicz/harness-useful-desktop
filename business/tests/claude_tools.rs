//! Which files a Claude Code tool call counts as changed.

use harness_business::claude_stream::tools::{is_file_change, touched_paths};
use serde_json::json;

#[test]
fn file_tools_name_their_file_relative_to_the_project() {
    let project = tempfile::tempdir().unwrap();
    std::fs::create_dir(project.path().join("src")).unwrap();
    let root = project.path().to_str().unwrap();

    let write = json!({ "file_path": format!("{root}/src/new.rs"), "content": "x" });
    let notebook = json!({ "notebook_path": "src/demo.ipynb", "new_source": "x" });

    assert_eq!(touched_paths(project.path(), "Write", &write), ["src/new.rs"]);
    assert_eq!(touched_paths(project.path(), "Edit", &write), ["src/new.rs"]);
    assert_eq!(touched_paths(project.path(), "NotebookEdit", &notebook), ["src/demo.ipynb"]);
}

#[test]
fn other_tools_and_outside_paths_touch_nothing() {
    let project = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let elsewhere = json!({ "file_path": outside.path().join("x.rs").to_str().unwrap() });

    assert!(touched_paths(project.path(), "Write", &elsewhere).is_empty());
    assert!(touched_paths(project.path(), "Read", &json!({ "file_path": "a.rs" })).is_empty());
    assert!(touched_paths(project.path(), "Bash", &json!({ "command": "touch a.rs" })).is_empty());
    assert!(touched_paths(project.path(), "write", &json!({ "file_path": "a.rs" })).is_empty());
    assert!(is_file_change("MultiEdit") && !is_file_change("Task"));
}
