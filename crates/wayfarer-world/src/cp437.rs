//! Code page 437, the game's character set: bytes 0x80-0xFF to Unicode and back.
//! Bytes below 0x80 are ASCII both ways.

const HIGH: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐\
                    └┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■\u{a0}";

/// Decode CP437 bytes to a string.
pub fn decode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if b < 0x80 {
                b as char
            } else {
                HIGH.chars().nth(usize::from(b - 0x80)).unwrap_or('?')
            }
        })
        .collect()
}

/// Encode a string as CP437 bytes; a character CP437 lacks becomes `?`.
pub fn encode(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| {
            if c.is_ascii() {
                c as u8
            } else {
                HIGH.chars()
                    .position(|h| h == c)
                    .map_or(b'?', |i| 0x80 + i as u8)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_has_128_characters_and_round_trips() {
        assert_eq!(HIGH.chars().count(), 128);
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(encode(&decode(&all)), all);
        assert_eq!(decode(&[0xB0, 0xC4, 0xDB]), "░─█");
    }
}
