//! Generated-pattern parity: the registry built from the world export is the one `patterns.py`
//! builds (`tests/fixtures/parse-golden/registry.jsonl.gz`), pattern for pattern: the same role,
//! category, priority, specificity, index key and regex, in the same order; and each generated
//! regex reads its own game text as Python's does.

mod common;

use serde_json::Value as Json;

// Over a second (about two in a debug build): it compiles every one of the 6,886 regexes,
// which the parser otherwise compiles only when a line first needs one.
#[test]
fn registry_matches_the_prototype() {
    let reg = common::registry();
    let want: Vec<Json> =
        common::read_gz_lines(&common::fixtures().join("parse-golden/registry.jsonl.gz"))
            .iter()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
    let got = reg.patterns();
    assert_eq!(got.len(), want.len());
    let mut generated = 0;
    for (p, w) in got.iter().zip(&want) {
        let what = format!("pattern {} ({} {:?})", w["order"], w["name"], w["text"]);
        assert_eq!(p.name, w["name"].as_str().unwrap(), "{what}");
        assert_eq!(p.cat, w["cat"].as_str().unwrap(), "{what}");
        assert_eq!(p.prio as i64, w["prio"].as_i64().unwrap(), "{what}");
        assert_eq!(p.spec as u64, w["spec"].as_u64().unwrap(), "{what}");
        assert_eq!(p.key.as_deref(), w["key"].as_str(), "{what}");
        assert!(
            p.regex().is_some(),
            "{what}: does not compile: {}",
            p.source
        );
        // The weapon-verb alternations list equally long verbs in Python's set order, which
        // changes from run to run: compare those as sets of alternatives.
        let rx = w["rx"].as_str().unwrap();
        if p.source != rx {
            // (the text around the verb group, the verbs sorted)
            let alts = |s: &str| {
                let (head, rest) = s.split_once("(?P<verb>").unwrap();
                let (verbs, tail) = rest.split_once(')').unwrap();
                let mut v: Vec<&str> = verbs.split('|').collect();
                v.sort_unstable();
                (head.to_owned(), v.join("|"), tail.to_owned())
            };
            assert!(
                p.template.is_none() && p.source.contains("(?P<verb>"),
                "{what}: {} != {rx}",
                p.source
            );
            assert_eq!(alts(&p.source), alts(rx), "{what}");
        }
        if w["generated"].as_bool().unwrap() {
            generated += 1;
            assert_eq!(p.template.as_deref(), w["text"].as_str(), "{what}");
            assert_eq!(p.msgnum.map(i64::from), w["msgnum"].as_i64(), "{what}");
            let text = p.template.as_deref().unwrap();
            let own = p.fullmatch(text).map(|c| {
                let mut f = serde_json::Map::new();
                for name in p.regex().unwrap().capture_names().flatten() {
                    if let Some(m) = c.name(name) {
                        f.insert(name.to_owned(), Json::String(m.as_str().to_owned()));
                    }
                }
                Json::Object(f)
            });
            assert_eq!(
                own.as_ref().unwrap_or(&Json::Null),
                &w["self_match"],
                "{what}: its own text"
            );
        }
    }
    assert_eq!(generated, 6443);
}

#[test]
fn static_examples_match_their_own_pattern() {
    // As `patterns.selftest`: an example may be taken by a more specific pattern of another
    // category, never by a less specific one or by nothing.
    let reg = common::registry();
    let mut bad = Vec::new();
    for own in reg.patterns().iter().filter(|p| p.template.is_none()) {
        if own.example.is_empty() || own.example.contains("...") || own.seen == "I" {
            continue;
        }
        match reg.match_line(own.example.trim()) {
            None => bad.push(format!("{}: nothing for {:?}", own.name, own.example)),
            Some((p, _)) if p.name != own.name && p.prio >= own.prio && p.cat != own.cat => {
                bad.push(format!("{}: {} for {:?}", own.name, p.name, own.example))
            }
            Some(_) => {}
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
    for t in reg.table_rows() {
        assert!(
            t.fullmatch(t.example.trim()).is_some(),
            "{} {:?}",
            t.name,
            t.example
        );
    }
}
