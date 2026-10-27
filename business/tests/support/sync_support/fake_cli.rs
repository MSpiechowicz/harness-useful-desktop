//! The fake harness CLI of the import tests (see [`FakeCli`]), a child of the import support
//! module so it shares the machine's [`Probe`].

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use harness_business::Result;
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ProcessRunner};
use serde_json::{Value, json};

use super::{
    CLAUDE_GLOBAL, CLAUDE_MARKETPLACES, CLAUDE_PLUGINS, CODEX_CONFIG, Machine, OMP_MARKETPLACES, OMP_PLUGINS,
    OPENCODE_JSON, OPENCODE_JSONC, PI_SETTINGS, Probe, write_json,
};
use crate::codex_support::{read_toml, write_toml};

/// The fake `omp`, `pi`, `opencode`, `claude` and `codex`: records each command line (program file
/// name first) and, unless told to fail it, does what it asks to the files of `home`: registers a
/// marketplace (named after `marketplace_names`), installs a user plugin, adds a PI package, adds an
/// OpenCode plugin to `opencode.json` (`plugin <spec> -g` to `plugin`, OpenCode 2's `plugin add
/// <spec>` to `plugins`), adds a server to the user scope of Claude Code's global file (`mcp
/// add-json`, refused with exit code 1 for a name it has, as Claude Code does; `mcp remove`) or to
/// Codex's `config.toml` (`mcp add`, which replaces a server of the same name and takes `--env`;
/// `mcp remove`).
///
/// `opencode debug config` answers the machine's [`Probe`]; it is recorded in
/// [`FakeCli::probes`], not in [`FakeCli::seen`].
pub struct FakeCli {
    home: PathBuf,
    marketplace_names: Vec<(String, String)>,
    failing: Vec<Vec<String>>,
    seen: Mutex<Vec<Vec<String>>>,
    probe: Arc<Mutex<Probe>>,
    probes: Mutex<Vec<ExecSpec>>,
}

impl FakeCli {
    pub fn new(machine: &Machine) -> Self {
        Self {
            home: machine.home().to_path_buf(),
            marketplace_names: Vec::new(),
            failing: Vec::new(),
            seen: Mutex::new(Vec::new()),
            probe: machine.probe.clone(),
            probes: Mutex::new(Vec::new()),
        }
    }

    /// The `opencode debug config` (OpenCode 2: `opencode api --standalone GET /api/config`)
    /// commands run.
    pub fn probes(&self) -> Vec<ExecSpec> {
        self.probes.lock().unwrap().clone()
    }

    /// The [`Probe::Merges`] names, or the output a [`Probe::Prints`] answers with.
    fn merged_names(&self) -> Result<Vec<String>, ExecOutput> {
        match &*self.probe.lock().unwrap() {
            Probe::Prints(output) => Err(output.clone()),
            Probe::Merges(names) => Ok(names.clone()),
        }
    }

    /// The answer of `opencode debug config`.
    fn merged_config(&self) -> ExecOutput {
        let names = match self.merged_names() {
            Ok(names) => names,
            Err(printed) => return printed,
        };

        let mut servers = serde_json::Map::new();
        for file in [OPENCODE_JSON, OPENCODE_JSONC, ".config/opencode/config.json"] {
            if let Some(found) = read_json(&self.home.join(file))["mcp"].as_object() {
                servers.extend(found.clone());
            }
        }
        for name in names {
            let org = json!({ "type": "remote", "url": "https://org.example.com/mcp",
                              "headers": { "Authorization": "***" } });
            servers.insert(name, org);
        }

        let config = json!({ "mcp": servers, "$schema": "https://opencode.ai/config.json", "plugin": [] });

        ExecOutput {
            status: Some(0),
            stdout: serde_json::to_string_pretty(&config).unwrap(),
            ..ExecOutput::default()
        }
    }

    /// The answer of OpenCode 2's `opencode api --standalone GET /api/config`: one document per
    /// OpenCode config file here (its `mcp` as written) and one, without a path, holding the
    /// [`Probe::Merges`] names under `mcp.servers` (a layer the app cannot read).
    fn merged_sources(&self) -> ExecOutput {
        let names = match self.merged_names() {
            Ok(names) => names,
            Err(printed) => return printed,
        };

        let mut sources = Vec::new();
        for file in [OPENCODE_JSON, OPENCODE_JSONC] {
            let path = self.home.join(file);
            if let Some(mcp) = read_json(&path).get("mcp") {
                sources.push(json!({ "type": "document", "path": path, "info": { "mcp": mcp } }));
            }
        }
        sources.push(json!({ "type": "directory", "path": self.home.join(".config/opencode") }));

        let org: serde_json::Map<String, Value> = names
            .into_iter()
            .map(|name| (name, json!({ "type": "remote", "url": "https://org.example.com/mcp" })))
            .collect();
        sources.push(json!({ "type": "document", "info": { "mcp": { "servers": org } } }));

        ExecOutput {
            status: Some(0),
            stdout: Value::Array(sources).to_string(),
            ..ExecOutput::default()
        }
    }

