//! Field validators for shared items, used on both sides: export leaves out what import would
//! refuse, and import marks it `invalid` (a refused value is never echoed back).
//!
//! Every CLI target also passes [`cli_target`]; the checks here are stricter and only apply to
//! shared items: no local paths (`..` segments, a leading `/` or `~`, `file:`, `link:`, a drive
//! letter, a backslash, a path embedded after `=`, `@` or a separator), no credentials, nothing the
//! redactor would hide (see [`super::secrets`]), and only the source kinds a bundle may carry.
//! Sources are classified in [`super::validate_source`], MCP fields in [`super::validate_mcp`];
//! both are re-exported here.

use std::sync::LazyLock;

use regex::Regex;

use super::schema::{MAX_TEXT_CHARS, Marketplace, Package, Plugin};
use super::secrets::exportable;
pub use super::validate_mcp::{
    is_shell, key_names, mcp_args, mcp_command, mcp_name, mcp_server, mcp_url, opencode_mcp_server,
};
use super::validate_source::{is_npm_registry_spec, split_npm_spec};
pub use super::validate_source::{marketplace_source, package_source};
use crate::harness::{TargetKind, cli_target};
use crate::model::SyncInvalidReason;

/// Outcome of a validator: the reason when the value is refused.
pub type Check = Result<(), SyncInvalidReason>;

pub(super) fn compile(pattern: &str) -> Regex {
    Regex::new(pattern).expect("static regex is valid")
}

