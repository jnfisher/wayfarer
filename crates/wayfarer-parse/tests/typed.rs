//! Typed events from fixture lines: the captures and transcripts as the parser reads them, and
//! for the events no fixture has yet, the static table's own example lines
//! (`tests/fixtures/parse-golden/patterns.json`).

mod common;

use std::sync::{Arc, OnceLock};

use wayfarer_parse::helpers::{Carried, Exit};
use wayfarer_parse::typed::{GameEvent, ManaKind, PromptForm, Rank, Refusal, Rest};
use wayfarer_parse::{Event, Parser, PromptMode, PromptParser, Registry};

fn reg() -> &'static Arc<Registry> {
    static REG: OnceLock<Arc<Registry>> = OnceLock::new();
    REG.get_or_init(common::registry)
}

/// Every event of a fixture (`capture-status-screens`, `transcript-thornwyn-early`, ...).
fn events(fixture: &str) -> Vec<Event> {
    common::run(reg(), &common::input(fixture))
}

/// The typed form of the first event in `fixture` named `name` whose line contains `has`.
fn first(fixture: &str, name: &str, has: &str) -> GameEvent {
    let evs = events(fixture);
    let ev = evs
        .iter()
        .find(|e| e.name == name && e.line.contains(has))
        .unwrap_or_else(|| panic!("{fixture}: no {name} with {has:?}"));
    GameEvent::from_event(ev)
}

/// The typed events of one line, fed whole.
fn line(text: &str) -> Vec<GameEvent> {
    let mut p = Parser::new(reg().clone());
    let mut evs = p.feed(&format!("{text}\n"));
    evs.extend(p.finish());
    evs.iter().map(GameEvent::from).collect()
}

/// A static pattern's example line from patterns.json, typed.
fn example(name: &str) -> GameEvent {
    let p = reg()
        .patterns()
        .iter()
        .find(|p| p.name == name && p.template.is_none())
        .unwrap_or_else(|| panic!("no static pattern {name}"));
    let evs = line(&p.example);
    assert_eq!(evs.len(), 1, "{name}: {evs:?}");
    evs.into_iter().next().unwrap()
}

#[test]
fn status_screen() {
    let f = "capture-death-rebirth";
    assert_eq!(
        first(f, "st_name", "Corvin"),
        GameEvent::StatusName {
            name: "Corvin Ashdown".into(),
            lives: Some(8),
            cp: Some(80)
        }
    );
    assert_eq!(
        first(f, "st_race", "Elf"),
        GameEvent::StatusRace {
            race: "Elf".into(),
            exp: Some(52),
            perception: Some(56)
        }
    );
    assert_eq!(
        first(f, "st_class", "Mage"),
        GameEvent::StatusClass {
            class: "Mage".into(),
            level: Some(1),
            stealth: Some(34)
        }
    );
    assert_eq!(
        first(f, "st_hits", "20/20"),
        GameEvent::StatusHits {
            hp: Some(20),
            hp_max: Some(20),
            ac: Some(0),
            dr: Some(0),
            thievery: Some(0)
        }
    );
    assert_eq!(
        first(f, "st_mana", "22/22"),
        GameEvent::StatusMana {
            kind: Some(ManaKind::Mana),
            mana: Some(22),
            mana_max: Some(22),
            spellcasting: Some(55),
            traps: Some(0)
        }
    );
    let stat = |name: &str, has: &str| match first(f, name, has) {
        GameEvent::StatusStat { stats } => stats,
        other => panic!("{other:?}"),
    };
    let s = |v: &[(&str, i64)]| -> Vec<(String, i64)> {
        v.iter().map(|(k, n)| (k.to_string(), *n)).collect()
    };
    assert_eq!(
        stat("st_str", "Strength"),
        s(&[("Strength", 35), ("Agility", 50), ("Tracking", 0)])
    );
    assert_eq!(
        stat("st_int", "Intellect"),
        s(&[("Intellect", 60), ("Health", 30), ("Martial Arts", 16)])
    );
    assert_eq!(
        stat("st_wil", "Willpower"),
        s(&[("Willpower", 50), ("Charm", 50), ("MagicRes", 52)])
    );
    assert_eq!(stat("st_picklocks", "Picklocks"), s(&[("Picklocks", 0)]));
    assert_eq!(
        match first("capture-status-screens", "st_traps", "Traps") {
            GameEvent::StatusStat { stats } => stats,
            other => panic!("{other:?}"),
        },
        s(&[("Traps", 0)])
    );
}

