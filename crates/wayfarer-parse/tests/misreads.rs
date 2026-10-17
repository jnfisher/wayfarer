//! The prototype bot's misreads (`kb/plan-patterns-migration.md`, "Why bother"; the audit in
//! `kb/screen-patterns.md` section 6), each with the events that fix it.

mod common;

use std::sync::Arc;
use wayfarer_parse::helpers::{parse_also_here, parse_exits};

use wayfarer_parse::{Event, Parser, Registry, Value};

fn events_with(reg: &Arc<Registry>, text: &str) -> Vec<Event> {
    let mut p = Parser::new(reg.clone());
    let mut out = p.feed(text);
    out.extend(p.finish());
    out
}

fn events(text: &str) -> Vec<Event> {
    events_with(&common::registry(), text)
}

fn names(evs: &[Event]) -> Vec<&str> {
    evs.iter().map(|e| e.name.as_str()).collect()
}

#[test]
fn titles_glued_behind_a_mana_or_resting_prompt() {
    let reg = common::registry();
    for (text, mana, state) in [
        ("[HP=12/MA=3]:Town Gates\r\n", Some(3), Value::Null),
        ("[HP=40 (Resting) ]:Town Gates\r\n", None, "Resting".into()),
        (
            "[HP=12/KAI=3]: (Meditating) Town Gates\r\n",
            Some(3),
            "Meditating".into(),
        ),
    ] {
        let evs = events_with(&reg, text);
        assert_eq!(names(&evs), ["prompt", "room_title"], "{text:?}");
        assert_eq!(evs[0].field("mana").and_then(Value::as_int), mana);
        assert_eq!(evs[0].field("state"), Some(&state));
        assert_eq!(evs[1].text("name"), Some("Town Gates"));
    }
}

#[test]
fn trap_doors_up_and_down_are_exits() {
    // wrapped at 79 columns after "trap": joined back
    let evs = events("Obvious exits: north, closed trap\r\ndoor above, open trap door below\r\n");
    assert_eq!(names(&evs), ["room_exits"]);
    let exits = parse_exits(evs[0].text("exits").unwrap());
    let dirs: Vec<(Option<&str>, &str)> = exits
        .iter()
        .map(|e| (e.dir.as_deref(), e.kind.as_str()))
        .collect();
    assert_eq!(
        dirs,
        [
            (Some("north"), "normal"),
            (Some("up"), "trap door"),
            (Some("down"), "trap door")
        ]
    );
}

#[test]
fn real_death_text_is_seen() {
    let evs = events(
        "Corvin drops to the ground!\r\nYou have been killed!\r\nYou have 8 lives left.\r\n",
    );
    assert_eq!(
        names(&evs),
        ["drops_unconscious", "you_killed", "lives_left"]
    );
    assert_eq!(evs[0].text("who"), Some("Corvin"));
    assert_eq!(evs[2].text("lives"), Some("8"));
}

#[test]
fn attacks_on_you_are_monster_events_not_any_you() {
    let evs = events(
        "The acid slime whips you with its pseudopod for 2 damage!\r\nThe fat giant rat lunges at you!\r\nThe giant rat creeps in after you!\r\n",
    );
    assert_eq!(names(&evs)[..2], ["mon_hits_you", "mon_miss_you"]);
    assert_eq!(evs[0].text("mon"), Some("acid slime"));
    assert_eq!(evs[0].text("dmg"), Some("2"));
    assert_eq!(evs[1].text("base"), Some("giant rat"));
    assert_eq!(evs[1].text("adj"), Some("fat"));
    // ATTACKED_RE took this for an attack: it is the monster following us
    assert!(evs[2].name.starts_with("monster_follows"), "{:?}", evs[2]);
}

#[test]
fn a_light_going_out_is_noticed() {
    let evs = events("torch is no longer lit!\r\n");
    assert_eq!(names(&evs), ["light_out"]);
    assert_eq!(evs[0].text("item"), Some("torch"));
}

#[test]
fn kai_is_read_in_the_status_screen() {
    let evs = events("Kai:      5/9                          Traps:           0\r\n");
    assert_eq!(names(&evs), ["st_mana"]);
    assert_eq!(evs[0].text("mtype"), Some("Kai"));
    assert_eq!(evs[0].text("mana"), Some("5"));
    assert_eq!(evs[0].text("maxmana"), Some("9"));
}

#[test]
fn a_shop_refusing_junk_is_seen() {
    let evs = events("You cannot sell padded gloves here.\r\n");
    assert_eq!(names(&evs), ["sell_refused"]);
    assert_eq!(evs[0].text("item"), Some("padded gloves"));
}

#[test]
fn move_refusals_are_seen() {
    let evs = events(concat!(
        "There is no exit in that direction!\r\n",
        "The gate is closed!\r\n",
        "There is a closed door in that direction!\r\n",
        "You do not have the appropriate item to go that direction!\r\n",
        "You have not progressed far enough to go through this exit!\r\n",
        "You are too heavy to move!\r\n",
        "You may not enter that room while in combat.\r\n",
        "You are not permitted in that room!\r\n",
    ));
    assert_eq!(
        names(&evs),
        [
            "no_exit",
            "door_closed",
            "door_closed_drag",
            "need_item",
            "exit_level_low",
            "too_heavy",
            "exit_combat",
            "not_permitted"
        ]
    );
}

#[test]
fn only_real_attacks_are_attacks() {
    let evs = events(concat!(
        "You punch fat giant rat for 2 damage!\r\n",
        "You swing at carrion beast!\r\n",
        "You swing at dark monk who dodges your attack!\r\n",
        "Your swing at fat giant rat hits, but glances off its armour.\r\n",
        "You cast vine strike at acid slime for 7 damage!\r\n",
        "You attempt to cast vine strike, but fail.\r\n",
        "You gain 13 experience.\r\n",
    ));
    assert_eq!(
        names(&evs),
        [
            "you_hit",
            "you_miss",
            "you_dodged",
            "you_glance",
            "spell_cast_you",
            "cast_fail",
            "exp_gain"
        ]
    );
    assert_eq!(evs[0].text("verb"), Some("punch"));
    assert_eq!(evs[4].text("dmg"), Some("7"));
}

#[test]
fn capitalised_monsters_in_also_here() {
    let mut p = Parser::new(common::registry());
    let mut evs = p.feed("Also here: Sheriff Lionheart, Thornwyn, Aiken,\r\ngiant rat.\r\n");
    evs.extend(p.finish());
    assert_eq!(names(&evs), ["room_also_here"]);
    let known = p.registry().monster_names();
    let present = parse_also_here(evs[0].text("who").unwrap().trim_end_matches('.'));
    let kinds: Vec<(&str, bool)> = present
        .iter()
        .map(|w| {
            (
                w.name.as_str(),
                w.player && !known.contains(&w.name.to_lowercase()),
            )
        })
        .collect();
    assert_eq!(
        kinds,
        [
            ("Sheriff Lionheart", false),
            ("Thornwyn", true),
            ("Aiken", false),
            ("giant rat", false)
        ]
    );
}
