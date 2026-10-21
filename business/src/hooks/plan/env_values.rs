//! The `HARNESS_HOOK_*` values that come from the agent (tool name, touched paths, project root),
//! kept only when they are plain enough to hand to a shell.

use std::ops::RangeInclusive;

use super::MAX_TOUCHED_BYTES;

/// Longest `HARNESS_HOOK_TOOL` value, in characters (the matcher's limit).
const MAX_TOOL_CHARS: usize = 200;

/// Characters `cmd.exe` treats specially, never in `HARNESS_HOOK_TOUCHED` or
/// `HARNESS_HOOK_PROJECT` on any platform.
const SHELL_SPECIALS: &[char] = &['&', '|', '<', '>', '^', '%', '!', '"', '(', ')'];

/// Characters Windows may turn into one of [`SHELL_SPECIALS`], `/` or `\` when it converts the
/// environment to a legacy code page ("best fit"), so a path that looked plain would not be once
/// converted. Taken from Microsoft's best-fit tables (`WindowsBestFit/bestfit1252.txt`,
/// `bestfit932.txt`, `bestfit936.txt` and the other `bestfit*.txt` on unicode.org, as far as they
/// could be checked) and widened with look-alikes of the same characters whose mapping could not
/// be confirmed, since refusing a rare character only costs one path. Not exhaustive: another code
/// page may map a character missing here, so the variables stay untrusted input (see the format
/// doc). Windows does not run a round trip through `WideCharToMultiByte` instead: the workspace
/// forbids `unsafe` code, and every binding of it is `unsafe`.
const BEST_FIT_SPECIALS: &[RangeInclusive<char>] = &[
    // `"`
    '\u{02BA}'..='\u{02BA}',
    '\u{02DD}'..='\u{02DD}',
    '\u{02EE}'..='\u{02EE}',
    '\u{030B}'..='\u{030B}',
    '\u{030E}'..='\u{030E}',
    '\u{05F4}'..='\u{05F4}',
    '\u{201F}'..='\u{201F}',
    '\u{2033}'..='\u{2033}',
    '\u{2036}'..='\u{2036}',
    '\u{3003}'..='\u{3003}',
    '\u{301D}'..='\u{301F}',
    // `|`
    '\u{00A6}'..='\u{00A6}',
    '\u{01C0}'..='\u{01C0}',
    '\u{05C0}'..='\u{05C0}',
    '\u{2016}'..='\u{2016}',
    '\u{2223}'..='\u{2223}',
    '\u{2225}'..='\u{2225}',
    '\u{23D0}'..='\u{23D0}',
    '\u{2502}'..='\u{2502}',
    '\u{2758}'..='\u{2758}',
    // `<` and `>`
    '\u{00AB}'..='\u{00AB}',
    '\u{00BB}'..='\u{00BB}',
    '\u{02C2}'..='\u{02C3}',
    '\u{2039}'..='\u{203A}',
    '\u{2190}'..='\u{2190}',
    '\u{2192}'..='\u{2192}',
    '\u{2329}'..='\u{232A}',
    '\u{276E}'..='\u{276F}',
    '\u{27E8}'..='\u{27E9}',
    '\u{3008}'..='\u{3009}',
    // `^`
    '\u{02C4}'..='\u{02C4}',
    '\u{02C6}'..='\u{02C6}',
    '\u{0302}'..='\u{0302}',
    '\u{2038}'..='\u{2038}',
    '\u{2191}'..='\u{2191}',
    '\u{2303}'..='\u{2303}',
    // `%` and `!`
    '\u{066A}'..='\u{066A}',
    '\u{2052}'..='\u{2052}',
    '\u{00A1}'..='\u{00A1}',
    '\u{01C3}'..='\u{01C3}',
    '\u{203C}'..='\u{203C}',
    // `(` and `)`
    '\u{207D}'..='\u{207E}',
    '\u{208D}'..='\u{208E}',
    '\u{2768}'..='\u{276B}',
    '\u{2E28}'..='\u{2E29}',
    // `/` and `\` (the yen and won signs are `\` in the Japanese and Korean code pages)
    '\u{00A5}'..='\u{00A5}',
    '\u{20A9}'..='\u{20A9}',
    '\u{2044}'..='\u{2044}',
    '\u{2215}'..='\u{2216}',
    '\u{2571}'..='\u{2572}',
    '\u{27CB}'..='\u{27CB}',
    '\u{27CD}'..='\u{27CD}',
    '\u{29F5}'..='\u{29F5}',
    '\u{29F8}'..='\u{29F9}',
    // Whole blocks of ASCII look-alikes: the vertical forms, the CJK compatibility forms with the
    // small form variants (small `& % ! < > ( ) \`), the fullwidth and halfwidth forms.
    '\u{FE10}'..='\u{FE1F}',
    '\u{FE30}'..='\u{FE6F}',
    '\u{FF00}'..='\u{FFEF}',
];

