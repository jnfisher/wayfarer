//! Python `re` compatibility.
//!
//! The prototype's patterns are Python regex sources, and several of its decisions are made on
//! that source text: a pattern's specificity (`Pattern.spec`) and its index key
//! (`_regex_tokens`). To keep those decisions identical, patterns are built as Python sources
//! exactly as `patterns.py` builds them, and translated to the `regex` crate's syntax only to
//! compile them.

/// Python's `str.isspace()`: Unicode White_Space plus the four ASCII separators U+001C..U+001F.
pub fn is_py_space(c: char) -> bool {
    c.is_whitespace() || ('\x1c'..='\x1f').contains(&c)
}

/// Python's `str.strip()`.
pub fn py_strip(s: &str) -> &str {
    s.trim_matches(is_py_space)
}

/// Python's `str.split()` (no separator): runs of whitespace, no empty words.
pub fn py_split(s: &str) -> impl Iterator<Item = &str> {
    s.split(is_py_space).filter(|w| !w.is_empty())
}

/// Python 3.7+ `re.escape`.
pub fn py_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        if "()[]{}?*+-|^$\\.&~# \t\n\r\x0b\x0c".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn is_py_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `Pattern.spec`: the length of the source once `(?P<name>`, escapes (`\x`, two characters)
/// and the metacharacters `()?*+.|[]{}^$` are removed (`patterns.py:765`).
pub fn spec(src: &str) -> usize {
    let s: Vec<char> = src.chars().collect();
    let mut n = 0;
    let mut i = 0;
    while i < s.len() {
        if s[i] == '('
            && s.get(i + 1) == Some(&'?')
            && s.get(i + 2) == Some(&'P')
            && s.get(i + 3) == Some(&'<')
        {
            let mut j = i + 4;
            while j < s.len() && is_py_word(s[j]) {
                j += 1;
            }
            if j > i + 4 && s.get(j) == Some(&'>') {
                i = j + 1;
                continue;
            }
        }
        if s[i] == '\\' && i + 1 < s.len() && s[i + 1] != '\n' {
            i += 2;
            continue;
        }
        if !"()?*+.|[]{}^$".contains(s[i]) {
            n += 1;
        }
        i += 1;
    }
    n
}

/// `_regex_tokens`: the literal words of a regex that are always present (depth 0, space
/// bounded, no metacharacter). Ported as is, quirks included: an escaped character inside a
/// group adds nothing, and a character class ends at the first `]`, escaped or not.
pub fn regex_tokens(src: &str) -> Vec<String> {
    const LITERAL_ESCAPES: &str = ".!*()[]?+{}|$^-/\"'#&%:,;=<>@ ";
    let s: Vec<char> = src.chars().collect();
    let mut cur: Vec<char> = Vec::with_capacity(s.len());
    let mut depth: i32 = 0;
    let mut i = 0;
    while i < s.len() {
        let c = s[i];
        if c == '\\' && i + 1 < s.len() {
            let nxt = s[i + 1];
            if LITERAL_ESCAPES.contains(nxt) {
                if depth == 0 {
                    cur.push(nxt);
                }
            } else {
                cur.push('\0');
            }
            i += 2;
            continue;
        }
        if c == '[' {
            let j = s[i + 1..]
                .iter()
                .position(|&x| x == ']')
                .map_or(s.len(), |p| i + 1 + p);
            cur.push('\0');
            i = j + 1;
            continue;
        }
        match c {
            '(' => {
                depth += 1;
                cur.push('\0');
            }
            ')' => {
                depth -= 1;
                cur.push('\0');
            }
            '.' | '*' | '+' | '?' | '{' | '}' | '|' | '^' | '$' => {
                if c == '|' && depth == 0 {
                    return Vec::new();
                }
                cur.push('\0');
            }
            _ if depth == 0 => cur.push(c),
            _ => cur.push('\0'),
        }
        i += 1;
    }
    let text: String = cur.into_iter().collect();
    text.split(' ')
        .filter(|p| !p.is_empty() && !p.contains('\0'))
        .map(str::to_owned)
        .collect()
}

/// Is `s[i..]` a `{m}`, `{m,}` or `{m,n}` quantifier? Returns its length.
fn quantifier_len(s: &[char], i: usize) -> Option<usize> {
    let mut j = i + 1;
    let start = j;
    while j < s.len() && s[j].is_ascii_digit() {
        j += 1;
    }
    if j == start {
        return None;
    }
    if s.get(j) == Some(&',') {
        j += 1;
        while j < s.len() && s[j].is_ascii_digit() {
            j += 1;
        }
    }
    (s.get(j) == Some(&'}')).then_some(j + 1 - i)
}

