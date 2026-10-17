//! Golden parity with the prototype: every capture and transcript fixture, fed to the parser,
//! gives the events `patterns.py` gave (`tests/fixtures/parse-golden/`, see its PROVENANCE.md).
//!
//! It builds the full registry from the world export and classifies every fixture line
//! (under a second in a debug build).

mod common;

use std::collections::BTreeMap;

use common::{Input, Step};
use serde_json::Value as Json;

/// One line's events: (name, fields).
type LineEvents = Vec<(String, Json)>;

fn python_events(name: &str, input: &Input) -> BTreeMap<u64, LineEvents> {
    // Python numbers the transcript's `>>>` markers as lines too; this parser does not.
    let line_no: Vec<Option<u64>> = match input {
        Input::Raw(_) => Vec::new(),
        Input::Transcript(steps) => {
            let mut n = 0;
            steps
                .iter()
                .map(|s| match s {
                    Step::Line(_) => {
                        n += 1;
                        Some(n - 1)
                    }
                    Step::Sent(_) => None,
                })
                .collect()
        }
    };
    let mut out: BTreeMap<u64, LineEvents> = BTreeMap::new();
    let path = common::fixtures()
        .join("parse-golden/events")
        .join(format!("{name}.jsonl.gz"));
    for l in common::read_gz_lines(&path) {
        let v: Json = serde_json::from_str(&l).unwrap();
        let n = v["n"].as_u64().unwrap();
        let n = if line_no.is_empty() {
            n
        } else {
            line_no[n as usize].expect("an event on a marker line")
        };
        out.entry(n)
            .or_default()
            .push((v["name"].as_str().unwrap().to_owned(), v["fields"].clone()));
    }
    out
}

/// Lines where this parser differs from `patterns.py`, with the reason; each is listed in
/// the pull request. (fixture, line, reason)
const KNOWN: &[(&str, u64, &str)] = &[(
    "transcript-thornwyn-early",
    335,
    "\"The angry kobold thief lunges at Kasimir with their shortsword!\": WCCMSG 8294 and 8310 \
     tie (priority 56, specificity 14) under different index keys, which Python orders by set \
     iteration, so its answer depends on PYTHONHASHSEED (the golden uses 0). This parser takes \
     the earlier pattern, 8294, as Python does with seeds 1 and 3, and names the monster \
     `angry kobold thief`; seed 0 takes 8310 and names it `angry kobold`.",
)];

#[test]
fn golden_parity() {
    let reg = common::registry();
    let (mut total, mut same, mut py_unknown, mut rs_unknown) = (0usize, 0usize, 0usize, 0usize);
    let mut report = String::new();
    let mut unexpected = Vec::new();
    for name in common::fixture_names() {
        let input = common::input(&name);
        let want = python_events(&name, &input);
        let mut got: BTreeMap<u64, LineEvents> = BTreeMap::new();
        for ev in common::run(&reg, &input) {
            rs_unknown += usize::from(ev.name == "unknown");
            got.entry(ev.line_no)
                .or_default()
                .push((ev.name, serde_json::to_value(&ev.fields).unwrap()));
        }
        py_unknown += want
            .values()
            .flatten()
            .filter(|(n, _)| n == "unknown")
            .count();
        let mut lines: Vec<u64> = want.keys().chain(got.keys()).copied().collect();
        lines.sort_unstable();
        lines.dedup();
        for n in lines {
            total += 1;
            let (w, g) = (want.get(&n), got.get(&n));
            if w == g {
                same += 1;
                continue;
            }
            report.push_str(&format!("{name}:{n}\n  python {w:?}\n  rust   {g:?}\n"));
            if !KNOWN.iter().any(|(f, l, _)| *f == name && *l == n) {
                unexpected.push(format!("{name}:{n}"));
            }
        }
    }
    let rate = same as f64 / total as f64;
    eprintln!("{report}");
    eprintln!(
        "golden parity: {same}/{total} lines agree ({:.2}%); unknown events: python {py_unknown}, rust {rs_unknown}",
        rate * 100.0
    );
    assert!(rate >= 0.995, "parity {:.2}% is under 99.5%", rate * 100.0);
    assert!(
        rs_unknown <= py_unknown,
        "more unclassified lines than patterns.py"
    );
    assert!(
        unexpected.is_empty(),
        "disagreements not in KNOWN: {unexpected:?}"
    );
}
