//! CP437: bytes 0x80-0xFF to Unicode and back. 0x00-0x7F pass through unchanged, so ESC and
//! the other controls stay controls and ANSI survives.

/// Unicode for bytes 0x80-0xFF.
pub const HIGH: [char; 128] = [
    'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', //
    'É', 'æ', 'Æ', 'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', //
    'á', 'í', 'ó', 'ú', 'ñ', 'Ñ', 'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', //
    '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕', '╣', '║', '╗', '╝', '╜', '╛', '┐', //
    '└', '┴', '┬', '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦', '╠', '═', '╬', '╧', //
    '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐', '▀', //
    'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', //
    '≡', '±', '≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', '\u{a0}',
];

/// The character for one received byte.
pub fn decode_byte(b: u8) -> char {
    if b < 0x80 {
        b as char
    } else {
        HIGH[usize::from(b) - 0x80]
    }
}

/// The byte for a character, or `?` if CP437 has no form for it.
pub fn encode_char(c: char) -> u8 {
    if c.is_ascii() {
        return c as u8;
    }
    HIGH.iter()
        .position(|&h| h == c)
        .map_or(b'?', |i| 0x80 + i as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_byte() {
        for b in 0..=255u8 {
            assert_eq!(encode_char(decode_byte(b)), b, "byte {b:#x}");
        }
    }

    #[test]
    fn table_has_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        assert!(HIGH.iter().all(|c| seen.insert(*c)));
    }

    #[test]
    fn banner_characters() {
        assert_eq!(decode_byte(0xC9), '╔');
        assert_eq!(decode_byte(0xDB), '█');
        assert_eq!(decode_byte(0x1B), '\u{1b}');
    }

    #[test]
    fn unmappable_becomes_question_mark() {
        assert_eq!(encode_char('€'), b'?');
        assert_eq!(encode_char('日'), b'?');
    }
}
