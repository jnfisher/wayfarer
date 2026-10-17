//! Speed figures for the pull request, not run in CI:
//!
//!     cargo test --release -p wayfarer-parse --test speed -- --ignored --nocapture
//!
//! Targets (docs/plan/tasks/C-010-parser.md): the registry built from the export in under
//! 150 ms, and under 5 µs a line on average (the Python is about 11 µs on its own logs).

mod common;

use std::time::{Duration, Instant};

use common::{Input, Step};
use wayfarer_parse::{GameTexts, Registry};

#[test]
#[ignore = "timing; run in release by hand"]
fn speed() {
    let mut builds = Vec::new();
    let mut reg = None;
    for _ in 0..5 {
        let t = Instant::now();
        let texts = GameTexts::from_export(common::fixtures().join("world")).unwrap();
        let loaded = t.elapsed();
        let r = Registry::new(&texts);
        builds.push((t.elapsed(), loaded));
        reg = Some(r);
    }
    builds.sort();
    let (build, load) = builds[builds.len() / 2];
    eprintln!(
        "registry: {build:?} median of 5 ({load:?} reading the export, {:?} building)",
        build - load
    );
    let reg = std::sync::Arc::new(reg.unwrap());

    let inputs: Vec<Input> = common::fixture_names()
        .iter()
        .map(|n| common::input(n))
        .collect();
    let lines: usize = inputs
        .iter()
        .map(|i| match i {
            Input::Raw(t) => {
                let mut c = wayfarer_parse::LineCutter::new();
                let mut out = Vec::new();
                c.feed(t, &mut out);
                c.finish(&mut out);
                out.len()
            }
            Input::Transcript(s) => s.iter().filter(|s| matches!(s, Step::Line(_))).count(),
        })
        .sum();
    let mut passes = Vec::new();
    for pass in 0..6 {
        let t = Instant::now();
        let mut events = 0;
        for i in &inputs {
            events += common::run(&reg, i).len();
        }
        let el = t.elapsed();
        if pass == 0 {
            eprintln!(
                "first pass (compiles the regexes it meets): {:.2} µs/line, {events} events",
                per_line(el, lines)
            );
        } else {
            passes.push(el);
        }
    }
    passes.sort();
    eprintln!(
        "classify: {:.2} µs/line over {lines} lines (median of 5 warm passes)",
        per_line(passes[2], lines)
    );
}

fn per_line(d: Duration, lines: usize) -> f64 {
    d.as_secs_f64() * 1e6 / lines as f64
}