#[test]
fn exp_and_health_screens() {
    assert_eq!(
        first("capture-fight-arena", "exp", "Exp: 9 "),
        GameEvent::ExpScreen {
            exp: Some(9),
            level: Some(1),
            needed: Some(991),
            next_total: Some(1000),
            percent: Some(0)
        }
    );
    assert_eq!(
        first("capture-status-screens", "health", "Health"),
        GameEvent::Health {
            hp: Some(35),
            hp_max: Some(35),
            mana: None,
            mana_max: None
        }
    );
}

#[test]
fn prompts() {
    assert_eq!(
        first("capture-death-rebirth", "prompt", "[HP=20/MA=22]:"),
        GameEvent::Prompt {
            hp: Some(20),
            hp_max: None,
            mana: Some(22),
            mana_max: None,
            mana_kind: Some(ManaKind::Mana),
            resting: Some(Rest::Awake),
            form: PromptForm::Std,
            exp: None,
            wealth: None,
        }
    );
    // Negative HP while dying.
    let GameEvent::Prompt { hp, .. } = first("capture-death-rebirth", "prompt", "HP=-203") else {
        panic!()
    };
    assert_eq!(hp, Some(-203));
    let GameEvent::Prompt { resting, .. } = first("capture-fight-arena", "prompt", "Resting")
    else {
        panic!()
    };
    assert_eq!(resting, Some(Rest::Resting));
}

/// The fixtures only have the standard prompt; the other forms from their own text.
#[test]
fn prompt_forms() {
    let typed = |prompt: PromptParser, text: &str| -> GameEvent {
        let mut p = Parser::with_prompt(reg().clone(), prompt);
        let evs = p.feed(text);
        let ev = evs.iter().find(|e| e.name == "prompt").expect("a prompt");
        GameEvent::from_event(ev)
    };
    let custom = PromptParser::custom("[%h/%H %m/%M x%x c%c%r]:").unwrap();
    assert_eq!(
        typed(custom, "[20/35 4/9 x1712 c223 (Meditating) ]:"),
        GameEvent::Prompt {
            hp: Some(20),
            hp_max: Some(35),
            mana: Some(4),
            mana_max: Some(9),
            mana_kind: None,
            resting: Some(Rest::Meditating),
            form: PromptForm::Custom,
            exp: Some(1712),
            wealth: Some(223),
        }
    );
    // A custom template without %r says nothing about resting.
    let GameEvent::Prompt { resting, .. } = typed(PromptParser::custom("<%h>").unwrap(), "<12>")
    else {
        panic!()
    };
    assert_eq!(resting, None);
    assert_eq!(
        typed(
            PromptParser::new(PromptMode::Auto),
            "[Hp=12/30 Ma=3/9 Cash=40]: (Resting) "
        ),
        GameEvent::Prompt {
            hp: Some(12),
            hp_max: Some(30),
            mana: Some(3),
            mana_max: Some(9),
            mana_kind: None,
            resting: Some(Rest::Resting),
            form: PromptForm::Generic,
            exp: None,
            wealth: Some(40),
        }
    );
    assert_eq!(
        typed(PromptParser::new(PromptMode::Off), ">"),
        GameEvent::Prompt {
            hp: None,
            hp_max: None,
            mana: None,
            mana_max: None,
            mana_kind: None,
            resting: None,
            form: PromptForm::Off,
            exp: None,
            wealth: None,
        }
    );
}

#[test]
fn room() {
    assert_eq!(
        first("capture-room-walk", "room_title", "Narrow Road"),
        GameEvent::RoomTitle {
            title: "Newhaven, Narrow Road".into()
        }
    );
    let GameEvent::RoomExits { exits } = first("capture-room-walk", "room_exits", "down") else {
        panic!()
    };
    let dirs: Vec<_> = exits.iter().filter_map(|e| e.dir.as_deref()).collect();
    assert_eq!(dirs, ["north", "east", "west", "down"]);
    assert_eq!(
        exits[0],
        Exit {
            dir: Some("north".into()),
            kind: "normal".into(),
            state: None
        }
    );
    assert_eq!(
        first("capture-room-walk", "room_dark", "very dark"),
        GameEvent::RoomDark
    );
}

