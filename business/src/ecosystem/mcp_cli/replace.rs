//! Share setup replacing a server that exists with another definition, through the harness's CLI
//! (see the module docs of [`super`]): the entry is removed and added again. When the add fails the
//! old entry is added back ([`super::replace_with`]).
//!
//! For the same HTTP or SSE endpoint (see `sync::endpoint`) the new entry is the old one with the
//! bundle's URL swapped in, so its other settings are carried over. Claude Code takes the whole
//! entry as JSON (`mcp add-json`): `headers`, `oauth` and the rest stay. Codex's `mcp add` carries a
//! URL and its `--bearer-token-env-var`. For anything else (a stdio server, another endpoint, a
//! change of transport) the new entry is the fresh definition a new add writes and nothing of the
//! old one is carried over.
//!
//! Codex can only write back an entry of a command, arguments and `env`, or of a URL and a bearer
//! variable ([`codex_replaceable`]): an old entry with anything else is not replaced at all, since
//! a failed add could not be rolled back. The old entry's values go on the command line only when it
//! is added back after a failed add, or (Keep) for the Claude JSON, so the local process list holds
//! them for the length of a run. They are read when the replace runs and never leave this module:
//! no result of this module holds an argument.

use serde_json::{Map, Value};

use super::{
    Applied, add_head, add_json_args, batch_safe, is_cli_name, remove_args, replace_with, string_args, transport_of,
};
use crate::ecosystem::mcp_merge::{Shape, replace_managed};
use crate::ecosystem::setup::McpFile;
use crate::harness::HarnessCli;

/// How a replace ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CliReplace {
    /// The old entry is gone and the new one added.
    Replaced,
    /// A run failed; the file is as it was, or the old entry was put back.
    Failed,
    /// The entry was removed and neither the new nor the old one could be added.
    RemovedEntry,
    /// A batch-shim binary cannot take one of the arguments; nothing ran.
    BatchShim,
    /// The CLI cannot write the old or the new entry; nothing ran.
    CannotWrite,
}

/// Replaces the server `name` of `file`, whose entry now is `existing`, by `fresh` (the definition
/// a new add writes, see `mcp_definition`). With `keep` (the same HTTP or SSE endpoint) the managed
/// keys of `fresh` are set in `existing` and the rest of it is kept, otherwise `fresh` is the new
/// entry.
pub(crate) async fn replace_entry(
    cli: HarnessCli<'_>,
    file: &McpFile,
    name: &str,
    existing: &Value,
    fresh: &Value,
    keep: bool,
) -> CliReplace {
    let shape = file.shape;
    let new = if keep {
        swapped(existing, fresh, shape)
    } else {
        fresh.clone()
    };
    // The old entry is put back when the add fails (`None`: the CLI cannot write it).
    let add_old = replace_add_args(shape, name, existing);
    let (Some(remove), Some(add_new)) = (remove_args(shape, name), replace_add_args(shape, name, &new)) else {
        return CliReplace::CannotWrite;
    };

    let runs = [Some(&remove), Some(&add_new), add_old.as_ref()];
    if cli.binary.is_batch_script && !runs.into_iter().flatten().all(|args| batch_safe(args)) {
        return CliReplace::BatchShim;
    }

    match replace_with(cli, file, name, Some(add_new), add_old).await {
        Applied::All => CliReplace::Replaced,
        Applied::Failed => CliReplace::Failed,
        Applied::RemovedEntry => CliReplace::RemovedEntry,
    }
}

/// True when a Codex entry `existing` can be replaced: only what `codex mcp add` carries over.
pub(crate) fn codex_replaceable(name: &str, existing: &Value) -> bool {
    replace_add_args(Shape::CodexCli, name, existing).is_some()
}

/// `existing` with the managed keys of `bundle` (a same-endpoint replace: the URL).
fn swapped(existing: &Value, bundle: &Value, shape: Shape) -> Value {
    let mut entry = existing.clone();
    replace_managed(&mut entry, bundle, shape);

    entry
}

