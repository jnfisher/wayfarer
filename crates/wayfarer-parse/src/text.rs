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

/// Cuts text into lines incrementally.
#[derive(Clone, Debug, Default)]
pub struct LineCutter {
    /// Text from an escape sequence that cannot be decided yet.
    pending: Vec<char>,
    /// The current line, after escapes and backspaces; may hold CRs and bells.
    cur: String,
}

impl LineCutter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add text; complete lines are appended to `out`.
    pub fn feed(&mut self, text: &str, out: &mut Vec<String>) {
        if self.pending.is_empty() && !text.contains('\x1b') {
            for c in text.chars() {
                self.put(c, out);
            }
            return;
        }
        self.pending.extend(text.chars());
        self.run(false, out);
    }

    /// End of text: whatever is left is the last line (possibly empty, as `str.split` gives).
    pub fn finish(&mut self, out: &mut Vec<String>) {
        self.run(true, out);
        let cur = std::mem::take(&mut self.cur);
        Self::cut(cur, false, out);
    }

    /// The text of the line in progress, for recognising a prompt before its line ends. `None`
    /// while it holds a bare CR (which may still end a line, or be erased).
    pub fn partial(&self) -> Option<String> {
        if self.cur.contains('\r') {
            return None;
        }
        Some(self.cur.chars().filter(|&c| c != '\x07').collect())
    }

    fn run(&mut self, last: bool, out: &mut Vec<String>) {
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
                Escape::Drop(n) => i += n,
                Escape::Literal => {
                    self.cur.push('\x1b');
                    i += 1;
                }
            }
        }
    }

    fn put(&mut self, c: char, out: &mut Vec<String>) {
        match c {
            '\n' | '\x01' => self.line_break(out),
            // `[^\n\x08]\x08` removed until nothing changes: a backspace erases the character
            // before it on its line, and one with nothing before it vanishes.
            '\x08' => {
                self.cur.pop();
            }
            _ => self.cur.push(c),
        }
    }

    fn line_break(&mut self, out: &mut Vec<String>) {
        let cur = std::mem::take(&mut self.cur);
        Self::cut(cur, true, out);
    }

    /// Bells go; then CR LF is one line break and a lone CR another.
    fn cut(cur: String, by_lf: bool, out: &mut Vec<String>) {
        let mut text: String = if cur.contains('\x07') {
            cur.chars().filter(|&c| c != '\x07').collect()
        } else {
            cur
        };
        if by_lf && text.ends_with('\r') {
            text.pop();
        }
        if text.contains('\r') {
            out.extend(text.split('\r').map(str::to_owned));
        } else {
            out.push(text);
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
