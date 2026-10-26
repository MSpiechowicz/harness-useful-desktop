//! Removing an OpenCode plugin by editing the `plugin` list of the global config file (OpenCode
//! has no uninstall command): the exact entry goes and everything else stays, removing twice is
//! harmless, a preview never writes, and files the app cannot rewrite safely are left alone.

use std::path::{Path, PathBuf};

use harness_business::BusinessError;
use harness_business::env::Env;
use harness_business::harness::Harness;
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
        let action = ExtensionAction::PluginUninstall { name: name.to_owned() };

        OpenCode::default()
            .config_action(&self.env, &action, project, dry_run)
            .map(|result| result.expect("a config edit"))
    }
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
fn removal_drops_the_exact_entry_and_keeps_everything_else() {
    let home = Home::new();
    let file = home.config("opencode.json");
    home.write(
        &file,
        r#"{
  "$schema": "https://opencode.ai/config.json",
  "model": "anthropic/claude",
  "plugin": ["first", "demo@1.2.0", "demo-extra", "@scope/demo", ["other", {"x": 1}], "last"],
  "mcp": {"server": {"type": "local", "command": ["x"]}}
}"#,
    );

    let result = home.remove("demo", None, false).expect("removed");

    assert!(result.ok, "{result:?}");
    assert_eq!(result.config_edit, edit(GLOBAL, false));
    assert_eq!(result.exit_code, None);
    assert!(result.argv.is_empty() && result.command.is_empty());
    assert_eq!(
        parsed(&file),
        json!({
            "$schema": "https://opencode.ai/config.json",
            "model": "anthropic/claude",
            "plugin": ["first", "demo-extra", "@scope/demo", ["other", {"x": 1}], "last"],
            "mcp": {"server": {"type": "local", "command": ["x"]}}
        })
    );
    let keys: Vec<String> = parsed(&file).as_object().expect("object").keys().cloned().collect();
    assert_eq!(keys, ["$schema", "model", "plugin", "mcp"], "key order is kept");

    let scoped = home.remove("@scope/demo", None, false).expect("removed");
    assert!(scoped.ok);
    assert_eq!(
        parsed(&file)["plugin"],
        json!(["first", "demo-extra", ["other", {"x": 1}], "last"])
    );
}

