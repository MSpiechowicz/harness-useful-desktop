//! The chat's own "Always allow" answers, for asks that bring no rule to echo back (every ask the
//! Always ask hook makes, and asks without `permission_suggestions`). Nothing is written anywhere:
//! the list lasts as long as the chat's process, and later asks it covers are allowed without a
//! dialog.
//!
//! - A tool is kept by its name ([`AllowKey`]). `Bash` also keeps its command: a simple command
//!   (no shell operators, substitutions or redirections) by its leading words, the program and a
//!   subcommand (`git status`, `cargo test`, `ls`), anything else by its exact text.
//! - A later `Bash` call is covered when it is a simple command starting with those words, or
//!   exactly the kept command.

use serde_json::Value;

use crate::claude_stream::mapper::PermissionAsk;
use crate::claude_stream::tools::BASH;
use crate::security::redact_text;

/// Characters that make a shell command more than one program with its arguments.
const SHELL_SYNTAX: [char; 12] = [';', '&', '|', '<', '>', '`', '$', '(', ')', '\n', '\r', '\\'];

/// What one "Always allow" answer allows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowKey {
    tool: String,
    command: Option<CommandKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CommandKey {
    /// Simple commands starting with these words.
    Prefix(Vec<String>),
    /// Exactly this command.
    Exact(String),
}

impl AllowKey {
    /// The key an "Always allow" answer to `ask` keeps.
    pub fn of(ask: &PermissionAsk) -> Self {
        let command = (ask.tool == BASH).then(|| {
            let text = command(&ask.input);

            match simple_words(text) {
                Some(words) => CommandKey::Prefix(prefix(&words)),
                None => CommandKey::Exact(text.trim().to_owned()),
            }
        });

        Self {
            tool: ask.tool.clone(),
            command,
        }
    }

    /// True when the key allows `ask`.
    pub fn covers(&self, ask: &PermissionAsk) -> bool {
        if ask.tool != self.tool {
            return false;
        }

        let text = command(&ask.input);

        match &self.command {
            None => true,
            Some(CommandKey::Exact(kept)) => text.trim() == kept,
            Some(CommandKey::Prefix(kept)) => simple_words(text).is_some_and(|words| words.starts_with(kept)),
        }
    }

    /// What the key allows, for the dialog (`label` names the tool).
    pub fn describe(&self, label: &str) -> String {
        let allowed = match &self.command {
            None => format!("Allows every {label} call"),
            Some(CommandKey::Prefix(words)) => format!("Allows every \"{} …\" command", words.join(" ")),
            Some(CommandKey::Exact(_)) => "Allows this exact command again".to_owned(),
        };

        redact_text(&format!("{allowed} until this chat ends"))
    }
}

/// The chat's kept "Always allow" answers.
#[derive(Debug, Default)]
pub struct AllowList {
    keys: Vec<AllowKey>,
}

impl AllowList {
    /// Keeps `key`.
    pub fn remember(&mut self, key: AllowKey) {
        if !self.keys.contains(&key) {
            self.keys.push(key);
        }
    }

    /// True when a kept answer allows `ask`.
    pub fn covers(&self, ask: &PermissionAsk) -> bool {
        self.keys.iter().any(|key| key.covers(ask))
    }
}

/// The `command` of a `Bash` input (empty without one).
fn command(input: &Value) -> &str {
    input.get("command").and_then(Value::as_str).unwrap_or_default()
}

/// The words of a simple command; `None` for one with shell syntax, or none at all.
fn simple_words(command: &str) -> Option<Vec<String>> {
    if command.contains(SHELL_SYNTAX) {
        return None;
    }

    let words: Vec<String> = command.split_whitespace().map(str::to_owned).collect();

    (!words.is_empty()).then_some(words)
}

/// The program, with its subcommand when the second word is one (a plain lowercase word).
fn prefix(words: &[String]) -> Vec<String> {
    let subcommand = words.get(1).filter(|word| {
        word.chars()
            .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit() || "-_:".contains(character))
            && !word.starts_with('-')
    });

    words.iter().take(1).chain(subcommand).cloned().collect()
}