#[test]
fn moves_and_doors() {
    let f = "capture-flood-and-refusals";
    assert_eq!(
        first(f, "no_exit", ""),
        GameEvent::MoveRefused {
            reason: Refusal::NoExit
        }
    );
    // The stock state: locked.
    assert_eq!(
        first("capture-room-walk", "door_closed", ""),
        GameEvent::DoorClosed { locked: true }
    );
    assert_eq!(
        first("capture-room-walk", "door_locked", ""),
        GameEvent::OpenRefused
    );
    assert_eq!(
        line("The door is Closed!"),
        [GameEvent::DoorClosed { locked: false }]
    );
    assert_eq!(
        line("The gate is closed!"),
        [GameEvent::DoorClosed { locked: true }]
    );
    assert_eq!(line("The door is now open."), [GameEvent::DoorOpened]);
    assert_eq!(line("The gate is already open."), [GameEvent::DoorOpened]);
    assert_eq!(
        line("You bashed the door open."),
        [GameEvent::BashResult { ok: true }]
    );
    assert_eq!(
        line("Your attempts to bash through fail!"),
        [GameEvent::BashResult { ok: false }]
    );
    for (text, found, dir) in [
        ("You found an exit to the north!", true, "north"),
        ("You found an exit upwards!", true, "up"),
        ("You found an exit downwards!", true, "down"),
        (
            "You notice nothing different to the southwest.",
            false,
            "southwest",
        ),
        ("You notice nothing different above you.", false, "up"),
        ("You notice nothing different below you.", false, "down"),
        // As silvermere sent it (C-037 live check): a letter and a backspace in the name.
        (
            "You notice nothing different to the sN\x08outh.",
            false,
            "south",
        ),
    ] {
        let dir = dir.to_owned();
        assert_eq!(
            line(text),
            [GameEvent::SearchResult { found, dir }],
            "{text}"
        );
    }
    for (text, reason) in [
        (
            "You have progressed too far to go through this exit!",
            Refusal::LevelHigh,
        ),
        (
            "You have not progressed far enough to go through this exit!",
            Refusal::LevelLow,
        ),
        ("You are not permitted in that room!", Refusal::NotPermitted),
        ("You may not go through this exit!", Refusal::Forbidden),
        (
            "You are too evil to go through this exit!",
            Refusal::Alignment,
        ),
        (
            "You need to cast a spell to go that way!",
            Refusal::NeedSpell,
        ),
        (
            "You do not have the appropriate item to go that direction!",
            Refusal::NeedItem,
        ),
        (
            "You do not have enough to cover the toll of 5 gold crowns.",
            Refusal::TollShort,
        ),
        (
            "You may not pass through that exit at this point in time.",
            Refusal::Timed,
        ),
    ] {
        assert_eq!(line(text), [GameEvent::MoveRefused { reason }], "{text}");
    }
}

#[test]
fn combat_and_rest() {
    let f = "capture-death-rebirth";
    assert_eq!(first(f, "combat_engaged", ""), GameEvent::CombatEngaged);
    assert_eq!(first(f, "combat_off", ""), GameEvent::CombatOff);
    assert_eq!(
        first(f, "monster_death", "giant rat"),
        GameEvent::MonsterDeath {
            monsters: vec!["Aderem".into(), "Cygani".into(), "giant rat".into()]
        }
    );
    assert_eq!(first(f, "exp_gain", "9"), GameEvent::ExpGain { amount: 9 });
    assert_eq!(
        first(f, "rest_start", ""),
        GameEvent::Resting { meditating: false }
    );
    assert_eq!(
        first("capture-fight-arena", "healed_paid", ""),
        GameEvent::Healed
    );
    // Not in any capture or transcript yet: the table's examples.
    assert_eq!(example("took_damage"), GameEvent::TookDamage { amount: 3 });
    assert_eq!(
        example("meditate_start"),
        GameEvent::Resting { meditating: true }
    );
    assert_eq!(example("meditate_done"), GameEvent::RestEnd);
}