/// MCP server and marketplace names.
static NAME: LazyLock<Regex> = LazyLock::new(|| compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$"));

/// Plugin ids: `name@marketplace`.
static MARKETPLACE_PLUGIN: LazyLock<Regex> =
    LazyLock::new(|| compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}@[A-Za-z0-9][A-Za-z0-9._-]{0,63}$"));

/// Plugin ids: an npm package name (`name` or `@scope/name`).
static NPM_NAME: LazyLock<Regex> =
    LazyLock::new(|| compile(r"^(?:@[a-z0-9][a-z0-9._-]{0,99}/)?[a-z0-9][a-z0-9._-]{0,99}$"));

/// Versions (see [`version`]).
static VERSION: LazyLock<Regex> =
    LazyLock::new(|| compile(r"^v?[0-9]{1,9}(?:\.[0-9]{1,9}){0,3}(?:[-+][0-9A-Za-z.+-]{1,64})?$"));

/// Dates (see [`date`]): year, month and day.
static DATE: LazyLock<Regex> = LazyLock::new(|| compile(r"^([0-9]{4})-([0-9]{2})-([0-9]{2})$"));

/// Harness section and item-kind keys that may be shown (see [`section_label`]).
static SECTION_KEY: LazyLock<Regex> = LazyLock::new(|| compile(r"^[A-Za-z0-9_-]{1,32}$"));

// ---------------------------------------------------------------------------------------------
// Generic checks
// ---------------------------------------------------------------------------------------------

/// True when `value` has no control characters, no invisible format characters (Unicode
/// categories Cf, Zl and Zp: bidirectional overrides, isolates and marks, zero-width characters,
/// the soft hyphen, the byte-order mark, line/paragraph separators, tag characters) and no other
/// character that renders as nothing (fillers, blanks, variation selectors), so it renders as what
/// it is.
pub fn display_safe(value: &str) -> bool {
    !value
        .chars()
        .any(|character| character.is_control() || is_format_character(character))
}

/// Unicode format characters (category Cf), the line and paragraph separators (Zl, Zp), and the
/// invisible characters outside them: the combining grapheme joiner (U+034F), the Hangul fillers
/// (U+115F, U+1160, U+3164, U+FFA0), the Khmer inherent vowels (U+17B4, U+17B5), the Mongolian
/// variation selectors (U+180B–U+180F), the blank Braille pattern (U+2800) and the variation
/// selectors (U+FE00–U+FE0F, U+E0100–U+E01EF).
const FORMAT_CHARACTERS: &[(char, char)] = &[
    ('\u{00AD}', '\u{00AD}'),
    ('\u{034F}', '\u{034F}'),
    ('\u{0600}', '\u{0605}'),
    ('\u{061C}', '\u{061C}'),
    ('\u{06DD}', '\u{06DD}'),
    ('\u{070F}', '\u{070F}'),
    ('\u{0890}', '\u{0891}'),
    ('\u{08E2}', '\u{08E2}'),
    ('\u{115F}', '\u{1160}'),
    ('\u{17B4}', '\u{17B5}'),
    ('\u{180B}', '\u{180F}'),
    ('\u{200B}', '\u{200F}'),
    ('\u{2028}', '\u{202E}'),
    ('\u{2060}', '\u{2064}'),
    ('\u{2066}', '\u{206F}'),
    ('\u{2800}', '\u{2800}'),
    ('\u{3164}', '\u{3164}'),
    ('\u{FE00}', '\u{FE0F}'),
    ('\u{FEFF}', '\u{FEFF}'),
    ('\u{FFA0}', '\u{FFA0}'),
    ('\u{FFF9}', '\u{FFFB}'),
    ('\u{110BD}', '\u{110BD}'),
    ('\u{110CD}', '\u{110CD}'),
    ('\u{13430}', '\u{1343F}'),
    ('\u{1BCA0}', '\u{1BCA3}'),
    ('\u{1D173}', '\u{1D17A}'),
    ('\u{E0001}', '\u{E0001}'),
    ('\u{E0020}', '\u{E007F}'),
    ('\u{E0100}', '\u{E01EF}'),
];

fn is_format_character(character: char) -> bool {
    FORMAT_CHARACTERS
        .iter()
        .any(|(first, last)| (*first..=*last).contains(&character))
}

/// A displayed text field: at most `max` characters and [`display_safe`].
pub fn text(value: &str, max: usize) -> Check {
    if value.chars().count() > max {
        return Err(SyncInvalidReason::TooLong);
    }

    if !display_safe(value) {
        return Err(SyncInvalidReason::UnsafeCharacters);
    }

    Ok(())
}

/// The key of a harness section or an item kind as it may be shown: itself when it is
/// `^[A-Za-z0-9_-]{1,32}$`, otherwise `"unknown"`.
pub fn section_label(key: &str) -> String {
    if SECTION_KEY.is_match(key) {
        key.to_owned()
    } else {
        "unknown".to_owned()
    }
}

/// True when `value` points to a local path: a `..` segment, a leading `/` or `~`, a `file:` or
/// `link:` spec (also after `npm:`), a drive letter (`C:`) or a backslash, or an embedded path
/// (see [`EMBEDDED_PATH`]).
pub fn is_local_path(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let spec = lower.strip_prefix("npm:").unwrap_or(&lower);
    let bytes = value.as_bytes();
    let drive_letter = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';

    value.starts_with('/')
        || value.starts_with('~')
        || value.contains('\\')
        || spec.starts_with("file:")
        || spec.starts_with("link:")
        || (drive_letter && (bytes.len() == 2 || matches!(bytes[2], b'/' | b'\\')))
        || value.split(['/', ':']).any(|segment| segment == "..")
        || EMBEDDED_PATH.is_match(value)
}

/// A path inside a value: an absolute, home (`~/`, `~user`) or drive path right after `=`
/// (`--root=/x`, `--config=~/x`, `--dir=C:/x`; not a semver `=~1.2`); an absolute path right after
/// `@` (`@/etc/x`; an npm scope `@scope/x` does not match); or a user/system root after `,` or `;`
/// and any number of slashes, or after `:` and one or three or more slashes
/// (`PATH=/usr/bin:/home/alice/bin`, `sqlite:///home/alice/x.db`). Two slashes after `:` start a
/// URL host, so `https://home.example.com/x` and a URL path (`https://host/home/x`) do not match.
static EMBEDDED_PATH: LazyLock<Regex> = LazyLock::new(|| {
    compile(
        r"(?i)=(?:/|~(?:[/\\a-z_]|$)|[a-z]:[/\\])|@/|(?:[,;]/+|:/(?:/{2,})?)(?:home|users|root|mnt|var|tmp|opt|srv|media|private)/",
    )
});

/// True when `value` names a user before its host: `scheme://user[:password]@host` (any user) or
/// `git:user@host` (except the conventional SSH user `git@`).
pub fn has_credentials(value: &str) -> bool {
    let authority = |rest: &str| rest.split(['/', '?', '#']).next().unwrap_or_default().to_owned();

    if let Some((_, rest)) = value.split_once("://") {
        return authority(rest).contains('@');
    }

    if let Some(rest) = value.strip_prefix("git:") {
        return match authority(rest).rsplit_once('@') {
            Some((user, _)) => user != "git",
            None => false,
        };
    }

    false
}

/// A value passed to a harness CLI: [`text`], no local path, no credentials, [`cli_target`] and
/// [`exportable`].
pub(super) fn target(kind: TargetKind, value: &str) -> Check {
    text(value, MAX_TEXT_CHARS)?;

    if is_local_path(value) {
        return Err(SyncInvalidReason::LocalPath);
    }

    if has_credentials(value) {
        return Err(SyncInvalidReason::Credentials);
    }

    if cli_target(kind, value).is_err() {
        return Err(SyncInvalidReason::UnsafeValue);
    }

    if !exportable(value) {
        return Err(SyncInvalidReason::ContainsSecret);
    }

    Ok(())
}

/// An optional version (a plugin's or package's, the app's): semver-like, such as `0.15.0`,
/// `v2.1` or `1.0.0-beta.1+build.5`.
pub fn version(value: Option<&str>) -> Check {
    let Some(value) = value else {
        return Ok(());
    };

    text(value, MAX_TEXT_CHARS)?;

    if !VERSION.is_match(value) {
        return Err(SyncInvalidReason::Malformed);
    }

    if !exportable(value) {
        return Err(SyncInvalidReason::ContainsSecret);
    }

    Ok(())
}

/// A date: `YYYY-MM-DD` with a month of 01–12 and a day of 01–31.
pub fn date(value: &str) -> Check {
    let Some(parts) = DATE.captures(value) else {
        return Err(SyncInvalidReason::Malformed);
    };

    let month: u32 = parts[2].parse().unwrap_or_default();
    let day: u32 = parts[3].parse().unwrap_or_default();

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(SyncInvalidReason::Malformed);
    }

    Ok(())
}

