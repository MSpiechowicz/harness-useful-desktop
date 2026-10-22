//! The app's own MCP server (`harness-desktop mcp`) in a harness's user MCP file: added by absolute
//! path once, kept out after the user removed it, repointed when the app's program moved (an
//! AppImage, an update), never changing an entry that is not the app's, and the setup step's view
//! of it. Every file lives in a temporary home.

use crate::mcp_support;

use std::path::{Path, PathBuf};

use harness_business::ecosystem::{
    DESKTOP_RECORD_FILE, DesktopRecord, DesktopRegistration, McpFile, Shape, desktop_mcp_state, register_desktop_mcp,
};
use harness_business::model::{HarnessId, McpState};
use mcp_support::{Home, keys, read};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[cfg(unix)]
use mcp_support::mode;

/// The app as a launch sees it: where its program is and its config folder.
struct App<'a> {
    home: &'a Home,
    program: PathBuf,
}

impl<'a> App<'a> {
    fn at(home: &'a Home, program: &str) -> Self {
        Self {
            home,
            program: home.dir.path().join(program),
        }
    }

    fn config(&self) -> PathBuf {
        self.home.dir.path().join("config")
    }

    fn omp(&self) -> McpFile {
        McpFile {
            path: self.home.agent("mcp.json"),
            also_read: vec![self.home.agent(".mcp.json")],
            shape: Shape::McpServers,
        }
    }

    /// One launch over `file`: registers as the record says, notes the outcome and saves it.
    fn launch_in(&self, file: &McpFile) -> DesktopRegistration {
        let env = &self.home.env;
        let mut record = DesktopRecord::load(&self.config());

        let outcome = register_desktop_mcp(
            file,
            &self.program,
            record.is_recorded(env, &file.path),
            record.command_in(env, &file.path),
        );

        record.note(env, HarnessId::Omp, &file.path, outcome, &self.program);
        record.save().unwrap();
        outcome
    }

    fn launch(&self) -> DesktopRegistration {
        self.launch_in(&self.omp())
    }

    fn command(&self) -> String {
        self.program.to_string_lossy().into_owned()
    }
}

fn servers(path: &Path) -> Value {
    read(path)["mcpServers"].clone()
}

#[test]
fn the_app_is_added_once_by_absolute_path() {
    let home = Home::new();
    let app = App::at(&home, "apps/harness-desktop");

    assert_eq!(app.launch(), DesktopRegistration::Added);
    assert_eq!(
        servers(&home.agent("mcp.json")),
        json!({ "harness-desktop": { "command": app.command(), "args": ["mcp"] } })
    );

    #[cfg(unix)]
    assert_eq!(mode(&home.agent("mcp.json")), 0o600);

    assert_eq!(
        app.launch(),
        DesktopRegistration::Present,
        "the next launch changes nothing"
    );

    let record: Value =
        serde_json::from_str(&std::fs::read_to_string(app.config().join(DESKTOP_RECORD_FILE)).unwrap()).unwrap();
    assert_eq!(
        record,
        json!({ "version": 1, "registered": [
            { "harness": "omp", "file": "~/.omp/agent/mcp.json", "command": app.command() }
        ] })
    );
}

#[test]
fn an_entry_the_user_removed_stays_removed() {
    let home = Home::new();
    let app = App::at(&home, "apps/harness-desktop");
    assert_eq!(app.launch(), DesktopRegistration::Added);

    let without = r#"{"mcpServers":{"github":{"command":"github-mcp"}}}"#;
    home.write("mcp.json", without);

    assert_eq!(app.launch(), DesktopRegistration::SkippedRemoved);
    assert_eq!(std::fs::read_to_string(home.agent("mcp.json")).unwrap(), without);

    assert_eq!(
        register_desktop_mcp(&app.omp(), &app.program, false, None),
        DesktopRegistration::Added,
        "the user asking for it adds it back"
    );
}