#[test]
fn the_characters_own_swings() {
    assert_eq!(
        line("You punch fat giant rat for 2 damage!"),
        [GameEvent::YouHit {
            target: "fat giant rat".into(),
            damage: 2,
            crit: false,
            backstab: false,
        }]
    );
    // A critical hit and a backstab (`kasimir-early.txt`; the `surprise` form, `patterns.py`).
    assert_eq!(
        line("You critically beat filthbug for 34 damage!"),
        [GameEvent::YouHit {
            target: "filthbug".into(),
            damage: 34,
            crit: true,
            backstab: false,
        }]
    );
    assert_eq!(
        line("You surprise smack kobold thief for 40 damage!"),
        [GameEvent::YouHit {
            target: "kobold thief".into(),
            damage: 40,
            crit: false,
            backstab: true,
        }]
    );
    assert_eq!(
        line("You critically surprise smack kobold thief for 90 damage!"),
        [GameEvent::YouHit {
            target: "kobold thief".into(),
            damage: 90,
            crit: true,
            backstab: true,
        }]
    );
    assert_eq!(
        line("You swing at carrion beast!"),
        [GameEvent::YouMiss {
            target: "carrion beast".into(),
            dodged: false,
        }]
    );
    assert_eq!(
        line("You swing at dark monk who dodges your attack!"),
        [GameEvent::YouMiss {
            target: "dark monk".into(),
            dodged: true,
        }]
    );
    // The line of the M3 soak's stalemate, confirmed live 2026-10-20 (`kb/combat.md`).
    assert_eq!(
        line("Your swing at gigantic black ooze hits, but glances off its armour."),
        [GameEvent::SwingGlanced {
            target: "gigantic black ooze".into()
        }]
    );
    assert_eq!(
        example("you_glance"),
        GameEvent::SwingGlanced { target: "Y".into() }
    );
}

#[test]
fn who_is_here_and_who_attacks() {
    let f = "capture-death-rebirth";
    assert_eq!(
        first(f, "room_also_here", "fat"),
        GameEvent::AlsoHere {
            players: vec![],
            monsters: vec!["fat giant rat".into()]
        }
    );
    assert_eq!(
        example("room_also_here"),
        GameEvent::AlsoHere {
            players: vec!["Thornwyn".into()],
            monsters: vec!["giant rat".into()]
        }
    );
    // The monster's own arrival text, and the generic one.
    assert_eq!(
        first(f, "monster_arrives", "thin"),
        GameEvent::MonsterArrives {
            name: "thin giant rat".into()
        }
    );
    assert_eq!(
        example("monster_arrives_generic"),
        GameEvent::MonsterArrives {
            name: "goblin".into()
        }
    );
    assert_eq!(
        line("The thin giant rat just left downwards."),
        [GameEvent::MonsterLeaves {
            name: "thin giant rat".into()
        }]
    );
    // `The fat giant rat lunges at you!`: a miss, without the article.
    assert_eq!(
        first(f, "mon_miss_you", "fat"),
        GameEvent::AttackedBy {
            name: "fat giant rat".into()
        }
    );
    assert_eq!(
        example("mon_hits_you_generic"),
        GameEvent::AttackedBy { name: "orc".into() }
    );
    assert_eq!(
        example("attacks_you"),
        GameEvent::AttackedBy { name: "Bob".into() }
    );
    assert_eq!(
        example("target_missing"),
        GameEvent::YouDoNotSee {
            target: "rat".into()
        }
    );
    assert_eq!(
        line("You don't see your target here!"),
        [GameEvent::YouDoNotSee {
            target: String::new()
        }]
    );
    assert_eq!(example("no_effect_weapon"), GameEvent::NoEffect);
    assert_eq!(example("rest_sick"), GameEvent::TooSick);
}

#[test]
fn death_and_conditions() {
    let f = "capture-death-rebirth";
    assert_eq!(
        first(f, "drops_unconscious", "Corvin"),
        GameEvent::Unconscious {
            who: "Corvin".into()
        }
    );
    assert_eq!(first(f, "you_killed", ""), GameEvent::YouKilled);
    assert_eq!(
        first(f, "lives_left", "8"),
        GameEvent::Dead {
            lives_left: Some(8)
        }
    );
    assert_eq!(
        first("transcript-thornwyn-early", "lives_left", "6"),
        GameEvent::Dead {
            lives_left: Some(6)
        }
    );
    // Not in any capture or transcript yet: the table's examples.
    assert_eq!(
        example("no_lives"),
        GameEvent::Dead {
            lives_left: Some(0)
        }
    );
    assert_eq!(example("st_poisoned"), GameEvent::Poisoned);
    assert_eq!(example("poison_tick"), GameEvent::PoisonTick);
    assert_eq!(example("cured_paid"), GameEvent::Cured);
    assert_eq!(example("blind"), GameEvent::Blind);
    assert_eq!(example("stunned"), GameEvent::Stunned);
}

