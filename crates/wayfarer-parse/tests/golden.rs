//! Golden parity with the prototype: every capture and transcript fixture, fed to the parser,
//! gives the events `patterns.py` gave (`tests/fixtures/parse-golden/`, see its PROVENANCE.md).
//! The comparison is `common::golden_parity`; `wayfarer-world` runs it on a registry built from
//! the world pack too.
//!
//! It builds the full registry from the world export and classifies every fixture line
//! (under a second in a debug build).

mod common;

#[test]
fn golden_parity() {
    common::golden_parity(&common::registry());
}
