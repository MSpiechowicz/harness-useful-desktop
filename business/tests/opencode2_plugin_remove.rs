//! Removing an OpenCode 2 plugin by editing its global config file, with the same preview as
//! OpenCode 1: plain entries go from `plugins` and the legacy `plugin` (no copy is left under the
//! other key), entries with options and commented files are left to the user, the file is the one
//! OpenCode 2's own `plugin add` writes, and `OPENCODE_CONFIG_DIR` replaces the global directory.

use std::path::{Path, PathBuf};

use harness_business::BusinessError;
use harness_business::env::Env;
use harness_business::harness::{Harness, OpenCodeGeneration};
use harness_business::harnesses::OpenCode;
use harness_business::model::{CliResult, ConfigEdit, ExtensionAction};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tempfile::TempDir;

struct Home {
    dir: TempDir,
    env: Env,
}

impl Home {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("home");
        let env = Env::for_tests(dir.path());

        Self { dir, env }
    }

    /// `name` in `~/.config/opencode`.
    fn config(&self, name: &str) -> PathBuf {
        self.dir.path().join(".config/opencode").join(name)
    }

    fn write(&self, path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
        std::fs::write(path, text).expect("write");
    }

    fn remove(&self, name: &str, project: Option<&Path>, dry_run: bool) -> harness_business::Result<CliResult> {
        remove_with(&self.env, name, project, dry_run)
    }
}

fn remove_with(env: &Env, name: &str, project: Option<&Path>, dry_run: bool) -> harness_business::Result<CliResult> {
    let action = ExtensionAction::PluginUninstall { name: name.to_owned() };

    OpenCode::with_generation(OpenCodeGeneration::V2)
        .config_action(env, &action, project, dry_run)
        .map(|result| result.expect("a config edit"))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).expect("read")
}

fn parsed(path: &Path) -> Value {
    serde_json::from_str(&read(path)).expect("json")
}

fn edit(file: &str, manual_edit: bool) -> Option<ConfigEdit> {
    Some(ConfigEdit {
        file: file.to_owned(),
        manual_edit,
    })
}

const GLOBAL: &str = "~/.config/opencode/opencode.json";

#[test]
fn removal_drops_plain_entries_from_both_keys_and_keeps_everything_else() {
    let home = Home::new();
    let file = home.config("opencode.json");
    home.write(
        &file,
        r#"{
  "$schema": "https://opencode.ai/config.json",
  "plugin": ["first", "demo@1.0.0", ["other", {"x": 1}]],
  "model": "anthropic/claude",
  "plugins": [{"package": "demo"}, "demo-extra", "-gone", {"package": "kept", "options": {"y": 2}}, "demo@latest"]
}"#,
    );

    let result = home.remove("demo", None, false).expect("removed");

    assert!(result.ok, "{result:?}");
    assert_eq!(result.config_edit, edit(GLOBAL, false));
    assert!(result.argv.is_empty() && result.command.is_empty());
    assert_eq!(
        result.summary.as_deref(),
        Some("Removed this plugin from \"plugin\" and \"plugins\" in ~/.config/opencode/opencode.json.")
    );
    assert_eq!(
        parsed(&file),
        json!({
            "$schema": "https://opencode.ai/config.json",
            "plugin": ["first", ["other", {"x": 1}]],
            "model": "anthropic/claude",
            "plugins": ["demo-extra", "-gone", {"package": "kept", "options": {"y": 2}}]
        })
    );
    let keys: Vec<String> = parsed(&file).as_object().expect("object").keys().cloned().collect();
    assert_eq!(keys, ["$schema", "plugin", "model", "plugins"], "key order is kept");

    let second = home.remove("demo", None, false).expect("a no-op");
    assert!(second.ok);
    assert!(second.summary.expect("summary").contains("not listed"));
}

#[test]
fn a_preview_names_the_key_and_writes_nothing() {
    let home = Home::new();
    let file = home.config("opencode.json");
    let original = r#"{"plugins": ["demo", "kept"]}"#;
    home.write(&file, original);

    let preview = home.remove("demo", None, true).expect("a preview");

    assert!(preview.ok);
    assert_eq!(preview.config_edit, edit(GLOBAL, false));
    assert_eq!(
        preview.summary.as_deref(),
        Some("Would remove this plugin from \"plugins\" in ~/.config/opencode/opencode.json. Nothing was changed.")
    );
    assert_eq!(read(&file), original);
}