/// A marketplace or MCP server name: `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$`, [`display_safe`] and
/// nothing secret.
pub fn name(value: &str) -> Check {
    text(value, MAX_TEXT_CHARS)?;

    if !NAME.is_match(value) {
        return Err(SyncInvalidReason::BadName);
    }

    if !exportable(value) {
        return Err(SyncInvalidReason::ContainsSecret);
    }

    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Marketplaces, plugins, packages
// ---------------------------------------------------------------------------------------------

/// A marketplace name (see [`name`]).
pub fn marketplace_name(value: &str) -> Check {
    name(value)
}

/// A plugin id: `name@marketplace` or an npm package name.
pub fn plugin_id(id: &str) -> Check {
    target(TargetKind::Plugin, id)?;

    if MARKETPLACE_PLUGIN.is_match(id) || NPM_NAME.is_match(id) {
        return Ok(());
    }

    Err(SyncInvalidReason::BadName)
}

/// A whole shared marketplace.
pub fn marketplace(item: &Marketplace) -> Check {
    marketplace_name(&item.name)?;
    marketplace_source(&item.source)
}

/// A whole shared plugin. A `marketplace` is only allowed on a `name@marketplace` id, and must
/// match its `@marketplace` part.
pub fn plugin(item: &Plugin) -> Check {
    plugin_id(&item.id)?;
    version(item.version.as_deref())?;

    let Some(marketplace) = item.marketplace.as_deref() else {
        return Ok(());
    };

    marketplace_name(marketplace)?;

    match item.id.rsplit_once('@') {
        Some((_, from_id)) if !item.id.starts_with('@') && from_id == marketplace => Ok(()),
        _ => Err(SyncInvalidReason::Malformed),
    }
}

/// A whole shared OpenCode plugin: an npm package name (`name` or `@scope/name`, never
/// `name@marketplace`) installed by `opencode plugin <id>[@<version>] -g`, so `version` is what gets
/// installed: a version, an x-range or a dist-tag npm reads from the registry (`1.2.3`, `1.x`,
/// `latest`). No marketplace.
pub fn opencode_plugin(item: &Plugin) -> Check {
    target(TargetKind::NpmModule, &item.id)?;

    let (_, wanted) = split_npm_spec(&item.id);
    if wanted.is_some() || !is_npm_registry_spec(&item.id) {
        return Err(SyncInvalidReason::BadName);
    }

    if item.marketplace.is_some() {
        return Err(SyncInvalidReason::Malformed);
    }

    let Some(version) = item.version.as_deref() else {
        return Ok(());
    };

    text(version, MAX_TEXT_CHARS)?;

    if !exportable(version) {
        return Err(SyncInvalidReason::ContainsSecret);
    }

    let spec = format!("{}@{version}", item.id);
    if !is_npm_registry_spec(&spec) || target(TargetKind::NpmModule, &spec).is_err() {
        return Err(SyncInvalidReason::Malformed);
    }

    Ok(())
}

/// A whole shared package.
pub fn package(item: &Package) -> Check {
    package_source(&item.source)?;
    version(item.version.as_deref())
}