fn push_escaped(out: &mut String, c: char, in_class: bool) {
    if c.is_ascii_alphanumeric() {
        // \d \s \w \b \x.. \u....: the same in both syntaxes, except Python's \Z.
        out.push('\\');
        out.push(if c == 'Z' { 'z' } else { c });
    } else if c == '<' || c == '>' || !c.is_ascii() {
        // `\<` and `\>` are word boundaries in `regex`; in Python they are the plain characters.
        out.push(c);
    } else if c == ' ' && !in_class {
        out.push(' ');
    } else {
        out.push('\\');
        out.push(c);
    }
}

/// Translate a Python regex source (as `patterns.py` writes them) into `regex` crate syntax.
/// Lookaround and backreferences have no translation and are an error; the one lookahead in
/// the prototype (the generic prompt) is handled by hand in `prompt.rs`.
pub fn to_rust(src: &str) -> Result<String, String> {
    let s: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len() + 16);
    let mut i = 0;
    while i < s.len() {
        let c = s[i];
        match c {
            '\\' => {
                let Some(&n) = s.get(i + 1) else {
                    return Err(format!("trailing backslash in {src:?}"));
                };
                if n == 'u' {
                    // \uXXXX: the same in both.
                    out.extend(&s[i..(i + 6).min(s.len())]);
                    i += 6;
                    continue;
                }
                push_escaped(&mut out, n, false);
                i += 2;
            }
            '[' => {
                out.push('[');
                i += 1;
                if s.get(i) == Some(&'^') {
                    out.push('^');
                    i += 1;
                }
                let mut first = true;
                let mut prev = '\0';
                loop {
                    let Some(&c) = s.get(i) else {
                        return Err(format!("unterminated class in {src:?}"));
                    };
                    if c == ']' && !first {
                        out.push(']');
                        i += 1;
                        break;
                    }
                    match c {
                        '\\' => {
                            let Some(&n) = s.get(i + 1) else {
                                return Err(format!("trailing backslash in {src:?}"));
                            };
                            if n == 'u' {
                                out.extend(&s[i..(i + 6).min(s.len())]);
                                i += 6;
                                prev = 'u';
                                first = false;
                                continue;
                            }
                            push_escaped(&mut out, n, true);
                            i += 2;
                            prev = n;
                            first = false;
                            continue;
                        }
                        '[' | ']' | '&' | '~' => {
                            out.push('\\');
                            out.push(c);
                        }
                        '-' if prev == '-' => out.push_str("\\-"),
                        _ => out.push(c),
                    }
                    prev = c;
                    first = false;
                    i += 1;
                }
            }
            '{' => {
                if let Some(len) = quantifier_len(&s, i) {
                    out.extend(&s[i..i + len]);
                    i += len;
                } else {
                    out.push_str("\\{");
                    i += 1;
                }
            }
            '}' => {
                out.push_str("\\}");
                i += 1;
            }
            '(' if s.get(i + 1) == Some(&'?') => {
                match s.get(i + 2) {
                    Some(':') => out.push_str("(?:"),
                    Some('P') if s.get(i + 3) == Some(&'<') => out.push_str("(?P<"),
                    _ => return Err(format!("unsupported group syntax in {src:?}")),
                }
                i += if s.get(i + 2) == Some(&':') { 3 } else { 4 };
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_matches_python() {
        // python3 -c "import re; print(re.escape(' -#&~<>%!,:\"\''))"
        assert_eq!(py_escape(" -#&~<>%!,:\"'"), "\\ \\-\\#\\&\\~<>%!,:\"'");
    }

    #[test]
    fn spec_drops_groups_escapes_and_metacharacters() {
        assert_eq!(
            spec(r"You gain (?P<exp>\d+) experience\."),
            "You gain  experience".len()
        );
        assert_eq!(spec(r"\*Combat Engaged\*"), "Combat Engaged".len());
    }

    #[test]
    fn tokens_follow_the_prototype() {
        assert_eq!(
            regex_tokens(r"You gain (?P<exp>\d+) experience\."),
            ["You", "gain", "experience."]
        );
        assert!(regex_tokens(r"a|b").is_empty());
        assert_eq!(regex_tokens(r"(?:x\.y) z"), ["z"]);
    }

    #[test]
    fn translation_compiles_and_keeps_meaning() {
        for (py, ok, bad) in [
            (r"\{(?P<world>[^}]+)\}", "{Realm Of Legends}", "{x"),
            (r"You\ gain\ \#1\&\~", "You gain #1&~", "You gain 1"),
            (r"[a-z -]+\<", "ab -<", "ab"),
            (r".*[─-╿].*", "a\u{2502}b", "ab"),
            (r"[\s_\\/|.]+", r" _\/|.", "a"),
            (r"-{20,}", "--------------------", "---"),
        ] {
            let rx = regex::Regex::new(&format!("^(?:{})$", to_rust(py).unwrap())).unwrap();
            assert!(rx.is_match(ok), "{py} should match {ok:?}");
            assert!(!rx.is_match(bad), "{py} should not match {bad:?}");
        }
        assert!(to_rust(r"(?=x)").is_err());
    }
}