/// Characters best fit may turn into `-` (see [`BEST_FIT_SPECIALS`]): hyphens, dashes, minus
/// signs and the soft hyphen. Refused at the start of a path only, where `-` reads as an option.
const BEST_FIT_DASHES: &[RangeInclusive<char>] = &[
    '\u{00AD}'..='\u{00AD}',
    '\u{02D7}'..='\u{02D7}',
    '\u{2010}'..='\u{2015}',
    '\u{2043}'..='\u{2043}',
    '\u{207B}'..='\u{207B}',
    '\u{208B}'..='\u{208B}',
    '\u{2212}'..='\u{2212}',
    '\u{2500}'..='\u{2500}',
    '\u{FE58}'..='\u{FE58}',
    '\u{FE63}'..='\u{FE63}',
    '\u{FF0D}'..='\u{FF0D}',
];

fn is_in(table: &[RangeInclusive<char>], character: char) -> bool {
    table.iter().any(|range| range.contains(&character))
}

/// True when `path` begins with `-` or a character best fit may turn into one.
fn starts_like_option(path: &str) -> bool {
    path.chars()
        .next()
        .is_some_and(|first| first == '-' || is_in(BEST_FIT_DASHES, first))
}

/// The tool name an agent reported, when it is 1 to [`MAX_TOOL_CHARS`] of `[A-Za-z0-9_.:-]`;
/// empty otherwise. The agent names its tools, so the name is untrusted input to a shell.
pub(super) fn tool_value(name: &str) -> String {
    let allowed = |character: char| character.is_ascii_alphanumeric() || "_.:-".contains(character);
    let valid = !name.is_empty() && name.chars().count() <= MAX_TOOL_CHARS && name.chars().all(allowed);

    if valid { name.to_owned() } else { String::new() }
}

/// True for a path `HARNESS_HOOK_TOUCHED` or `HARNESS_HOOK_PROJECT` may carry: no control
/// character (a newline would split it), no `cmd.exe` special character (nor one "best fit" could
/// turn into one, see [`BEST_FIT_SPECIALS`]) and no leading `-` (it would read as an option), nor
/// one best fit could turn into `-` ([`BEST_FIT_DASHES`]).
fn is_plain_path(path: &str) -> bool {
    let special = |character: char| {
        character.is_control() || SHELL_SPECIALS.contains(&character) || is_in(BEST_FIT_SPECIALS, character)
    };

    !path.is_empty() && !starts_like_option(path) && !path.chars().any(special)
}

/// The chat's project root when it is a plain path ([`is_plain_path`]), empty otherwise: the root
/// is a folder name the agent may have chosen, so it is untrusted input to a shell too.
pub(super) fn project_value(project: Option<&str>) -> String {
    match project {
        Some(project) if is_plain_path(project) => project.to_owned(),
        _ => String::new(),
    }
}

/// `paths` joined by newlines, within [`MAX_TOUCHED_BYTES`]. A path that is not plain
/// ([`is_plain_path`]) is left out: the agent reports these paths, so they are untrusted. So is a
/// path longer than the limit on its own, and the paths after it still count.
pub(super) fn touched_value(paths: &[String]) -> String {
    let mut value = String::new();
    let fits_alone = |path: &&String| path.len() <= MAX_TOUCHED_BYTES;

    for path in paths.iter().filter(fits_alone).filter(|path| is_plain_path(path)) {
        let extra = path.len() + usize::from(!value.is_empty());

        if value.len() + extra > MAX_TOUCHED_BYTES {
            break;
        }

        if !value.is_empty() {
            value.push('\n');
        }

        value.push_str(path);
    }

    value
}
