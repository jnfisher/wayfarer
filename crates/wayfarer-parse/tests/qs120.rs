//! Quicksilver 1.20's lines (card C-063): every line of `tests/fixtures/qs120/lines.txt`
//! classifies as named, the `help 1.20` answer reads as no 1.20 event, and no stock text is
//! taken by a 1.20 pattern. The stock captures and transcripts still give exactly the
//! prototype's events: that is `golden.rs`, run on the same registry.

mod common;

use std::sync::{Arc, OnceLock};

use wayfarer_parse::typed::GameEvent;
use wayfarer_parse::{Event, Parser, Registry};

fn reg() -> &'static Arc<Registry> {
    static REG: OnceLock<Arc<Registry>> = OnceLock::new();
    REG.get_or_init(common::registry)
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(common::fixtures().join("qs120").join(name)).unwrap()
}

fn events(text: &str) -> Vec<Event> {
    let mut p = Parser::new(reg().clone());
    let mut evs = p.feed(&format!("{text}\n"));
    evs.extend(p.finish());
    evs
}

fn one(line: &str) -> Event {
    let evs = events(line);
    assert_eq!(evs.len(), 1, "{line}: {evs:?}");
    evs.into_iter().next().unwrap()
}

fn typed(line: &str) -> GameEvent {
    GameEvent::from_event(&one(line))
}

