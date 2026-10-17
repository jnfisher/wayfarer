//! Shared test helpers: fixture paths, CP437, the golden events.
#![allow(dead_code)]

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::Arc;

use flate2::read::GzDecoder;
use wayfarer_parse::{GameTexts, Parser, Registry};

pub fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

pub fn registry() -> Arc<Registry> {
    let texts = GameTexts::from_export(fixtures().join("world")).expect("world export loads");
    Arc::new(Registry::new(&texts))
}

/// Python's `cp437` codec: ASCII as is, the upper half as the IBM PC glyphs.
pub fn cp437(bytes: &[u8]) -> String {
    const HIGH: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■\u{a0}";
    let high: Vec<char> = HIGH.chars().collect();
    assert_eq!(high.len(), 128);
    bytes
        .iter()
        .map(|&b| {
            if b < 0x80 {
                b as char
            } else {
                high[b as usize - 0x80]
            }
        })
        .collect()
}

pub fn read_gz_lines(path: &std::path::Path) -> Vec<String> {
    let f = std::fs::File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    BufReader::new(GzDecoder::new(f))
        .lines()
        .map(|l| l.unwrap())
        .collect()
}

/// A fixture's input: a raw capture (`raw.bin`, fed as one text) or a transcript (lines,
/// with the `>>> 'cmd'` markers turned into `sent` calls, as `patterns.selftest` reads it).
pub enum Input {
    Raw(String),
    Transcript(Vec<Step>),
}

pub enum Step {
    Line(String),
    Sent(String),
}

/// `m.group(1).encode().decode("unicode_escape")` on a `repr()` body.
fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    let hex = |h: &[u8]| u32::from_str_radix(std::str::from_utf8(h).unwrap(), 16).unwrap();
    while i < b.len() {
        if b[i] == b'\\' && i + 1 < b.len() {
            let (c, n) = match b[i + 1] {
                b'n' => ('\n', 2),
                b'r' => ('\r', 2),
                b't' => ('\t', 2),
                b'\\' => ('\\', 2),
                b'\'' => ('\'', 2),
                b'"' => ('"', 2),
                b'x' => (char::from_u32(hex(&b[i + 2..i + 4])).unwrap(), 4),
                b'u' => (char::from_u32(hex(&b[i + 2..i + 6])).unwrap(), 6),
                _ => ('\\', 1),
            };
            out.push(c);
            i += n;
        } else {
            // unicode_escape reads the UTF-8 bytes as Latin-1
            out.push(b[i] as char);
            i += 1;
        }
    }
    out
}

pub fn fixture_names() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(fixtures().join("parse-golden/events"))
        .unwrap()
        .map(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .trim_end_matches(".jsonl.gz")
                .to_owned()
        })
        .collect();
    v.sort();
    v
}

pub fn input(name: &str) -> Input {
    if let Some(cap) = name.strip_prefix("capture-") {
        let raw = std::fs::read(fixtures().join("captures").join(cap).join("raw.bin")).unwrap();
        Input::Raw(cp437(&raw))
    } else {
        let t = name.strip_prefix("transcript-").unwrap();
        let text = std::fs::read_to_string(fixtures().join("transcripts").join(format!("{t}.txt")))
            .unwrap();
        let mut steps = Vec::new();
        for line in text.split('\n') {
            if line.starts_with("### ") {
                continue;
            }
            if let Some(rest) = line.strip_prefix(">>> ") {
                if let Some(body) = rest.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')) {
                    steps.push(Step::Sent(unescape(body)));
                }
                continue;
            }
            steps.push(Step::Line(line.to_owned()));
        }
        Input::Transcript(steps)
    }
}

/// Feed a fixture whole (a capture as one chunk; a transcript a line at a time).
pub fn run(reg: &Arc<Registry>, input: &Input) -> Vec<wayfarer_parse::Event> {
    let mut p = Parser::new(reg.clone());
    let mut out = Vec::new();
    match input {
        Input::Raw(text) => out.extend(p.feed(text)),
        Input::Transcript(steps) => {
            let n = steps.iter().filter(|s| matches!(s, Step::Line(_))).count();
            let mut k = 0;
            for s in steps {
                match s {
                    Step::Sent(c) => p.sent(c),
                    Step::Line(l) => {
                        k += 1;
                        // the last element of split("\n") has no newline after it
                        if k < n {
                            out.extend(p.feed(&format!("{l}\n")));
                        } else {
                            out.extend(p.feed(l));
                        }
                    }
                }
            }
        }
    }
    out.extend(p.finish());
    out
}
