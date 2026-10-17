//! Unicode character classes the hook file rules need.

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