#[test]
fn removing_twice_or_from_a_missing_file_changes_nothing() {
    let home = Home::new();
    let file = home.config("opencode.json");

    let missing = home.remove("demo", None, false).expect("nothing to do");
    assert!(missing.ok);
    assert_eq!(missing.config_edit, edit(GLOBAL, false));
    assert!(!file.exists(), "no file is created");

    home.write(&file, r#"{"plugin": ["demo@latest", "kept"]}"#);
    assert!(home.remove("demo", None, false).expect("removed").ok);
    let after_first = read(&file);

    let second = home.remove("demo", None, false).expect("a no-op");

    assert!(second.ok);
    assert!(second.summary.expect("summary").contains("not listed"));
    assert_eq!(read(&file), after_first, "the file is not rewritten");
}

#[test]
fn a_preview_reports_the_edit_without_writing() {
    let home = Home::new();
    let file = home.config("opencode.json");
    let original = r#"{"plugin": ["demo"]}"#;
    home.write(&file, original);

    let preview = home.remove("demo", None, true).expect("a preview");

    assert!(preview.ok);
    assert_eq!(preview.config_edit, edit(GLOBAL, false));
    assert!(preview.summary.expect("summary").starts_with("Would remove"));
    assert_eq!(read(&file), original);
}

#[test]
fn a_file_with_comments_is_left_to_the_user() {
    let home = Home::new();
    let file = home.config("opencode.json");
    let original = "{\n  // my plugins\n  \"plugin\": [\"demo\", \"kept\"]\n}\n";
    home.write(&file, original);

    for dry_run in [true, false] {
        let result = home.remove("demo", None, dry_run).expect("a manual edit");

        assert!(!result.ok);
        assert_eq!(result.config_edit, edit(GLOBAL, true));
        assert!(result.summary.expect("summary").contains("comments"));
        assert_eq!(read(&file), original, "no byte changed");
    }

    let absent = home.remove("not-there", None, false).expect("a no-op");
    assert!(absent.ok, "an absent entry needs no edit, comments or not");
}

#[test]
fn an_entry_with_options_is_left_to_the_user() {
    let home = Home::new();
    let file = home.config("opencode.json");
    let original = r#"{"plugin": ["demo", ["demo@1.0.0", {"apiKey": "x"}]]}"#;
    home.write(&file, original);

    let result = home.remove("demo", None, false).expect("a manual edit");

    assert!(!result.ok);
    assert_eq!(result.config_edit, edit(GLOBAL, true));
    assert_eq!(read(&file), original);
}

#[test]
fn opencode_jsonc_is_edited_when_present() {
    let home = Home::new();
    let json = home.config("opencode.json");
    let jsonc = home.config("opencode.jsonc");
    home.write(&json, r#"{"plugin": ["other"]}"#);
    home.write(&jsonc, r#"{"plugin": ["demo", "kept",]}"#);

    let result = home.remove("demo", None, false).expect("removed");

    assert!(result.ok, "{result:?}");
    assert_eq!(result.config_edit, edit("~/.config/opencode/opencode.jsonc", false));
    assert_eq!(parsed(&jsonc)["plugin"], json!(["kept"]));
    assert_eq!(read(&json), r#"{"plugin": ["other"]}"#);
}

#[test]
fn a_plugin_defined_elsewhere_is_left_to_the_user() {
    let home = Home::new();
    let global = home.config("opencode.json");
    let global_text = r#"{"plugin": ["kept"]}"#;
    home.write(&global, global_text);

    let project = home.dir.path().join("work/app");
    let project_file = project.join("opencode.json");
    home.write(&project_file, r#"{"plugin": ["demo"]}"#);

    let in_project = home.remove("demo", Some(&project), false).expect("a manual edit");

    assert!(!in_project.ok);
    assert_eq!(in_project.config_edit, edit("~/work/app/opencode.json", true));
    assert_eq!(read(&project_file), r#"{"plugin": ["demo"]}"#);

    let plugin_file = home.config("plugins/local-demo.ts");
    home.write(&plugin_file, "export default {}\n");
    let id = plugin_file.to_string_lossy().into_owned();

    let local = home.remove(&id, None, false).expect("a manual edit");

    assert!(!local.ok);
    assert_eq!(
        local.config_edit,
        edit("~/.config/opencode/plugins/local-demo.ts", true)
    );
    assert!(local.summary.expect("summary").contains("Delete"));
    assert!(plugin_file.exists());
    assert_eq!(read(&global), global_text);
}

#[test]
fn a_removal_names_the_other_file_that_still_lists_the_plugin() {
    let home = Home::new();
    let file = home.config("opencode.json");
    let legacy = home.config("config.json");
    let legacy_text = r#"{"plugin": ["demo@1.0.0"]}"#;
    home.write(&legacy, legacy_text);
    home.write(&file, r#"{"plugin": ["demo", "kept"]}"#);

    let preview = home.remove("demo", None, true).expect("a preview");

    assert!(preview.ok);
    assert_eq!(
        preview.summary.as_deref(),
        Some(
            "Would remove this plugin from \"plugin\" in ~/.config/opencode/opencode.json. Nothing was changed. \
             It is still listed in ~/.config/opencode/config.json."
        )
    );

    let removed = home.remove("demo", None, false).expect("removed");

    assert!(removed.ok);
    assert_eq!(removed.config_edit, edit(GLOBAL, false));
    assert_eq!(
        removed.summary.as_deref(),
        Some(
            "Removed this plugin from \"plugin\" in ~/.config/opencode/opencode.json. \
             It is still listed in ~/.config/opencode/config.json."
        )
    );
    assert_eq!(parsed(&file)["plugin"], json!(["kept"]));
    assert_eq!(read(&legacy), legacy_text, "the other file is never edited");

    let alone = Home::new();
    alone.write(&alone.config("opencode.json"), r#"{"plugin": ["demo"]}"#);
    let summary = alone.remove("demo", None, false).expect("removed").summary;
    assert_eq!(
        summary.as_deref(),
        Some("Removed this plugin from \"plugin\" in ~/.config/opencode/opencode.json.")
    );
}

#[test]
fn files_the_app_cannot_rewrite_safely_are_refused() {
    let home = Home::new();
    let file = home.config("opencode.json");

    home.write(&file, "{ not json");
    let error = home.remove("demo", None, false).expect_err("unparsable");
    assert!(matches!(error, BusinessError::InvalidInput(_)), "{error:?}");

    home.write(&file, r#"{"plugin": "demo"}"#);
    let error = home.remove("demo", None, false).expect_err("not a list");
    assert!(matches!(error, BusinessError::InvalidInput(_)), "{error:?}");
    assert_eq!(read(&file), r#"{"plugin": "demo"}"#);
}

#[cfg(unix)]
#[test]
fn a_symlinked_config_file_is_refused() {
    let home = Home::new();
    let real = home.dir.path().join("dotfiles/opencode.json");
    home.write(&real, r#"{"plugin": ["demo"]}"#);
    std::fs::create_dir_all(home.config("")).expect("config dir");
    std::os::unix::fs::symlink(&real, home.config("opencode.json")).expect("symlink");

    for dry_run in [true, false] {
        let error = home.remove("demo", None, dry_run).expect_err("a symlink");

        assert!(matches!(error, BusinessError::Forbidden(_)), "{error:?}");
    }
    assert_eq!(read(&real), r#"{"plugin": ["demo"]}"#);
}

#[test]
fn names_that_cannot_be_shown_back_are_refused() {
    let home = Home::new();

    for name in ["", "a\nb", "a\u{202e}b", &"x".repeat(513)] {
        let error = home.remove(name, None, false).expect_err(name);

        assert!(matches!(error, BusinessError::InvalidInput(_)), "{name:?}: {error:?}");
    }
}