    /// `marketplace add <source>` registers the marketplace as `name`.
    pub fn naming(mut self, source: &str, name: &str) -> Self {
        self.marketplace_names.push((source.to_owned(), name.to_owned()));
        self
    }

    /// `argv` exits with 1 and an OMP-style failure line, changing nothing.
    pub fn failing(mut self, argv: &[&str]) -> Self {
        self.failing.push(argv.iter().map(|arg| (*arg).to_owned()).collect());
        self
    }

    pub fn seen(&self) -> Vec<Vec<String>> {
        self.seen.lock().unwrap().clone()
    }

    fn perform(&self, argv: &[String]) {
        let args: Vec<&str> = argv.iter().map(String::as_str).collect();

        match args.as_slice() {
            ["omp", "plugin", "marketplace", "add", source] => {
                let name = self
                    .marketplace_names
                    .iter()
                    .find(|(known, _)| known == source)
                    .map_or(*source, |(_, name)| name.as_str());
                let path = self.home.join(OMP_MARKETPLACES);
                let mut registry = read_json(&path);
                let entry = json!({ "name": name, "sourceType": "github", "sourceUri": source });

                match registry["marketplaces"].as_array_mut() {
                    Some(entries) => entries.push(entry),
                    None => registry = json!({ "version": 1, "marketplaces": [entry] }),
                }
                write_json(&path, &registry);
            }
            ["omp", "plugin", "install", id, "--scope", "user"] => {
                let path = self.home.join(OMP_PLUGINS);
                let mut registry = read_json(&path);
                if registry.get("plugins").is_none() {
                    registry = json!({ "version": 2, "plugins": {} });
                }
                registry["plugins"][*id] = json!([{ "scope": "user" }]);
                write_json(&path, &registry);
            }
            ["pi", "install", source] => {
                let path = self.home.join(PI_SETTINGS);
                let mut settings = read_json(&path);
                match settings["packages"].as_array_mut() {
                    Some(packages) => packages.push(json!(source)),
                    None => settings["packages"] = json!([source]),
                }
                write_json(&path, &settings);
            }
            ["opencode", "plugin", spec, "-g"] => {
                let path = self.home.join(OPENCODE_JSON);
                let mut config = read_json(&path);
                match config["plugin"].as_array_mut() {
                    Some(plugins) => plugins.push(json!(spec)),
                    None => config["plugin"] = json!([spec]),
                }
                write_json(&path, &config);
            }
            ["opencode", "plugin", "add", spec] => {
                let path = self.home.join(OPENCODE_JSON);
                let mut config = read_json(&path);
                match config["plugins"].as_array_mut() {
                    Some(plugins) => plugins.push(json!(spec)),
                    None => config["plugins"] = json!([spec]),
                }
                write_json(&path, &config);
            }
            [
                "claude",
                "plugin",
                "marketplace",
                "add",
                source,
                "--scope",
                "user",
                "--json",
            ] => {
                let name = self
                    .marketplace_names
                    .iter()
                    .find(|(known, _)| known == source)
                    .map_or(*source, |(_, name)| name.as_str());
                let recorded = match source.strip_prefix("https://") {
                    Some(_) => json!({ "source": "git", "url": source }),
                    None => json!({ "source": "github", "repo": source }),
                };
                let path = self.home.join(CLAUDE_MARKETPLACES);
                let mut registry = read_json(&path);
                registry[name] = json!({ "source": recorded, "installLocation": "~/.claude/plugins/marketplaces/x" });
                write_json(&path, &registry);
            }
            ["claude", "plugin", "install", id, "--scope", "user", "--json"] => {
                let path = self.home.join(CLAUDE_PLUGINS);
                let mut registry = read_json(&path);
                if registry.get("plugins").is_none() {
                    registry = json!({ "version": 2, "plugins": {} });
                }
                registry["plugins"][*id] = json!([{ "scope": "user", "version": "1.0.0" }]);
                write_json(&path, &registry);
            }
            ["codex", "plugin", "marketplace", "add", source] => {
                let name = self
                    .marketplace_names
                    .iter()
                    .find(|(known, _)| known == source)
                    .map_or(*source, |(_, name)| name.as_str());
                self.change_codex(|config| {
                    let marketplaces = table(config, "marketplaces");
                    marketplaces.insert(name.to_owned(), json!({ "source_type": "git", "source": source }));
                });
            }
            ["codex", "plugin", "add", id] => {
                self.change_codex(|config| {
                    table(config, "plugins").insert((*id).to_owned(), json!({ "enabled": true }));
                });
            }
            ["codex", "mcp", "add", name, rest @ ..] => {
                let entry = codex_entry(rest);
                self.change_codex(|config| {
                    table(config, "mcp_servers").insert((*name).to_owned(), entry);
                });
            }
            ["codex", "mcp", "remove", name] => {
                self.change_codex(|config| {
                    table(config, "mcp_servers").shift_remove(*name);
                });
            }
            _ => panic!("unexpected command {argv:?}"),
        }
    }

