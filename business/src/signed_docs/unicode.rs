//! Unicode character classes the signed document rules need (names, descriptions, labels).

/// The Unicode format characters (general category `Cf`, Unicode 16): bidirectional controls,
/// zero-width characters, the soft hyphen, tags and the like. They are invisible or reorder text,
/// so a name or description the user reviews could read differently from what it holds.
const FORMAT_CHARS: &[(u32, u32)] = &[
    (0x00AD, 0x00AD),
    (0x0600, 0x0605),
    (0x061C, 0x061C),
    (0x06DD, 0x06DD),
    (0x070F, 0x070F),
    (0x0890, 0x0891),
    (0x08E2, 0x08E2),
    (0x180E, 0x180E),
    (0x200B, 0x200F),
    (0x202A, 0x202E),
    (0x2060, 0x2064),
    (0x2066, 0x206F),
    (0xFEFF, 0xFEFF),
    (0xFFF9, 0xFFFB),
    (0x110BD, 0x110BD),
    (0x110CD, 0x110CD),
    (0x13430, 0x1343F),
    (0x1BCA0, 0x1BCA3),
    (0x1D173, 0x1D17A),
    (0xE0001, 0xE0001),
    (0xE0020, 0xE007F),
];

/// Invisible characters outside `Cf` that Harness Memory rejects as well: the line and paragraph
/// separators (`Zl`, `Zp`), the combining grapheme joiner, the Hangul fillers (U+115F, U+1160,
/// U+3164, U+FFA0), the Khmer inherent vowels, the Mongolian variation selectors (U+180B to
/// U+180F, U+180E is `Cf`), the blank Braille pattern and the variation selectors (U+FE00 to
/// U+FE0F, U+E0100 to U+E01EF). A name or description holding one shows a break or nothing where
/// the file holds something.
const INVISIBLE_CHARS: &[(u32, u32)] = &[
    (0x034F, 0x034F),
    (0x115F, 0x1160),
    (0x17B4, 0x17B5),
    (0x180B, 0x180F),
    (0x2028, 0x2029),
    (0x2800, 0x2800),
    (0x3164, 0x3164),
    (0xFE00, 0xFE0F),
    (0xFFA0, 0xFFA0),
    (0xE0100, 0xE01EF),
];

/// True for a character a name or description may not hold: a format character
/// ([`is_format_char`]) or another invisible one ([`INVISIBLE_CHARS`]).
pub(crate) fn is_hidden_char(character: char) -> bool {
    let code = u32::from(character);

    is_format_char(character)
        || INVISIBLE_CHARS
            .iter()
            .any(|(first, last)| (*first..=*last).contains(&code))
}

/// `text` with each hidden character ([`is_hidden_char`]) spelled out as `⟨U+XXXX⟩`, the way the
/// app shows them elsewhere, so text the user approves reads as it runs (no bidirectional control
/// reorders it, no zero-width character hides in it).
pub(crate) fn spelled_out(text: &str) -> String {
    let mut shown = String::with_capacity(text.len());

    for character in text.chars() {
        if is_hidden_char(character) {
            shown.push_str(&format!("⟨U+{:04X}⟩", u32::from(character)));
        } else {
            shown.push(character);
        }
    }

    shown
}

/// True for a Unicode format character (see [`FORMAT_CHARS`]).
pub(crate) fn is_format_char(character: char) -> bool {
    let code = u32::from(character);

    FORMAT_CHARS
        .iter()
        .any(|(first, last)| (*first..=*last).contains(&code))
}

#[cfg(test)]
mod tests {
    use super::is_format_char;

    /// `Cf` of Unicode 16.0.0 as `first..last` hex ranges, generated from the Unicode data (see
    /// the file's header).
    const CF_16: &str = include_str!("../../tests/fixtures/unicode/cf-16.0.0.txt");

    fn listed_ranges() -> Vec<(u32, u32)> {
        CF_16
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| {
                let (first, last) = line.split_once("..").expect("a first..last range");
                let parse = |hex: &str| u32::from_str_radix(hex, 16).expect("a hex code point");

                (parse(first), parse(last))
            })
            .collect()
    }

    #[test]
    fn the_format_characters_are_exactly_unicode_16_cf() {
        let ranges = listed_ranges();
        assert!(!ranges.is_empty());

        for character in (0..=0x10FFFF).filter_map(char::from_u32) {
            let code = u32::from(character);
            let listed = ranges.iter().any(|(first, last)| (*first..=*last).contains(&code));

            assert_eq!(is_format_char(character), listed, "U+{code:04X}");
        }
    }
}