#[test]
fn a_moved_appimage_is_repointed_and_the_other_fields_kept() {
    let home = Home::new();
    let first = App::at(&home, "Downloads/Harness_Desktop_0.21.0_amd64.AppImage");
    assert_eq!(first.launch(), DesktopRegistration::Added);

    // The user adds a field to the entry, then moves the AppImage.
    let path = home.agent("mcp.json");
    let mut document = read(&path);
    document["mcpServers"]["harness-desktop"]["timeout"] = json!(30);
    std::fs::write(&path, document.to_string()).unwrap();

    let moved = App::at(&home, "Applications/Harness_Desktop_0.22.0_amd64.AppImage");
    assert_eq!(moved.launch(), DesktopRegistration::Updated);

    let entry = &servers(&path)["harness-desktop"];
    assert_eq!(
        entry,
        &json!({ "command": moved.command(), "args": ["mcp"], "timeout": 30 })
    );
    assert_eq!(keys(entry), vec!["command", "args", "timeout"]);

    assert_eq!(moved.launch(), DesktopRegistration::Present);
}

#[test]
fn an_entry_running_harness_desktop_elsewhere_is_repointed() {
    let home = Home::new();
    home.write(
        "mcp.json",
        r#"{"mcpServers":{"harness-desktop":{"command":"/opt/old/harness-desktop.exe","args":["mcp"],"env":{"A":"1"}}}}"#,
    );
    let app = App::at(&home, "apps/harness-desktop");

    assert_eq!(app.launch(), DesktopRegistration::Updated);
    assert_eq!(
        servers(&home.agent("mcp.json")),
        json!({ "harness-desktop": { "command": app.command(), "args": ["mcp"], "env": { "A": "1" } } })
    );
}

#[test]
fn entries_that_are_not_the_apps_are_left_alone() {
    let home = Home::new();
    let app = App::at(&home, "apps/harness-desktop");

    let users_own = r#"{"mcpServers":{"harness-desktop":{"command":"/opt/proxy/hooks-proxy","args":["mcp"]}}}"#;
    home.write("mcp.json", users_own);
    assert_eq!(
        app.launch(),
        DesktopRegistration::Present,
        "a program that is not the app"
    );
    assert_eq!(std::fs::read_to_string(home.agent("mcp.json")).unwrap(), users_own);

    let extra_args = r#"{"mcpServers":{"harness-desktop":{"command":"/opt/harness-desktop","args":["mcp","--x"]}}}"#;
    home.write("mcp.json", extra_args);
    assert_eq!(app.launch(), DesktopRegistration::Present, "other arguments");
    assert_eq!(std::fs::read_to_string(home.agent("mcp.json")).unwrap(), extra_args);
}

#[test]
fn an_entry_in_a_file_the_harness_also_reads_counts_and_is_not_changed() {
    let home = Home::new();
    let other = r#"{"mcpServers":{"hooks":{"command":"/old/harness-desktop","args":["mcp"],"enabled":false}}}"#;
    home.write(".mcp.json", other);
    let app = App::at(&home, "apps/harness-desktop");

    assert_eq!(app.launch(), DesktopRegistration::Disabled);
    assert!(!home.agent("mcp.json").exists(), "nothing is added beside it");
    assert_eq!(std::fs::read_to_string(home.agent(".mcp.json")).unwrap(), other);
}

#[test]
fn opencode_gets_a_command_array_and_its_program_follows_the_app() {
    let home = Home::new();
    let file = McpFile {
        path: home.dir.path().join(".config/opencode/opencode.json"),
        also_read: Vec::new(),
        shape: Shape::OpenCode,
    };

    let first = App::at(&home, "one/harness-desktop");
    assert_eq!(first.launch_in(&file), DesktopRegistration::Added);
    assert_eq!(
        read(&file.path)["mcp"]["harness-desktop"],
        json!({ "type": "local", "command": [first.command(), "mcp"], "enabled": true })
    );

    let second = App::at(&home, "two/harness-desktop");
    assert_eq!(second.launch_in(&file), DesktopRegistration::Updated);
    assert_eq!(
        read(&file.path)["mcp"]["harness-desktop"]["command"],
        json!([second.command(), "mcp"])
    );
}

