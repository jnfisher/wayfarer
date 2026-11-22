//! Cutting terminal text into lines as it arrives (`patterns.raw_to_lines`, made incremental).
//!
//! `ESC[79D ESC[K` (ANSI_CLRLINE, which starts nearly every message), `^A`, CR and LF end a
//! line; other ANSI sequences are dropped; backspaces are applied (direction names carry a
//! random capital and a backspace, `nJ\borth`); bells are dropped. The result is the same as
//! `raw_to_lines` on the whole text, however the text is split into chunks.

const CLRLINE: [char; 8] = ['\x1b', '[', '7', '9', 'D', '\x1b', '[', 'K'];

/// What an escape sequence at the front of the pending text turned out to be.
enum Escape {
    /// More text is needed to tell.
    Wait,
    /// ANSI_CLRLINE: a line break.
    LineBreak,
    /// An ANSI sequence of this many characters: dropped.
    Drop(usize),
    /// Not a sequence `raw_to_lines` removes: the ESC stays in the text.
    Literal,
}

fn clrline_at(s: &[char], i: usize, last: bool) -> Option<bool> {
    let rest = &s[i..];
    if rest.len() >= CLRLINE.len() {
        Some(rest[..CLRLINE.len()] == CLRLINE)
    } else if !last && CLRLINE.starts_with(rest) {
        None
    } else {
        Some(false)
    }
}

/// `raw_to_lines` replaces CLRLINE with a line break first, then removes
/// `\x1b\[[0-9;?]*[A-Za-z]|\x1b[()][A-Z0-9]|\x1b.` (where `.` never takes a line break).
fn escape_at(s: &[char], i: usize, last: bool) -> Escape {
    match clrline_at(s, i, last) {
        None => return Escape::Wait,
        Some(true) => return Escape::LineBreak,
        Some(false) => {}
    }
    let rest = &s[i..];
    if rest.len() < 2 {
        return if last { Escape::Literal } else { Escape::Wait };
    }
    if rest[1] == '[' {
        let mut j = 2;
        while j < rest.len() && (rest[j].is_ascii_digit() || rest[j] == ';' || rest[j] == '?') {
            j += 1;
        }
        if j == rest.len() {
            if !last {
                return Escape::Wait;
            }
        } else if rest[j].is_ascii_alphabetic() {
            return Escape::Drop(j + 1);
        }
    }
    if rest[1] == '(' || rest[1] == ')' {
        if rest.len() == 2 {
            if !last {
                return Escape::Wait;
            }
        } else if rest[2].is_ascii_uppercase() || rest[2].is_ascii_digit() {
            return Escape::Drop(3);
        }
    }
    match rest[1] {
        '\n' | '\x01' => Escape::Literal,
        '\x1b' => match clrline_at(s, i + 1, last) {
            None => Escape::Wait,
            Some(true) => Escape::Literal,
            Some(false) => Escape::Drop(2),
        },
        _ => Escape::Drop(2),
    }
}

/// One line as the cutter hands it up: its text with the escapes gone, and the colours
/// (`ESC[...m`, SGR) that were in the text, each with the byte offset in `text` of the first
/// character it coloured (C-129: a name's colour in `Also here:` tells its alignment). A line
/// cut from the middle of coloured text starts with the colour in force at offset 0.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CutLine {
    pub text: String,
    /// `(offset, parameters)`: `(11, "1;35")` for `ESC[1;35m` before the 12th byte.
    pub colours: Vec<(usize, String)>,
}

/// Cuts text into lines incrementally.
#[derive(Clone, Debug, Default)]
pub struct LineCutter {
    /// Text from an escape sequence that cannot be decided yet.
    pending: Vec<char>,
    /// The current line, after escapes and backspaces; may hold CRs and bells.
    cur: String,
    /// The SGR sequences dropped from `cur`, each at the count of chars before it.
    colours: Vec<(usize, String)>,
}