#[test]
fn every_line_classifies_as_named() {
    let mut bad = Vec::new();
    for row in fixture("lines.txt").lines().filter(|l| !l.starts_with('#')) {
        let (line, name) = row.split_once('\t').unwrap();
        let ev = one(line);
        if ev.name != name {
            bad.push(format!("{line:?}: {} (want {name})", ev.name));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn the_patterns_are_marked_and_rank_first() {
    let qs: Vec<_> = reg()
        .patterns()
        .iter()
        .filter(|p| p.rules.is_some())
        .collect();
    // 22 lines of section 3, 16 regen-line texts of 3.8 (C-066).
    assert_eq!(qs.len(), 38);
    for p in qs {
        assert_eq!(p.rules.as_deref(), Some("1.20"));
        let want = if p.name.ends_with("_victim") { 39 } else { 40 };
        assert_eq!(p.prio, want, "{}", p.name);
        assert!(p.regex().is_some(), "{} compiles", p.name);
    }
}

#[test]
fn no_stock_text_matches_a_qs120_pattern() {
    // Every stock static example and every game text the registry was built from.
    let qs: Vec<_> = reg()
        .patterns()
        .iter()
        .filter(|p| p.rules.is_some())
        .collect();
    let mut bad = Vec::new();
    for p in reg().patterns().iter().filter(|p| p.rules.is_none()) {
        let text = p.template.as_deref().unwrap_or(&p.example).trim();
        for q in &qs {
            if q.fullmatch(text).is_some() {
                bad.push(format!("{} {text:?} matches {}", p.name, q.name));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn the_help_answer_holds_no_qs120_event() {
    let help = fixture("help-1.20.txt");
    assert_eq!(help.lines().count(), 34);
    assert_eq!(
        help.lines().next(),
        Some("Quicksilver 1.20: how the rules differ from 1.11p")
    );
    for ev in events(&help) {
        let rules = reg()
            .patterns()
            .iter()
            .find(|p| p.name == ev.name)
            .and_then(|p| p.rules.clone());
        assert_eq!(rules, None, "{:?} read as {}", ev.line, ev.name);
    }
}

#[test]
fn a_critical_stab_is_a_crit_and_a_stab() {
    assert_eq!(
        typed("You critically surprise slash giant rat for 140 damage!"),
        GameEvent::YouHit {
            target: "giant rat".into(),
            damage: 140,
            crit: true,
            backstab: true,
        }
    );
    let ev = one("Thornwyn critically surprise slashes giant rat for 140 damage!");
    assert_eq!(ev.name, "other_hits");
    assert_eq!(ev.text("crit"), Some("critically"));
    assert_eq!(ev.text("surprise"), Some("surprise"));
    assert_eq!(ev.text("who"), Some("Thornwyn"));
}

#[test]
fn openings() {
    assert_eq!(
        typed("You find an opening and slash giant rat for 31 damage!"),
        GameEvent::Opening {
            who: None,
            target: Some("giant rat".into()),
            verb: "slash".into(),
            damage: 31,
        }
    );
    assert_eq!(
        typed("Thornwyn finds an opening and slashes giant rat for 23 damage!"),
        GameEvent::Opening {
            who: Some("Thornwyn".into()),
            target: Some("giant rat".into()),
            verb: "slashes".into(),
            damage: 23,
        }
    );
    assert_eq!(
        typed("Thornwyn finds an opening and slashes you for 23 damage!"),
        GameEvent::Opening {
            who: Some("Thornwyn".into()),
            target: None,
            verb: "slashes".into(),
            damage: 23,
        }
    );
}

#[test]
fn weaves() {
    let weave = |who: Option<&str>, target: Option<&str>, damage, resisted| GameEvent::Weave {
        who: who.map(Into::into),
        target: target.map(Into::into),
        spell: "meteor swarm".into(),
        damage,
        resisted,
    };
    assert_eq!(
        typed("You weave meteor swarm into giant rat for 75 damage!"),
        weave(None, Some("giant rat"), 75, false)
    );
    assert_eq!(
        typed("You weave meteor swarm into giant rat for 0 damage!"),
        weave(None, Some("giant rat"), 0, false)
    );
    assert_eq!(
        typed("Thornwyn weaves meteor swarm into giant rat for 75 damage!"),
        weave(Some("Thornwyn"), Some("giant rat"), 75, false)
    );
    assert_eq!(
        typed("Thornwyn weaves meteor swarm into you for 75 damage!"),
        weave(Some("Thornwyn"), None, 75, false)
    );
    assert_eq!(
        typed("You weave meteor swarm into giant rat, but it is resisted!"),
        weave(None, Some("giant rat"), 0, true)
    );
    assert_eq!(
        typed("Thornwyn weaves meteor swarm into giant rat, but it is resisted!"),
        weave(Some("Thornwyn"), Some("giant rat"), 0, true)
    );
    assert_eq!(
        typed("Thornwyn weaves meteor swarm into you, but you resist!"),
        weave(Some("Thornwyn"), None, 0, true)
    );
}

#[test]
fn a_spell_named_with_into_splits_on_the_last_into() {
    assert_eq!(
        typed("You weave plunge into darkness into giant rat for 9 damage!"),
        GameEvent::Weave {
            who: None,
            target: Some("giant rat".into()),
            spell: "plunge into darkness".into(),
            damage: 9,
            resisted: false,
        }
    );
}

#[test]
fn intercepts() {
    assert_eq!(
        typed("You intercept the orc's attack on Mirelle!"),
        GameEvent::Intercept {
            guard: None,
            monster: "orc".into(),
            ward: Some("Mirelle".into()),
        }
    );
    assert_eq!(
        typed("Brannoc intercepts the orc's attack!"),
        GameEvent::Intercept {
            guard: Some("Brannoc".into()),
            monster: "orc".into(),
            ward: None,
        }
    );
    assert_eq!(
        typed("Brannoc intercepts the orc's attack on Mirelle!"),
        GameEvent::Intercept {
            guard: Some("Brannoc".into()),
            monster: "orc".into(),
            ward: Some("Mirelle".into()),
        }
    );
}

#[test]
fn guard_lines() {
    let g = |guard: Option<&str>, ward: Option<&str>, on| GameEvent::Guarding {
        guard: guard.map(Into::into),
        ward: ward.map(Into::into),
        on,
    };
    assert_eq!(
        typed("You are now guarding Mirelle."),
        g(None, Some("Mirelle"), true)
    );
    assert_eq!(
        typed("Brannoc is now guarding you."),
        g(Some("Brannoc"), None, true)
    );
    assert_eq!(
        typed("You stop guarding Mirelle."),
        g(None, Some("Mirelle"), false)
    );
    assert_eq!(
        typed("Brannoc stops guarding you."),
        g(Some("Brannoc"), None, false)
    );
    for (line, why) in [
        ("You are not guarding anyone.", "not_guarding"),
        ("You don't see that player here.", "not_here"),
        ("Mirelle is not in your party.", "not_party"),
        ("You cannot guard yourself.", "self"),
        ("You cannot guard while resting or meditating.", "resting"),
    ] {
        assert_eq!(typed(line), GameEvent::GuardRefused { why }, "{line}");
    }
}

#[test]
fn the_str_requirement() {
    assert_eq!(
        typed("It requires 75 strength to wield well (you have 92)."),
        GameEvent::StrRequirement {
            required: 75,
            have: 92
        }
    );
}

#[test]
fn the_regen_line_texts() {
    for (line, family, kind, who) in [
        (
            "You attune your mind to the flow of mana.",
            "attunement",
            "cast",
            None,
        ),
        (
            "Thornwyn bows their head in quiet devotion.",
            "devotion",
            "room",
            Some("Thornwyn"),
        ),
        (
            "Your communion with nature ends.",
            "communion",
            "ends",
            None,
        ),
        ("A dreaming melody fills your mind.", "song", "status", None),
        (
            "Thornwyn hums a slow, dreaming melody.",
            "song",
            "room",
            Some("Thornwyn"),
        ),
    ] {
        assert_eq!(
            typed(line),
            GameEvent::RegenLine {
                family: family.into(),
                kind: kind.into(),
                who: who.map(Into::into),
            },
            "{line}"
        );
    }
}