#[test]
fn a_file_with_comments_is_never_written_and_the_state_has_the_entry_to_paste() {
    let home = Home::new();
    let path = home.dir.path().join(".config/opencode/opencode.jsonc");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let text = "{\n  // mine\n  \"mcp\": {}\n}\n";
    std::fs::write(&path, text).unwrap();

    let file = McpFile {
        path: path.clone(),
        also_read: Vec::new(),
        shape: Shape::OpenCode,
    };
    let app = App::at(&home, "apps/harness-desktop");

    assert_eq!(app.launch_in(&file), DesktopRegistration::ManualEdit);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), text);

    let McpState::ManualEdit { file: shown, snippet } = desktop_mcp_state(&home.env, &file, Some(&app.program)) else {
        panic!("a manual edit is offered");
    };
    assert_eq!(shown, "~/.config/opencode/opencode.jsonc");
    assert!(snippet.starts_with("\"harness-desktop\": {"));

    // The snippet is JSON: a Windows path has its backslashes escaped in it.
    let pasted: Value = serde_json::from_str(&format!("{{{snippet}}}")).expect("the snippet is a JSON member");
    assert_eq!(pasted["harness-desktop"]["command"], json!([app.command(), "mcp"]));
}

#[test]
fn the_setup_state_reads_the_file() {
    let home = Home::new();
    let app = App::at(&home, "apps/harness-desktop");

    assert_eq!(
        desktop_mcp_state(&home.env, &app.omp(), Some(&app.program)),
        McpState::Missing {
            file: "~/.omp/agent/mcp.json".to_owned(),
            exists: false
        }
    );

    app.launch();
    assert_eq!(
        desktop_mcp_state(&home.env, &app.omp(), None),
        McpState::Configured {
            file: "~/.omp/agent/mcp.json".to_owned(),
            name: "harness-desktop".to_owned()
        }
    );
}

#[test]
fn a_corrupt_record_adds_nothing_and_is_rebuilt() {
    let home = Home::new();
    let app = App::at(&home, "apps/harness-desktop");
    std::fs::create_dir_all(app.config()).unwrap();
    std::fs::write(app.config().join(DESKTOP_RECORD_FILE), "{ not json").unwrap();

    assert_eq!(app.launch(), DesktopRegistration::SkippedRemoved);
    assert!(!home.agent("mcp.json").exists());

    let record = DesktopRecord::load(&app.config());
    assert!(!record.rebuilding());
    assert_eq!(record.entries().len(), 1, "the file checked is recorded");
    assert_eq!(record.entries()[0].command, None);

    let aside = std::fs::read_dir(app.config())
        .unwrap()
        .filter(|entry| {
            let name = entry.as_ref().unwrap().file_name();
            name.to_string_lossy()
                .starts_with("desktop-mcp-registration.json.corrupt-")
        })
        .count();
    assert_eq!(aside, 1, "the unusable record is kept aside");

    assert_eq!(
        app.launch(),
        DesktopRegistration::SkippedRemoved,
        "later launches keep it out too"
    );
}

#[cfg(unix)]
#[test]
fn an_appimage_is_registered_by_its_file_not_its_mount() {
    use harness_business::ecosystem::app_command;
    use std::ffi::OsStr;

    let mount = Path::new("/tmp/.mount_HarnesXyz/usr/bin/harness-desktop");
    let appdir = Some(OsStr::new("/tmp/.mount_HarnesXyz"));
    let appimage = Some(OsStr::new("/home/u/Apps/Harness.AppImage"));

    assert_eq!(
        app_command(mount, appimage, appdir),
        PathBuf::from("/home/u/Apps/Harness.AppImage")
    );
    assert_eq!(app_command(mount, Some(OsStr::new("relative.AppImage")), appdir), mount);
    assert_eq!(app_command(mount, None, appdir), mount);
}

#[cfg(unix)]
#[test]
fn appimage_alone_does_not_redirect_an_app_outside_its_mount() {
    use harness_business::ecosystem::app_command;
    use std::ffi::OsStr;

    let installed = Path::new("/usr/bin/harness-desktop");
    let appimage = Some(OsStr::new("/home/u/Apps/Other.AppImage"));

    // Inherited from another AppImage that started this app: no APPDIR, or one elsewhere.
    assert_eq!(app_command(installed, appimage, None), installed);
    assert_eq!(
        app_command(installed, appimage, Some(OsStr::new("/tmp/.mount_OtherAbc"))),
        installed
    );
    assert_eq!(
        app_command(installed, appimage, Some(OsStr::new("relative/mount"))),
        installed
    );
}