#[test]
fn inventory() {
    let f = "capture-status-screens";
    let GameEvent::Inventory { coins, items } = first(f, "inv_carrying", "padded vest") else {
        panic!()
    };
    assert!(coins.is_empty());
    assert_eq!(items.len(), 6);
    assert_eq!(
        items[5],
        Carried {
            name: "quarterstaff".into(),
            slot: Some("Two handed".into()),
            count: 1
        }
    );
    assert_eq!(
        first(f, "inv_wealth", "Wealth"),
        GameEvent::Wealth { coins: 0 }
    );
    assert_eq!(
        first(f, "inv_encumbrance", "500"),
        GameEvent::Encumbrance {
            current: Some(500),
            max: Some(2400),
            level: "Light".into(),
            percent: Some(20)
        }
    );
    assert_eq!(
        first("capture-death-rebirth", "inv_nothing", ""),
        GameEvent::Inventory {
            coins: Default::default(),
            items: vec![]
        }
    );
    // Coins in the list, and wealth, from a transcript.
    let f = "transcript-kasimir-early";
    let GameEvent::Inventory { coins, items } = first(f, "inv_carrying", "18 silver nobles") else {
        panic!()
    };
    assert_eq!(
        coins.into_iter().collect::<Vec<_>>(),
        [
            ("copper farthings".to_string(), 43),
            ("silver nobles".to_string(), 18)
        ]
    );
    assert_eq!(items.len(), 6);
    assert_eq!(
        first(f, "inv_wealth", "223"),
        GameEvent::Wealth { coins: 223 }
    );
}

#[test]
fn unknown_and_other() {
    let mut unknown = 0;
    let mut other = 0;
    for name in common::fixture_names() {
        for ev in events(&name) {
            match GameEvent::from_event(&ev) {
                GameEvent::Unknown { line } => {
                    assert_eq!(ev.cat, "unclassified", "{name}: {ev:?}");
                    assert_eq!(line, ev.line);
                    unknown += 1;
                }
                GameEvent::Other(kept) => {
                    assert_ne!(ev.cat, "unclassified");
                    assert_eq!(kept, ev);
                    other += 1;
                }
                _ => assert_ne!(ev.cat, "unclassified"),
            }
        }
    }
    // patterns.py's count of unknown events over the fixtures (tests/golden.rs), less the own
    // `par` row after `party_none` that this parser reads (C-038).
    assert_eq!(unknown, 356);
    assert!(other > 1_000);
    assert_eq!(
        first("capture-status-screens", "unknown", "Username"),
        GameEvent::Unknown {
            line: "Username: Aldric".into()
        }
    );
}

#[test]
fn coins_on_the_floor_and_picked_up() {
    let got = line("You notice 12 gold crowns, 3 silver nobles, torch here.");
    let GameEvent::RoomItems { coins, items } = &got[0] else {
        panic!("{got:?}")
    };
    assert_eq!(
        coins
            .iter()
            .map(|(c, n)| (c.as_str(), *n))
            .collect::<Vec<_>>(),
        [("gold crowns", 12), ("silver nobles", 3)]
    );
    assert_eq!(
        items.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(),
        ["torch"]
    );
    assert_eq!(
        example("picked_up_coins"),
        GameEvent::CoinsPicked {
            n: 9,
            coin: "copper farthings".into()
        }
    );
    assert_eq!(example("too_much"), GameEvent::CannotCarry);
}

#[test]
fn a_cast_that_does_damage_is_a_spell_cast_and_a_heal_is_not() {
    let got = line("You cast magic missile at the large giant rat for 7 damage!");
    assert_eq!(
        got[0],
        GameEvent::SpellCast {
            spell: "magic missile".into(),
            target: Some("the large giant rat".into()),
            damage: 7
        },
        "{got:?}"
    );
    let got = line("You cast minor healing, for 5");
    assert!(!matches!(got[0], GameEvent::SpellCast { .. }), "{got:?}");
}

