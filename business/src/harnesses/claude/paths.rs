//! Claude Code directory resolution (Claude Code 2.1.304).
//!
//! - Config, data and sessions: `$CLAUDE_CONFIG_DIR`, else `~/.claude`, on every OS. A relative
//!   `CLAUDE_CONFIG_DIR` is ignored (Claude would resolve it against its working directory).
//! - Global file (user MCP servers, per-project state): `$CLAUDE_CONFIG_DIR/.claude.json` when the
//!   variable is set, else `~/.claude.json`, next to the config directory and not inside it.
//! - Sessions of a project: `<config>/projects/<slug>`, where the slug is the project's absolute
//!   path with every character that is not an ASCII letter or digit replaced by `-`, cut to
//!   [`MAX_SLUG_LEN`] characters plus a hash of the path when longer (see [`project_slug`]).
//! - Project layer: `<project>/.claude`.

use std::path::{Path, PathBuf};

use crate::env::Env;
use crate::error::Result;
use crate::harness::HarnessPaths;
use crate::security::SafeReader;
use crate::security::deny::{CLAUDE_GLOBAL_FILE, add_claude_global_file};

/// The default config directory name below the home directory, and the project layer's name.
pub const PROJECT_DIR_NAME: &str = ".claude";

/// The global file's name.
const GLOBAL_FILE_NAME: &str = CLAUDE_GLOBAL_FILE;

/// Largest global file read. Claude Code keeps every project's state in it, so it grows well past
/// the 2 MiB of other config files on a long-used machine.
pub const MAX_GLOBAL_FILE_BYTES: u64 = 32 * 1024 * 1024;

/// Longest slug kept whole; a longer one is cut to this length and gets a hash suffix.
pub const MAX_SLUG_LEN: usize = 200;

/// Resolves every Claude Code directory for `env` (see the module docs). Claude has no profiles;
/// settings, plugins, skills, agents and sessions all live in the config directory.
pub fn paths(env: &Env, project: Option<&Path>) -> HarnessPaths {
    let config = config_dir(env);

    HarnessPaths {
        config_root: config.clone(),
        agent_dir: config.clone(),
        data_root: config,
        profile: None,
        project_dir: project.map(|project| project.join(PROJECT_DIR_NAME)),
    }
}

/// The config directory: the absolute `CLAUDE_CONFIG_DIR`, else `~/.claude`.
pub fn config_dir(env: &Env) -> PathBuf {
    custom_config_dir(env).unwrap_or_else(|| env.home_join(PROJECT_DIR_NAME))
}

/// The global file: `.claude.json` inside the absolute `CLAUDE_CONFIG_DIR`, else `~/.claude.json`.
/// Its name is on the deny-list; the path is registered as the one file
/// [`SafeReader::read_claude_global_file`] opens (see [`read_global_file`]).
pub fn global_config_file(env: &Env) -> PathBuf {
    let path = match custom_config_dir(env) {
        Some(dir) => dir.join(GLOBAL_FILE_NAME),
        None => env.home_join(GLOBAL_FILE_NAME),
    };

    add_claude_global_file(&path);
    path
}

/// The text of the global file at `path` (a [`global_config_file`]), read through
/// [`SafeReader::read_claude_global_file`] with a [`MAX_GLOBAL_FILE_BYTES`] cap. Only the readers
/// that keep to an allow-list of its keys call this: the config and extensions views, the MCP
/// states and changes of Recommended setup, and Share setup's export.
pub(crate) fn read_global_file(path: &Path) -> Result<String> {
    SafeReader::read_claude_global_file(path, MAX_GLOBAL_FILE_BYTES)
}

/// The folder holding the session files of `project`: `<config>/projects/<slug>`.
pub fn sessions_dir(env: &Env, project: &Path) -> PathBuf {
    config_dir(env).join("projects").join(project_slug(project))
}

/// `project` as Claude names its session folder, as Claude Code 2.1.304 does: every UTF-16 code
/// unit that is not an ASCII letter or digit becomes `-` (`replace(/[^a-zA-Z0-9]/g, "-")`). A slug
/// longer than [`MAX_SLUG_LEN`] is cut to that length and followed by `-` and the base-36
/// absolute value of the path's 32-bit string hash (`(h << 5) - h + unit`, wrapping, over the
/// path's UTF-16 code units).
pub fn project_slug(project: &Path) -> String {
    let path = project.to_string_lossy();
    let slug: String = path
        .encode_utf16()
        .map(|unit| match char::from_u32(u32::from(unit)) {
            Some(character) if character.is_ascii_alphanumeric() => character,
            _ => '-',
        })
        .collect();

    // Every character of the slug is ASCII, so its length counts UTF-16 code units as Claude does.
    if slug.len() <= MAX_SLUG_LEN {
        return slug;
    }

    format!(
        "{}-{}",
        &slug[..MAX_SLUG_LEN],
        base36(string_hash(&path).unsigned_abs())
    )
}

/// The 32-bit string hash Claude names long session folders with (Java's `String.hashCode`).
fn string_hash(text: &str) -> i32 {
    text.encode_utf16().fold(0_i32, |hash, unit| {
        hash.wrapping_shl(5).wrapping_sub(hash).wrapping_add(i32::from(unit))
    })
}

/// `number` in base 36 with lowercase digits, as JavaScript's `toString(36)` writes it.
fn base36(mut number: u32) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";

    let mut digits = Vec::new();

    loop {
        digits.push(DIGITS[(number % 36) as usize]);
        number /= 36;

        if number == 0 {
            break;
        }
    }

    digits.reverse();
    String::from_utf8(digits).unwrap_or_default()
}

/// The directory `CLAUDE_CONFIG_DIR` names, when absolute.
fn custom_config_dir(env: &Env) -> Option<PathBuf> {
    env.var("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_slug_replaces_every_character_but_ascii_letters_and_digits() {
        assert_eq!(project_slug(Path::new("/home/u/.config/x")), "-home-u--config-x");
        assert_eq!(project_slug(Path::new("/w/Zażółć_1")), "-w-Za-----1");
        assert_eq!(project_slug(Path::new("/w/a😀b")), "-w-a--b");
    }

    /// Reference values from Claude's own JavaScript rule, run in Bun.
    #[test]
    fn a_slug_over_200_characters_is_cut_and_hashed() {
        let exact = format!("/{}", "x".repeat(199));
        assert_eq!(project_slug(Path::new(&exact)), exact.replace('/', "-"));

        let over = format!("/{}", "x".repeat(200));
        assert_eq!(project_slug(Path::new(&over)), format!("-{}-d0i18x", "x".repeat(199)));

        let long = format!("/home/u/{}/Zażółć 😀/{}", "a".repeat(150), "b".repeat(60));
        assert_eq!(
            project_slug(Path::new(&long)),
            format!("-home-u-{}-Za--------{}-vnfog3", "a".repeat(150), "b".repeat(31))
        );
    }

    #[test]
    fn base36_matches_javascript() {
        assert_eq!(base36(0), "0");
        assert_eq!(base36(35), "z");
        assert_eq!(base36(36), "10");
        assert_eq!(base36(2_147_483_648), "zik0zk");
    }
}