#[test]
fn entries_with_options_and_commented_files_are_left_to_the_user() {
    let home = Home::new();
    let file = home.config("opencode.json");

    let cases = [
        (
            r#"{"plugins": ["demo", {"package": "demo", "options": {"k": 1}}]}"#,
            "\"plugins\"",
        ),
        (r#"{"plugin": [["demo", {"k": 1}]], "plugins": ["demo"]}"#, "\"plugin\""),
        (
            "{\n  // mine\n  \"plugins\": [\"demo\", \"kept\"]\n}\n",
            "\"plugins\" there",
        ),
    ];

    for (original, named) in cases {
        home.write(&file, original);

        for dry_run in [true, false] {
            let result = home.remove("demo", None, dry_run).expect("a manual edit");

            assert!(!result.ok, "{original}");
            assert_eq!(result.config_edit, edit(GLOBAL, true));
            assert!(result.summary.expect("summary").contains(named), "{original}");
            assert_eq!(read(&file), original, "no byte changed");
        }
    }
}

#[test]
fn the_file_opencode_2_writes_is_edited_and_config_json_is_not_read() {
    let home = Home::new();
    let json = home.config("opencode.json");
    let jsonc = home.config("opencode.jsonc");
    let legacy = home.config("config.json");
    home.write(&json, r#"{"plugins": ["demo", "kept"]}"#);
    home.write(&jsonc, r#"{"plugins": ["other"]}"#);
    home.write(&legacy, r#"{"plugin": ["demo"]}"#);

    let result = home.remove("demo", None, false).expect("removed");

    assert!(result.ok, "{result:?}");
    assert_eq!(result.config_edit, edit(GLOBAL, false));
    assert_eq!(
        result.summary.as_deref(),
        Some("Removed this plugin from \"plugins\" in ~/.config/opencode/opencode.json."),
        "config.json is not an OpenCode 2 layer"
    );
    assert_eq!(parsed(&json)["plugins"], json!(["kept"]));
    assert_eq!(read(&jsonc), r#"{"plugins": ["other"]}"#);
}

#[test]
fn the_custom_config_dir_replaces_the_global_one() {
    let home = Home::new();
    let custom = home.dir.path().join("team-config");
    let env = home
        .env
        .clone()
        .with_var("OPENCODE_CONFIG_DIR", custom.to_string_lossy());
    let default_file = home.config("opencode.json");
    home.write(&default_file, r#"{"plugins": ["demo"]}"#);
    home.write(&custom.join("opencode.jsonc"), r#"{"plugins": ["demo", "kept"]}"#);

    let result = remove_with(&env, "demo", None, false).expect("removed");

    assert!(result.ok, "{result:?}");
    assert_eq!(result.config_edit, edit("~/team-config/opencode.jsonc", false));
    assert_eq!(parsed(&custom.join("opencode.jsonc"))["plugins"], json!(["kept"]));
    assert_eq!(
        read(&default_file),
        r#"{"plugins": ["demo"]}"#,
        "not read on OpenCode 2"
    );
}

#[test]
fn a_plugin_defined_elsewhere_is_left_to_the_user() {
    let home = Home::new();
    let global = home.config("opencode.json");
    home.write(&global, r#"{"plugins": ["kept"]}"#);

    let project = home.dir.path().join("work/app");
    let project_file = project.join("opencode.json");
    home.write(&project_file, r#"{"plugins": ["demo"]}"#);

    let in_project = home.remove("demo", Some(&project), false).expect("a manual edit");

    assert!(!in_project.ok);
    assert_eq!(in_project.config_edit, edit("~/work/app/opencode.json", true));
    assert_eq!(read(&project_file), r#"{"plugins": ["demo"]}"#);

    let folder = home.config("plugins/local-demo");
    std::fs::create_dir_all(&folder).expect("plugin folder");
    let id = folder.to_string_lossy().into_owned();

    let local = home.remove(&id, None, false).expect("a manual edit");

    assert!(!local.ok);
    assert_eq!(local.config_edit, edit("~/.config/opencode/plugins/local-demo", true));
    assert_eq!(
        local.summary.as_deref(),
        Some("This plugin is the folder ~/.config/opencode/plugins/local-demo. Delete that folder to remove it.")
    );
    assert!(folder.exists());
}

#[test]
fn a_plugins_key_that_is_not_a_list_is_refused() {
    let home = Home::new();
    let file = home.config("opencode.json");
    home.write(&file, r#"{"plugin": ["demo"], "plugins": "demo"}"#);

    let error = home.remove("demo", None, false).expect_err("not a list");

    assert!(matches!(error, BusinessError::InvalidInput(_)), "{error:?}");
    assert!(error.to_string().contains("\"plugins\""), "{error}");
    assert_eq!(read(&file), r#"{"plugin": ["demo"], "plugins": "demo"}"#);
}
