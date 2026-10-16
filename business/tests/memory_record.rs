//! The record of the MCP files the app registered the memory server in: round trip, clearing, and
//! a corrupt record rebuilt from every file checked, so nothing is added that launch or later.

mod mcp_support;

use harness_business::ecosystem::{
    MEMORY_RECORD_FILE, McpFile, MemoryRecord, MemoryRegistration, RecordedFile, Shape, register_memory_mcp,
};
use harness_business::model::HarnessId;
use mcp_support::Home;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

impl Home {
    fn config(&self) -> std::path::PathBuf {
        self.dir.path().join("config")
    }

    fn config_files(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.config())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// One launch: registers in OMP's `mcp.json` as the record says, notes the outcome and saves.
    fn launch(&self) -> MemoryRegistration {
        let path = self.agent("mcp.json");
        let mut record = MemoryRecord::load(&self.config());

        let binary = self.dir.path().join(".local/bin/harness-memory");
        let file = McpFile {
            path: path.clone(),
            also_read: Vec::new(),
            shape: Shape::McpServers,
        };
        let outcome = register_memory_mcp(&file, &binary, record.is_recorded(&self.env, &path));

        record.note(&self.env, HarnessId::Omp, &path, outcome);
        record.save().unwrap();
        outcome
    }
}

fn stored(home: &Home) -> Value {
    serde_json::from_str(&std::fs::read_to_string(home.config().join(MEMORY_RECORD_FILE)).unwrap()).unwrap()
}

#[test]
fn a_registration_is_recorded_by_home_relative_file() {
    let home = Home::new();

    assert_eq!(home.launch(), MemoryRegistration::Added);
    assert_eq!(
        stored(&home),
        json!({ "version": 1, "registered": [{ "harness": "omp", "file": "~/.omp/agent/mcp.json" }] })
    );

    std::fs::remove_file(home.agent("mcp.json")).unwrap();
    assert_eq!(home.launch(), MemoryRegistration::SkippedRemoved);
    assert!(!home.agent("mcp.json").exists());

    MemoryRecord::clear(&home.config()).unwrap();
    MemoryRecord::clear(&home.config()).unwrap();
    assert_eq!(
        home.launch(),
        MemoryRegistration::Added,
        "a cleared record adds it back"
    );
}

#[test]
fn nothing_registered_writes_no_record() {
    let home = Home::new();
    home.write("mcp.json", "[]");

    assert_eq!(
        home.launch(),
        MemoryRegistration::Refused(harness_business::model::McpRefusal::NotAnObject)
    );
    assert!(!home.config().exists());
}

#[test]
fn a_corrupt_record_is_moved_aside_and_rebuilt_without_adding_on_later_launches() {
    let home = Home::new();
    std::fs::create_dir_all(home.config()).unwrap();
    std::fs::write(home.config().join(MEMORY_RECORD_FILE), "{ not json").unwrap();

    let record = MemoryRecord::load(&home.config());
    assert!(record.rebuilding());
    assert!(record.is_recorded(&home.env, &home.agent("mcp.json")));
    drop(record);

    let files = home.config_files();
    assert_eq!(files.len(), 1);
    assert!(
        files[0].starts_with(&format!("{MEMORY_RECORD_FILE}.corrupt-")),
        "{files:?}"
    );

    // The launch that finds it corrupt adds nothing and records the file it checked.
    std::fs::remove_dir_all(home.config()).unwrap();
    std::fs::create_dir_all(home.config()).unwrap();
    std::fs::write(home.config().join(MEMORY_RECORD_FILE), "{ not json").unwrap();
    assert_eq!(home.launch(), MemoryRegistration::SkippedRemoved);
    assert!(!home.agent("mcp.json").exists());

    let recorded = json!({ "version": 1, "registered": [{ "harness": "omp", "file": "~/.omp/agent/mcp.json" }] });
    assert_eq!(stored(&home), recorded);

    // The next one respects the removal too.
    assert_eq!(home.launch(), MemoryRegistration::SkippedRemoved);
    assert!(!home.agent("mcp.json").exists());
    assert_eq!(stored(&home), recorded);
}

#[test]
fn a_rebuilt_record_keeps_every_file_where_the_server_is_found() {
    let home = Home::new();
    home.write(
        "mcp.json",
        r#"{"mcpServers":{"memory":{"command":"/opt/harness-memory","args":["mcp"]}}}"#,
    );
    std::fs::create_dir_all(home.config()).unwrap();
    std::fs::write(
        home.config().join(MEMORY_RECORD_FILE),
        r#"{"version":7,"registered":[]}"#,
    )
    .unwrap();

    assert_eq!(home.launch(), MemoryRegistration::Present);

    let record = MemoryRecord::load(&home.config());
    assert!(!record.rebuilding());
    assert_eq!(
        record.registered(),
        [RecordedFile {
            harness: HarnessId::Omp,
            file: "~/.omp/agent/mcp.json".to_owned()
        }]
    );
}

#[test]
fn an_oversized_record_counts_as_corrupt() {
    let home = Home::new();
    std::fs::create_dir_all(home.config()).unwrap();
    let padding = "x".repeat(70 * 1024);
    std::fs::write(
        home.config().join(MEMORY_RECORD_FILE),
        format!(r#"{{"version":1,"registered":[],"pad":"{padding}"}}"#),
    )
    .unwrap();

    assert!(MemoryRecord::load(&home.config()).rebuilding());
}