    /// Reads Codex's `config.toml`, lets `change` edit it and writes it back.
    fn change_codex(&self, change: impl FnOnce(&mut serde_json::Map<String, Value>)) {
        let path = self.home.join(CODEX_CONFIG);
        let mut config = read_toml(&path);
        change(&mut config);
        write_toml(&path, &config);
    }

    /// `claude mcp add-json --scope user <name> <json>` and `claude mcp remove --scope user
    /// <name>` (exit code 1 for a name it does not have): `None` for any other command line.
    fn claude_mcp(&self, argv: &[String]) -> Option<ExecOutput> {
        let args: Vec<&str> = argv.iter().map(String::as_str).collect();
        let path = self.home.join(CLAUDE_GLOBAL);

        if let ["claude", "mcp", "remove", "--scope", "user", name] = args.as_slice() {
            let mut global = read_json(&path);
            let removed = global["mcpServers"]
                .as_object_mut()
                .and_then(|servers| servers.shift_remove(*name));

            return Some(match removed {
                Some(_) => {
                    write_json(&path, &global);
                    ExecOutput {
                        status: Some(0),
                        stdout: format!("Removed MCP server {name} from user config"),
                        ..ExecOutput::default()
                    }
                }
                None => ExecOutput {
                    status: Some(1),
                    stderr: format!("No user-scoped MCP server found with name: {name}"),
                    ..ExecOutput::default()
                },
            });
        }

        let ["claude", "mcp", "add-json", "--scope", "user", name, definition] = args.as_slice() else {
            return None;
        };

        let mut global = read_json(&path);
        if global["mcpServers"].get(*name).is_some() {
            return Some(ExecOutput {
                status: Some(1),
                stderr: format!("MCP server {name} already exists in user config"),
                ..ExecOutput::default()
            });
        }

        global["mcpServers"][*name] = serde_json::from_str(definition).unwrap();
        write_json(&path, &global);

        Some(ExecOutput {
            status: Some(0),
            stdout: format!("Added MCP server {name} to user config"),
            ..ExecOutput::default()
        })
    }
}

impl ProcessRunner for FakeCli {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        let program = spec.program.file_name().unwrap().to_string_lossy().into_owned();
        let mut argv = vec![program];
        argv.extend(spec.args.iter().map(|arg| arg.to_string_lossy().into_owned()));

        if argv == ["opencode", "debug", "config"] {
            self.probes.lock().unwrap().push(spec.clone());
            let output = self.merged_config();

            return Box::pin(async move { Ok(output) });
        }

        if argv == ["opencode", "api", "--standalone", "GET", "/api/config"] {
            self.probes.lock().unwrap().push(spec.clone());
            let output = self.merged_sources();

            return Box::pin(async move { Ok(output) });
        }

        self.seen.lock().unwrap().push(argv.clone());

        let output = if self.failing.contains(&argv) {
            ExecOutput {
                status: Some(1),
                stderr: "✘ Failed to add marketplace: Error: not found".to_owned(),
                ..ExecOutput::default()
            }
        } else if let Some(output) = self.claude_mcp(&argv) {
            output
        } else {
            self.perform(&argv);
            ExecOutput {
                status: Some(0),
                stdout: "Done.".to_owned(),
                ..ExecOutput::default()
            }
        };

        Box::pin(async move { Ok(output) })
    }
}

fn read_json(path: &Path) -> Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| json!({}))
}

/// The entry `codex mcp add <name> <rest…>` writes: `[--env KEY=VALUE]… -- <command> <args…>`, or
/// `--url <url> [--bearer-token-env-var <name>]`.
fn codex_entry(rest: &[&str]) -> Value {
    let mut env = serde_json::Map::new();
    let mut entry = json!({});
    let mut words = rest.iter();

    while let Some(word) = words.next() {
        match *word {
            "--env" => {
                let (key, value) = words.next().and_then(|pair| pair.split_once('=')).expect("KEY=VALUE");
                env.insert(key.to_owned(), json!(value));
            }
            "--url" => entry["url"] = json!(words.next().expect("a url")),
            "--bearer-token-env-var" => entry["bearer_token_env_var"] = json!(words.next().expect("a variable")),
            "--" => {
                let command = words.next().expect("a command");
                entry["command"] = json!(command);
                entry["args"] = json!(words.copied().collect::<Vec<_>>());
                break;
            }
            other => panic!("unexpected codex add argument {other}"),
        }
    }

    if !env.is_empty() {
        entry["env"] = Value::Object(env);
    }

    entry
}

/// The table `key` of `config`, created when missing.
fn table<'a>(config: &'a mut serde_json::Map<String, Value>, key: &str) -> &'a mut serde_json::Map<String, Value> {
    config.entry(key).or_insert_with(|| json!({})).as_object_mut().unwrap()
}