impl LineCutter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add text; complete lines are appended to `out`.
    pub fn feed(&mut self, text: &str, out: &mut Vec<String>) {
        let mut lines = Vec::new();
        self.feed_lines(text, &mut lines);
        out.extend(lines.into_iter().map(|l| l.text));
    }

    /// End of text: whatever is left is the last line (possibly empty, as `str.split` gives).
    pub fn finish(&mut self, out: &mut Vec<String>) {
        let mut lines = Vec::new();
        self.finish_lines(&mut lines);
        out.extend(lines.into_iter().map(|l| l.text));
    }

    /// [`LineCutter::feed`], with each line's colours.
    pub fn feed_lines(&mut self, text: &str, out: &mut Vec<CutLine>) {
        if self.pending.is_empty() && !text.contains('\x1b') {
            for c in text.chars() {
                self.put(c, out);
            }
            return;
        }
        self.pending.extend(text.chars());
        self.run(false, out);
    }

    /// [`LineCutter::finish`], with each line's colours.
    pub fn finish_lines(&mut self, out: &mut Vec<CutLine>) {
        self.run(true, out);
        let cur = std::mem::take(&mut self.cur);
        let colours = std::mem::take(&mut self.colours);
        Self::cut(cur, colours, false, out);
    }

    /// The text of the line in progress, for recognising a prompt before its line ends. `None`
    /// while it holds a bare CR (which may still end a line, or be erased).
    pub fn partial(&self) -> Option<String> {
        if self.cur.contains('\r') {
            return None;
        }
        Some(self.cur.chars().filter(|&c| c != '\x07').collect())
    }

    fn run(&mut self, last: bool, out: &mut Vec<CutLine>) {
        let s = std::mem::take(&mut self.pending);
        let mut i = 0;
        while i < s.len() {
            let c = s[i];
            if c != '\x1b' {
                self.put(c, out);
                i += 1;
                continue;
            }
            match escape_at(&s, i, last) {
                Escape::Wait => {
                    self.pending = s[i..].to_vec();
                    return;
                }
                Escape::LineBreak => {
                    self.line_break(out);
                    i += CLRLINE.len();
                }
                Escape::Drop(n) => {
                    // A colour (`ESC[...m`) is kept with its place in the line.
                    if n > 2 && s[i + 1] == '[' && s[i + n - 1] == 'm' {
                        let params: String = s[i + 2..i + n - 1].iter().collect();
                        self.colours.push((self.cur.chars().count(), params));
                    }
                    i += n;
                }
                Escape::Literal => {
                    self.cur.push('\x1b');
                    i += 1;
                }
            }
        }
    }

    fn put(&mut self, c: char, out: &mut Vec<CutLine>) {
        match c {
            '\n' | '\x01' => self.line_break(out),
            // `[^\n\x08]\x08` removed until nothing changes: a backspace erases the character
            // before it on its line, and one with nothing before it vanishes.
            '\x08' => {
                self.cur.pop();
                if !self.colours.is_empty() {
                    let len = self.cur.chars().count();
                    for c in &mut self.colours {
                        c.0 = c.0.min(len);
                    }
                }
            }
            _ => self.cur.push(c),
        }
    }

    fn line_break(&mut self, out: &mut Vec<CutLine>) {
        let cur = std::mem::take(&mut self.cur);
        let colours = std::mem::take(&mut self.colours);
        Self::cut(cur, colours, true, out);
    }

    /// Bells go; then CR LF is one line break and a lone CR another. `colours` are at char
    /// counts into `cur`; each line gets its own, at byte offsets, the colour in force at its
    /// start first.
    fn cut(cur: String, mut colours: Vec<(usize, String)>, by_lf: bool, out: &mut Vec<CutLine>) {
        let mut text: String = if cur.contains('\x07') {
            // A colour's place moves up by the bells before it.
            let bells: Vec<usize> = cur
                .chars()
                .enumerate()
                .filter(|&(_, c)| c == '\x07')
                .map(|(i, _)| i)
                .collect();
            for c in &mut colours {
                c.0 -= bells.iter().filter(|&&b| b < c.0).count();
            }
            cur.chars().filter(|&c| c != '\x07').collect()
        } else {
            cur
        };
        if by_lf && text.ends_with('\r') {
            text.pop();
        }
        if colours.is_empty() {
            if text.contains('\r') {
                out.extend(text.split('\r').map(|t| CutLine {
                    text: t.to_owned(),
                    colours: Vec::new(),
                }));
            } else {
                out.push(CutLine {
                    text,
                    colours: Vec::new(),
                });
            }
            return;
        }
        let mut start = 0; // chars before this piece
        let mut in_force: Option<String> = None;
        for piece in text.split('\r') {
            let len = piece.chars().count();
            let bytes: Vec<usize> = piece
                .char_indices()
                .map(|(b, _)| b)
                .chain(std::iter::once(piece.len()))
                .collect();
            let mut own: Vec<(usize, String)> = Vec::new();
            for (at, params) in &colours {
                if *at <= start {
                    in_force = Some(params.clone());
                } else if *at <= start + len {
                    own.push((bytes[*at - start], params.clone()));
                }
            }
            if let Some(p) = &in_force
                && own.first().is_none_or(|(b, _)| *b > 0)
            {
                own.insert(0, (0, p.clone()));
            }
            if let Some((_, p)) = own.last() {
                in_force = Some(p.clone());
            }
            out.push(CutLine {
                text: piece.to_owned(),
                colours: own,
            });
            start += len + 1; // the CR
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(chunks: &[&str]) -> Vec<String> {
        let mut c = LineCutter::new();
        let mut out = Vec::new();
        for ch in chunks {
            c.feed(ch, &mut out);
        }
        c.finish(&mut out);
        out
    }

    fn coloured(chunks: &[&str]) -> Vec<CutLine> {
        let mut c = LineCutter::new();
        let mut out = Vec::new();
        for ch in chunks {
            c.feed_lines(ch, &mut out);
        }
        c.finish_lines(&mut out);
        out
    }

    #[test]
    fn colours_keep_their_place_in_the_line() {
        // Asa's room view (silver-mirror, 2026-11-22): the lunatic in bright magenta.
        let got = coloured(&[
            "\x1b[79D\x1b[K\x1b[1;36mWarped Asylum\r\n\x1b[0;35mAlso here: \x1b[1;35mcrazed lunatic\x1b[0m\x1b[0;35m.\r\n",
        ]);
        // The clear-line sequence that opens the chunk ends an empty line first.
        assert_eq!(got[0].text, "");
        assert_eq!(got[1].text, "Warped Asylum");
        assert_eq!(got[1].colours, [(0, "1;36".to_owned())]);
        assert_eq!(got[2].text, "Also here: crazed lunatic.");
        assert_eq!(
            got[2].colours,
            [
                (0, "0;35".to_owned()),
                (11, "1;35".to_owned()),
                (25, "0".to_owned()),
                (25, "0;35".to_owned()),
            ]
        );
        // The last colour of a line is in force at the start of the next; a backspace moves a
        // colour back with the text it coloured; a bell takes no place.
        let got = coloured(&["\x1b[0;32mab\rcd\x07\x1b[1;31mX\x08Y\n"]);
        assert_eq!(got[0].text, "ab");
        assert_eq!(got[1].text, "cdY");
        assert_eq!(
            got[1].colours,
            [(0, "0;32".to_owned()), (2, "1;31".to_owned())]
        );
        // Split between the escape's characters: the colour still lands.
        let got = coloured(&["a\x1b[1;", "35mb\n"]);
        assert_eq!(got[0].colours, [(1, "1;35".to_owned())]);
    }

    #[test]
    fn backspaces_are_applied() {
        assert_eq!(
            lines(&["Obvious exits: nJ\x08orth, sD\x08outh\r\n"]),
            ["Obvious exits: north, south", ""]
        );
        assert_eq!(lines(&["ab\x08\x08\x08c\n"]), ["c", ""]);
        assert_eq!(lines(&["x\n\x08y"]), ["x", "y"]);
        // a bell is erased by the backspace after it, as `[^\n\x08]\x08` would
        assert_eq!(lines(&["a\x07\x08"]), ["a"]);
        // split between the letter and its backspace
        assert_eq!(lines(&["nJ", "\x08orth"]), ["north"]);
    }

    #[test]
    fn clrline_cr_lf_and_ansi() {
        let raw = "\x1b[79D\x1b[K\x1b[0;37m[HP=35\x1b[0;37m]:w\r\n\x1b[0;37;40m\x1b[79D\x1b[K\x1b[1;36mNewhaven\r\n";
        assert_eq!(lines(&[raw]), ["", "[HP=35]:w", "", "Newhaven", ""]);
        for k in 1..raw.len() {
            assert_eq!(
                lines(&[&raw[..k], &raw[k..]]),
                lines(&[raw]),
                "split at {k}"
            );
        }
        assert_eq!(lines(&["a\rb\r\nc\r"]), ["a", "b", "c", ""]);
        assert_eq!(lines(&["a\x01b"]), ["a", "b"]);
    }

    #[test]
    fn escapes_raw_to_lines_keeps() {
        // ESC before a line break is not a sequence; ESC ESC... before CLRLINE either
        assert_eq!(lines(&["a\x1b\nb"]), ["a\x1b", "b"]);
        assert_eq!(lines(&["a\x1b\x1b[79D\x1b[Kb"]), ["a\x1b", "b"]);
        assert_eq!(lines(&["a\x1b(Bb\x1b)x"]), ["abx"]);
        assert_eq!(lines(&["a\x1b[12"]), ["a12"]);
    }
}
