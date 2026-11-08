//! The options of `push` and `discard-local` of the stand-in, and the selection `push --selection -`
//! reads from stdin. A child module of `memory_push`, whose entries and routing it shares.

use std::io::Read;

use serde_json::Value;

use super::{Pushed, usage};

/// The options of `push` and `discard-local`.
#[derive(Default)]
pub struct Options {
    pub flags: Vec<String>,
    /// `--all`, `-` or ids (`discard-local`).
    pub which: Option<String>,
    pub overwrite: Option<String>,
    /// `--selection -`.
    pub selection: Option<String>,
    pub scope: Option<String>,
}

/// One entry of `only`: the entry as the review showed it.
pub struct Reviewed {
    pub id: i64,
    /// The note's title or the script's name.
    pub name: String,
    /// The `localUpdatedAt` the dry run printed.
    pub updated_at: String,
}

/// What `--selection -` reads from stdin.
pub struct Selection {
    pub only: Vec<Reviewed>,
    pub overwrite: Vec<i64>,
}

impl Selection {
    /// `{"only": [{"id", "name", "updatedAt"}, ...], "overwrite": [ids]}` from stdin; a usage error
    /// for anything else, an `only` of bare ids included.
    pub fn read() -> Self {
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text).unwrap_or_else(|_| usage());

        let value: Value = serde_json::from_str(&text).unwrap_or_else(|_| usage());
        let list = |field: &str| -> Vec<Value> { value[field].as_array().unwrap_or_else(|| usage()).clone() };
        let text_of =
            |entry: &Value, field: &str| -> String { entry[field].as_str().unwrap_or_else(|| usage()).to_owned() };

        Self {
            only: list("only")
                .iter()
                .map(|entry| Reviewed {
                    id: entry["id"].as_i64().unwrap_or_else(|| usage()),
                    name: text_of(entry, "name"),
                    updated_at: text_of(entry, "updatedAt"),
                })
                .collect(),
            overwrite: list("overwrite")
                .iter()
                .map(|id| id.as_i64().unwrap_or_else(|| usage()))
                .collect(),
        }
    }
}

impl Options {
    /// The options in `args`; `flags` are those that take no value.
    pub fn parse(args: &[String], flags: &[&str]) -> Self {
        let mut options = Self::default();
        let mut rest = args.iter();

        while let Some(arg) = rest.next() {
            let mut value = || rest.next().cloned().unwrap_or_else(|| usage());

            match arg.as_str() {
                "--overwrite" => options.overwrite = Some(value()),
                "--selection" => options.selection = Some(value()),
                "--scope" => options.scope = Some(value()),
                flag if flags.contains(&flag) => options.flags.push(flag.to_owned()),
                which if options.which.is_none() && (which == "--all" || !which.starts_with("--")) => {
                    options.which = Some(which.to_owned());
                }
                _ => usage(),
            }
        }

        options
    }

    pub fn in_scope(&self, scope: &str) -> bool {
        self.scope.as_deref().is_none_or(|wanted| wanted == scope)
    }

    /// Whether the push of `pushed` takes its entries of `scope`: the one named, else every scope
    /// routed to the team database alone.
    pub fn pushed(&self, pushed: &Pushed, scope: &str) -> bool {
        let routed = pushed.routed(scope);

        if self.flags.iter().any(|flag| flag == "--remote-local") {
            return routed.as_deref() == Some("remote+local");
        }

        match self.scope.as_deref() {
            Some(wanted) => wanted == scope,
            None => !matches!(routed.as_deref(), Some("local" | "remote+local")),
        }
    }
}
