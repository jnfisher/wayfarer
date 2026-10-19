//! Split feeding: each capture fed in random chunk sizes gives the events, and the typed events,
//! it gives fed whole.

mod common;

use common::Input;
use regex::Regex;
use wayfarer_parse::typed::GameEvent;
use wayfarer_parse::{Event, Parser};

/// A small xorshift generator: deterministic, no dependency.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }
}

/// What must match: everything but the chunk number.
fn key(evs: &[Event]) -> Vec<(String, String, String, String, u64)> {
    evs.iter()
        .map(|e| {
            (
                e.name.clone(),
                e.cat.clone(),
                serde_json::to_string(&e.fields).unwrap(),
                e.line.clone(),
                e.line_no,
            )
        })
        .collect()
}

/// The typed events, without the chunk number of the events they keep.
fn typed(evs: &[Event]) -> Vec<GameEvent> {
    evs.iter()
        .map(|e| match GameEvent::from_event(e) {
            GameEvent::Other(mut e) => {
                e.seq = 0;
                GameEvent::Other(e)
            }
            t => t,
        })
        .collect()
}

#[test]
fn random_chunks_give_the_same_events() {
    let reg = common::registry();
    // The one documented exception: a cut after a prompt's `]:` and before the end of the
    // resting mark or debug tail that follows it (the game writes a prompt in one piece).
    // Cuts there move on.
    let tail = Regex::new(
        r"\]:(?:\x1b\[[0-9;]*[A-Za-z])*( \((?:Resting|Meditating)\) |\{Energy=[^}]*\}:)",
    )
    .unwrap();
    for name in common::fixture_names()
        .into_iter()
        .filter(|n| n.starts_with("capture-"))
    {
        let Input::Raw(text) = common::input(&name) else {
            unreachable!()
        };
        let whole = {
            let mut p = Parser::new(reg.clone());
            let mut evs = p.feed(&text);
            evs.extend(p.finish());
            (key(&evs), typed(&evs))
        };
        let (whole, whole_typed) = whole;
        let forbidden: Vec<(usize, usize)> = tail
            .captures_iter(&text)
            .map(|c| (c.get(0).unwrap().start() + 2, c.get(1).unwrap().end() - 1))
            .collect();
        for (seed, max) in [(1u64, 6usize), (2, 17), (3, 64), (4, 300)] {
            let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let mut p = Parser::new(reg.clone());
            let mut evs = Vec::new();
            let mut at = 0;
            while at < text.len() {
                let mut end = (at + 1 + rng.below(max)).min(text.len());
                while !text.is_char_boundary(end)
                    || forbidden.iter().any(|&(a, b)| end >= a && end <= b)
                {
                    end += 1;
                }
                evs.extend(p.feed(&text[at..end]));
                at = end;
            }
            evs.extend(p.finish());
            assert!(
                typed(&evs) == whole_typed,
                "{name} in chunks of 1..={max}: typed events differ"
            );
            let got = key(&evs);
            if let Some(i) = (0..got.len().max(whole.len())).find(|&i| got.get(i) != whole.get(i)) {
                let lo = i.saturating_sub(2);
                panic!(
                    "{name} in chunks of 1..={max}: event {i} differs\n  split {:#?}\n  whole {:#?}",
                    &got[lo..(i + 2).min(got.len())],
                    &whole[lo..(i + 2).min(whole.len())]
                );
            }
        }
    }
}