#[test]
fn the_spell_list_and_the_casting_refusals() {
    let got = line(
        "Level Mana Short Spell Name\n  1   2    vine  vine strike\n  3  12    mmis  magic missile",
    );
    assert_eq!(got[0], GameEvent::SpellsHeader, "{got:?}");
    assert_eq!(
        got[1],
        GameEvent::SpellRow {
            level: 1,
            mana: 2,
            short: "vine".into(),
            name: "vine strike".into()
        }
    );
    assert!(
        matches!(
            got[2],
            GameEvent::SpellRow {
                level: 3,
                mana: 12,
                ..
            }
        ),
        "{got:?}"
    );
    assert_eq!(example("spells_none"), GameEvent::SpellsNone);
    assert_eq!(example("no_mana"), GameEvent::NoMana);
    assert_eq!(example("already_this_round"), GameEvent::AlreadyCast);
    assert_eq!(example("cast_fail"), GameEvent::CastFailed);
}

#[test]
fn buying_reading_and_the_bank() {
    assert_eq!(
        example("bought"),
        GameEvent::Bought {
            item: "torch".into(),
            copper: 25,
        }
    );
    assert_eq!(example("buy_poor"), GameEvent::BuyRefused { why: "poor" });
    assert_eq!(
        example("buy_not_here"),
        GameEvent::BuyRefused { why: "not_here" }
    );
    assert_eq!(
        example("learn_scroll"),
        GameEvent::Learned {
            spell: "vine strike".into(),
        }
    );
    assert_eq!(example("already_known"), GameEvent::AlreadyKnown);
    // Same line as an item the game does not know how to use.
    assert_eq!(
        line("You don't know what to do with this!")[0],
        GameEvent::LearnRefused
    );
    // The balance: the first line names the bank, the second is the amount.
    assert!(matches!(example("bank_balance"), GameEvent::Other(_)));
    assert_eq!(
        line("On deposit: 1 gold crowns, 5 copper farthings")[0],
        GameEvent::Deposit { copper: 105 }
    );
    assert_eq!(example("bank_withdraw"), GameEvent::Withdrew { copper: 5 });
}

#[test]
fn training_and_what_the_trainer_refuses() {
    assert_eq!(example("trained"), GameEvent::Trained { copper: 50 });
    assert_eq!(
        example("train_no_exp"),
        GameEvent::TrainRefused { why: "no_exp" }
    );
    assert_eq!(
        example("train_no_money"),
        GameEvent::TrainRefused { why: "no_money" }
    );
    assert_eq!(
        example("train_level_band"),
        GameEvent::TrainRefused { why: "band" }
    );
    assert_eq!(
        example("train_wrong_room"),
        GameEvent::TrainRefused { why: "wrong_room" }
    );
}

#[test]
fn the_keys_carried() {
    assert_eq!(
        example("inv_keys"),
        GameEvent::Keys {
            keys: vec!["brass key".into()],
        }
    );
    assert_eq!(
        line("You have the following keys:  large iron key, bone key.")[0],
        GameEvent::Keys {
            keys: vec!["large iron key".into(), "bone key".into()],
        }
    );
    assert_eq!(example("inv_no_keys"), GameEvent::Keys { keys: vec![] });
}

/// The typed party events of one session in the `party` fixture (silvermere, 2026-10-22).
fn party(who: &str) -> Vec<GameEvent> {
    events(&format!("party-{who}"))
        .iter()
        .map(GameEvent::from_event)
        .filter(|e| {
            matches!(
                e,
                GameEvent::PartyInvited { .. }
                    | GameEvent::PartyInviteSent { .. }
                    | GameEvent::PartyFollowing { .. }
                    | GameEvent::PartyFollowerJoined { .. }
                    | GameEvent::PartyLeft { .. }
                    | GameEvent::PartyFollowerRemoved { .. }
                    | GameEvent::PartyMustBeInvited
                    | GameEvent::PartyMoved { .. }
                    | GameEvent::ParFollowing { .. }
                    | GameEvent::ParHeader
                    | GameEvent::ParNone
                    | GameEvent::ParRow { .. }
            )
        })
        .collect()
}

fn row(
    name: &str,
    class: &str,
    mana: Option<i64>,
    hp: i64,
    resting: bool,
    rank: Rank,
) -> GameEvent {
    GameEvent::ParRow {
        name: name.into(),
        class: Some(class.into()),
        mana,
        hp: Some(hp),
        resting,
        rank: Some(rank),
        invited: false,
    }
}