/// The arguments that add `entry` as it is: Claude Code's `add-json`, or Codex's `mcp add` with
/// `--env` for its `env`. `None` when the name would read as an option or Codex cannot take the
/// entry.
fn replace_add_args(shape: Shape, name: &str, entry: &Value) -> Option<Vec<String>> {
    if !is_cli_name(name) {
        return None;
    }

    match shape {
        Shape::ClaudeCli => Some(add_json_args(name, entry)),
        Shape::CodexCli => codex_args(name, entry.as_object()?),
        Shape::McpServers | Shape::OpenCode | Shape::OpenCodeV2 => None,
    }
}

fn codex_args(name: &str, fields: &Map<String, Value>) -> Option<Vec<String>> {
    let mut args = add_head(name);

    match transport_of(fields) {
        (Some(command), None) => {
            let known = fields
                .keys()
                .all(|key| matches!(key.as_str(), "command" | "args" | "env"));
            let words = string_args(fields)?;

            if !known {
                return None;
            }

            match fields.get("env") {
                None => {}
                Some(Value::Object(env)) => {
                    for (key, value) in env {
                        if key.is_empty() || key.contains('=') || key.starts_with('-') {
                            return None;
                        }

                        args.extend(["--env".to_owned(), format!("{key}={}", value.as_str()?)]);
                    }
                }
                Some(_) => return None,
            }

            args.push("--".to_owned());
            args.push(command.to_owned());
            args.extend(words);
        }
        (None, Some(url)) => {
            if !fields
                .keys()
                .all(|key| matches!(key.as_str(), "url" | "bearer_token_env_var"))
            {
                return None;
            }

            args.extend(["--url".to_owned(), url.to_owned()]);

            match fields.get("bearer_token_env_var") {
                None => {}
                Some(Value::String(variable)) => {
                    args.extend(["--bearer-token-env-var".to_owned(), variable.clone()]);
                }
                Some(_) => return None,
            }
        }
        _ => return None,
    }

    Some(args)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn words(args: &[&str]) -> Option<Vec<String>> {
        Some(args.iter().map(|word| (*word).to_owned()).collect())
    }

    #[test]
    fn codex_carries_env_and_a_bearer_variable_only() {
        assert_eq!(
            replace_add_args(
                Shape::CodexCli,
                "s",
                &json!({ "command": "c", "args": ["a"], "env": { "K": "v" } })
            ),
            words(&["mcp", "add", "s", "--env", "K=v", "--", "c", "a"])
        );
        assert_eq!(
            replace_add_args(
                Shape::CodexCli,
                "s",
                &json!({ "url": "https://x.example", "bearer_token_env_var": "T" })
            ),
            words(&[
                "mcp",
                "add",
                "s",
                "--url",
                "https://x.example",
                "--bearer-token-env-var",
                "T"
            ])
        );
    }

    #[test]
    fn codex_refuses_what_its_add_cannot_carry() {
        for entry in [
            json!({ "command": "c", "enabled": false }),
            json!({ "command": "c", "env_vars": ["A"] }),
            json!({ "command": "c", "env": { "K": 1 } }),
            json!({ "command": "c", "env": { "K=": "v" } }),
            json!({ "url": "https://x.example", "env": { "K": "v" } }),
            json!({ "url": "https://x.example", "http_headers": {} }),
            json!({ "command": "c", "url": "https://x.example" }),
        ] {
            assert!(!codex_replaceable("s", &entry), "{entry}");
        }
    }

    #[test]
    fn claude_keeps_every_other_key() {
        let old = json!({ "type": "stdio", "command": "c", "args": [], "env": { "K": "v" }, "oauth": { "x": 1 } });
        let new = swapped(
            &old,
            &json!({ "type": "http", "url": "https://x.example" }),
            Shape::ClaudeCli,
        );

        assert_eq!(
            new,
            json!({ "env": { "K": "v" }, "oauth": { "x": 1 }, "type": "http", "url": "https://x.example" })
        );
    }
}
