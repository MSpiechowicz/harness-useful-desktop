//! The characters a listed name may hold.
//!
//! A picked path goes straight into a prompt, so a name may not hold a control character, a
//! replacement character (bytes that are no text), a `\`, or an invisible or bidirectional character
//! ([`is_hidden_char`]). Some of those are ordinary in real names, so they are allowed in the places
//! where they shape visible text, and only there:
//!
//! - U+200D (joiner): between two visible non-ASCII characters (`👨‍👩‍👧`), or after a U+FE0F that
//!   follows one (`❤️‍🔥`).
//! - U+200C (non-joiner): between two visible non-ASCII characters (Persian and Hindi words).
//! - U+FE0E and U+FE0F (text and emoji selectors): directly after an emoji (see [`is_emoji`]:
//!   `❤️`), and nowhere else. A keycap (`1️⃣`) is a digit, `#` or `*`, then U+FE0F, then U+20E3. The
//!   other variation selectors (U+FE00 to U+FE0D) carry data in text that no emoji needs.
//! - U+200E and U+200F (direction marks): next to a visible non-ASCII character on one side at least.
//!
//! "Visible" is any character that is no control, no replacement character and not hidden itself, so
//! two of these characters never touch (a run of them could carry a payload), except U+FE0F then
//! U+200D. A whole path holds at most [`MAX_FORMAT_CHARS`] of them, which bounds what is left of such a
//! channel to about 20 bytes. Every other hidden character stays refused: the bidirectional embeddings,
//! overrides and isolates, U+200B, the line and paragraph separators, U+FEFF, the soft hyphen, U+2060,
//! the tag characters, the other variation selectors, the Hangul fillers and the blank Braille pattern.

use crate::signed_docs::unicode::is_hidden_char;

const JOINER: char = '\u{200d}';
const NON_JOINER: char = '\u{200c}';
const LEFT_TO_RIGHT_MARK: char = '\u{200e}';
const RIGHT_TO_LEFT_MARK: char = '\u{200f}';
const TEXT_SELECTOR: char = '\u{fe0e}';
const EMOJI_SELECTOR: char = '\u{fe0f}';
const KEYCAP: char = '\u{20e3}';

/// The most characters of [`is_format`] a whole path may hold. Real names need a handful (a family
/// emoji has two joiners, a Persian name a few non-joiners).
const MAX_FORMAT_CHARS: usize = 16;

/// True when every character of `name` is allowed where it stands. `name` may be a whole path:
/// a `/` is no visible non-ASCII character, so a name never borrows one from its neighbour.
pub(super) fn is_clean(name: &str) -> bool {
    let mut characters = name.chars().peekable();
    let mut previous = None;
    let mut formats = 0;

    while let Some(character) = characters.next() {
        let next = characters.peek().copied();

        if is_format(character) {
            formats += 1;
        }

        if formats > MAX_FORMAT_CHARS || !is_allowed(character, previous, next) {
            return false;
        }

        previous = Some(character);
    }

    true
}

fn is_allowed(character: char, previous: Option<char>, next: Option<char>) -> bool {
    if character == '\\' || character.is_control() || character == char::REPLACEMENT_CHARACTER {
        return false;
    }

    if character.is_ascii() || !is_hidden_char(character) {
        return true;
    }

    match character {
        JOINER => {
            let after_text = is_visible(previous) || previous == Some(EMOJI_SELECTOR);

            after_text && is_visible(next)
        }
        NON_JOINER => is_visible(previous) && is_visible(next),
        TEXT_SELECTOR | EMOJI_SELECTOR => follows_emoji(character, previous, next),
        LEFT_TO_RIGHT_MARK | RIGHT_TO_LEFT_MARK => {
            let after_format = previous.is_some_and(is_format);

            !after_format && (is_visible(previous) || is_visible(next))
        }
        _ => false,
    }
}

/// True when selector `character` may follow `previous` (and precede `next`): an emoji, or for U+FE0F
/// a keycap's digit, `#` or `*` that U+20E3 follows at once.
fn follows_emoji(character: char, previous: Option<char>, next: Option<char>) -> bool {
    let Some(previous) = previous else {
        return false;
    };

    if character == EMOJI_SELECTOR && matches!(previous, '0'..='9' | '#' | '*') {
        return next == Some(KEYCAP);
    }

    is_emoji(previous)
}

/// True for the characters an emoji is written with, and that a selector may follow: the few symbols
/// below U+2190, the arrows, technical, shapes and dingbats blocks (U+2190 to U+2BFF), two CJK
/// symbols and the emoji planes.
fn is_emoji(character: char) -> bool {
    matches!(
        character,
        '\u{a9}' | '\u{ae}' | '\u{203c}' | '\u{2049}' | '\u{2122}' | '\u{2139}'
            | '\u{2190}'..='\u{2bff}'
            | '\u{3030}' | '\u{303d}' | '\u{3297}' | '\u{3299}'
            | '\u{1f000}'..='\u{1faff}'
    )
}

/// True for a visible character that is no ASCII one: what a shaping character may sit next to.
fn is_visible(character: Option<char>) -> bool {
    character.is_some_and(|character| {
        !character.is_ascii()
            && !character.is_control()
            && character != char::REPLACEMENT_CHARACTER
            && !is_hidden_char(character)
    })
}

/// True for one of the characters [`is_allowed`] lets through in some places.
fn is_format(character: char) -> bool {
    matches!(
        character,
        JOINER | NON_JOINER | LEFT_TO_RIGHT_MARK | RIGHT_TO_LEFT_MARK | TEXT_SELECTOR | EMOJI_SELECTOR
    )
}