#[test]
fn party_leader() {
    use GameEvent as G;
    let aldis = |hp, resting, rank| row("Aldis Vey", "Witchunter", None, hp, resting, rank);
    let isolde = row(
        "Isolde Harrower",
        "Cleric",
        Some(100),
        100,
        false,
        Rank::Mid,
    );
    assert_eq!(
        party("aldis"),
        [
            G::ParNone,
            aldis(73, true, Rank::Mid),
            G::PartyInviteSent {
                name: "Isolde".into()
            },
            G::PartyFollowerJoined {
                name: "Isolde".into()
            },
            G::ParHeader,
            isolde.clone(),
            aldis(80, true, Rank::Front),
            G::PartyInviteSent {
                name: "Wren".into()
            },
            G::PartyFollowerJoined {
                name: "Wren".into()
            },
            G::PartyInviteSent {
                name: "Corvin".into()
            },
            G::ParHeader,
            isolde,
            row("Wren Talbot", "Druid", Some(100), 81, false, Rank::Mid),
            G::ParRow {
                name: "Corvin Ashdown".into(),
                class: Some("Bard".into()),
                mana: None,
                hp: None,
                resting: false,
                rank: None,
                invited: true,
            },
            aldis(83, true, Rank::Front),
            G::PartyFollowerRemoved {
                name: "Wren".into()
            },
            G::PartyFollowerRemoved {
                name: "Corvin".into()
            },
            G::ParNone,
            aldis(83, false, Rank::Mid),
        ]
    );
}

#[test]
fn party_followers() {
    use GameEvent as G;
    let leader = || "Aldis".to_owned();
    assert_eq!(
        party("isolde"),
        [
            G::PartyInvited { leader: leader() },
            // No full stop on silvermere.
            G::PartyFollowing { leader: leader() },
            G::ParFollowing { leader: leader() },
            G::ParHeader,
            row("Aldis Vey", "Witchunter", None, 80, true, Rank::Front),
            row(
                "Isolde Harrower",
                "Cleric",
                Some(100),
                100,
                false,
                Rank::Mid
            ),
            G::PartyMoved {
                dir: "north".into()
            },
            G::PartyLeft { leader: leader() },
            G::ParNone,
            row(
                "Isolde Harrower",
                "Cleric",
                Some(100),
                100,
                false,
                Rank::Mid
            ),
        ]
    );
    assert_eq!(
        party("wren"),
        [
            G::PartyMustBeInvited,
            G::PartyInvited { leader: leader() },
            G::PartyFollowing { leader: leader() },
            G::PartyMoved {
                dir: "north".into()
            },
            G::PartyLeft { leader: leader() },
        ]
    );
    assert_eq!(party("corvin"), [G::PartyInvited { leader: leader() }]);
}

#[test]
fn par_row_at_minus_100() {
    // `par` prints `%3.3s` of the percentage: -100% and below are `-10` (`kb/grouping.md` §2a).
    let evs = line(
        "The following people are in your travel party:\n  Aldis Vey                      (Witchunter)          [H:-10%]   - Frontrank",
    );
    assert_eq!(
        evs[1],
        row("Aldis Vey", "Witchunter", None, -10, false, Rank::Front)
    );
}

#[test]
fn the_characters_own_cast_is_no_blow() {
    // Message 17 ("You cast %s on %s!") is a spellcasting monster's hit message, so its first
    // text is a generated `mon_hits_you`; it is the character's own cast.
    for l in ["You cast bless on Aldric!", "You cast bless on yourself!"] {
        let evs = line(l);
        assert!(
            !evs.iter()
                .any(|e| matches!(e, GameEvent::AttackedBy { .. })),
            "{l}: {evs:?}"
        );
    }
}

#[test]
fn sneak_lines() {
    assert_eq!(
        line("Attempting to sneak..."),
        [GameEvent::SneakAttempt { failed: false }]
    );
    assert_eq!(
        line("Attempting to sneak...You don't think you're sneaking."),
        [GameEvent::SneakAttempt { failed: true }]
    );
    for l in [
        "You don't think you're sneaking.",
        "You are no longer sneaking.",
        "You make a sound as you enter the room!",
    ] {
        assert_eq!(line(l), [GameEvent::NotSneaking], "{l}");
    }
    for l in [
        "You may not sneak right now!",
        "You must wait before you may do that!",
    ] {
        assert_eq!(line(l), [GameEvent::SneakRefused], "{l}");
    }
    assert_eq!(line("Sneaking..."), [GameEvent::StillSneaking]);
}
