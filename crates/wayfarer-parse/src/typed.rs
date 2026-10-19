//! Typed events: the events the character model reads, with numbers as numbers and lists split
//! (docs/plan/03-engine.md section 2). A thin layer over [`Event`]: `Parser::feed` is unchanged,
//! and [`GameEvent::from_event`] never fails; anything it does not type is [`GameEvent::Other`]
//! with the event kept.
//!
//! The status-screen variants are named after their patterns (`st_name` is `StatusName`).
//! Several patterns capture some values in unnamed groups (`st_str`, `st_int`, `st_wil`,
//! `st_picklocks`, `st_traps`, and the skill at the end of `st_race`, `st_class`, `st_hits` and
//! `st_mana`), so `Event.fields` lacks them; those are read from `Event.line` here.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;

use crate::event::{Event, Value};
use crate::helpers::{Carried, Exit, parse_carrying, parse_exits};

/// Which prompt form a prompt was read in (`set statline`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptForm {
    /// `[HP=../MA=..]:` (statline ON or FULL).
    Std,
    /// The player's `set statline custom` template.
    Custom,
    /// Any bracketed `[...hp=...]:`.
    Generic,
    /// Statline OFF: `>`.
    Off,
}

/// The resting mark of a prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rest {
    Awake,
    Resting,
    Meditating,
}

/// Mana or Kai (a Mystic's or Ninja's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManaKind {
    Mana,
    Kai,
}

/// The events the character model needs, typed; `Other` for the rest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameEvent {
    /// Every value is optional: the standard prompt has no maxima (they come from `st` or a
    /// custom statline), statline OFF has nothing, and a custom template has what it names.
    /// `resting` is `None` when the form does not show it.
    Prompt {
        hp: Option<i64>,
        hp_max: Option<i64>,
        mana: Option<i64>,
        mana_max: Option<i64>,
        mana_kind: Option<ManaKind>,
        resting: Option<Rest>,
        form: PromptForm,
        /// Custom statlines (`%x`, `%c`) and generic `exp=`/`cash=` forms.
        exp: Option<i64>,
        wealth: Option<i64>,
    },

    /// `st_name`: `Name: Kasimir Drakemoor   Lives/CP:  9/12`.
    StatusName {
        name: String,
        lives: Option<i64>,
        cp: Option<i64>,
    },
    /// `st_race`: `Race: Kang   Exp: 1712   Perception: 33`.
    StatusRace {
        race: String,
        exp: Option<i64>,
        perception: Option<i64>,
    },
    /// `st_class`: `Class: Witchunter Level: 2   Stealth: 0`.
    StatusClass {
        class: String,
        level: Option<i64>,
        stealth: Option<i64>,
    },
    /// `st_hits`: `Hits: 48/56   Armour Class: 19/1   Thievery: 0`.
    StatusHits {
        hp: Option<i64>,
        hp_max: Option<i64>,
        ac: Option<i64>,
        dr: Option<i64>,
        thievery: Option<i64>,
    },
    /// `st_mana`: `Mana: * 9/9   Spellcasting: 40   Traps: 0`.
    StatusMana {
        kind: Option<ManaKind>,
        mana: Option<i64>,
        mana_max: Option<i64>,
        spellcasting: Option<i64>,
        traps: Option<i64>,
    },
    /// `st_str`, `st_int`, `st_wil`, `st_picklocks`, `st_traps`: the line's stats and skills as
    /// labelled (`Strength`, `Agility`, `Tracking`, `Martial Arts`, `MagicRes`, ...).
    StatusStat {
        stats: Vec<(String, i64)>,
    },
    /// `health`: `Health: 48/56 [85%]   Mana: 9/9 [100%]`.
    Health {
        hp: Option<i64>,
        hp_max: Option<i64>,
        mana: Option<i64>,
        mana_max: Option<i64>,
    },
    /// `exp`: `Exp: 346 Level: 1 Exp needed for next level: 1304 (1650) [21%]`.
    ExpScreen {
        exp: Option<i64>,
        level: Option<i64>,
        needed: Option<i64>,
        next_total: Option<i64>,
        percent: Option<i64>,
    },

    RoomTitle {
        title: String,
    },
    RoomExits {
        exits: Vec<Exit>,
    },
    /// `The room is very dark - you can't see anything` (or pitch black).
    RoomDark,

    CombatEngaged,
    CombatOff,
    /// `monster_death`: a monster's death text, with the monsters that text belongs to (one
    /// text can be several monsters'; the line does not say which).
    MonsterDeath {
        monsters: Vec<String>,
    },
    ExpGain {
        amount: i64,
    },
    /// `took_damage`: `You took 3 damage.` (blows from monsters are `mon_hits_you`, in `Other`).
    TookDamage {
        amount: i64,
    },
    /// `healed_paid`: a healer healed every wound.
    Healed,
    /// `rest_start`, `meditate_start`.
    Resting {
        meditating: bool,
    },
    /// `meditate_done`. The game prints nothing when plain resting ends; the prompt's resting
    /// mark going away is the sign.
    RestEnd,

    /// `drops_unconscious`: someone (this character or another) dropped to the ground.
    Unconscious {
        who: String,
    },
    /// `you_killed`: `You have been killed!`.
    YouKilled,
    /// `lives_left` (`You have 8 lives left.`) and `no_lives`.
    Dead {
        lives_left: Option<i64>,
    },
    /// `st_poisoned`: `You are Poisoned!` on the status screen.
    Poisoned,
    /// `cured_paid`: a healer cured the poisoning.
    Cured,
    /// `poison_tick`: `You feel ill.`
    PoisonTick,
    Blind,
    Stunned,

    /// `inv_carrying` (and `inv_nothing`, empty): coins by name and items.
    Inventory {
        coins: BTreeMap<String, u64>,
        items: Vec<Carried>,
    },
    /// `inv_wealth`: the total, in copper farthings.
    Wealth {
        coins: i64,
    },
    /// `inv_encumbrance`: `Encumbrance: 520/3792 - None [13%]`.
    Encumbrance {
        current: Option<i64>,
        max: Option<i64>,
        level: String,
        percent: Option<i64>,
    },

    /// A line nothing recognised (category `unclassified`).
    Unknown {
        line: String,
    },
    /// Every other event, as the parser gave it.
    Other(Event),
}

/// A number from a field: an integer, or text such as `"48"` or `"1,304"`.
fn int(ev: &Event, name: &str) -> Option<i64> {
    match ev.field(name)? {
        Value::Int(n) => Some(*n),
        Value::Text(s) => parse_int(s),
        _ => None,
    }
}

fn parse_int(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.contains(',') {
        s.replace(',', "").parse().ok()
    } else {
        s.parse().ok()
    }
}

fn text(ev: &Event, name: &str) -> String {
    ev.text(name).unwrap_or_default().to_owned()
}

/// `Label: 79`, `Label: * 9`, `Martial Arts:   0`: every labelled number on a status line.
fn labelled_rx() -> &'static Regex {
    static RX: OnceLock<Regex> = OnceLock::new();
    RX.get_or_init(|| {
        Regex::new(r"(?P<label>[A-Z][A-Za-z]*(?: [A-Z][A-Za-z]*)?):\s*\*?\s*(?P<n>-?\d[\d,]*)")
            .expect("compiles")
    })
}

fn labelled(line: &str) -> Vec<(String, i64)> {
    labelled_rx()
        .captures_iter(line)
        .filter_map(|c| Some((c["label"].to_owned(), parse_int(&c["n"])?)))
        .collect()
}

/// The number labelled `label` on the line.
fn skill(ev: &Event, label: &str) -> Option<i64> {
    labelled(&ev.line)
        .into_iter()
        .find_map(|(l, n)| (l == label).then_some(n))
}

fn mana_kind(s: Option<&str>) -> Option<ManaKind> {
    match s? {
        "MA" | "Mana" => Some(ManaKind::Mana),
        "KAI" | "Kai" => Some(ManaKind::Kai),
        _ => None,
    }
}

fn prompt(ev: &Event) -> GameEvent {
    let form = match ev.text("form") {
        Some("custom") => PromptForm::Custom,
        Some("generic") => PromptForm::Generic,
        Some("off") => PromptForm::Off,
        _ => PromptForm::Std,
    };
    let resting = match ev.field("state") {
        Some(Value::Text(s)) if s == "Meditating" => Some(Rest::Meditating),
        Some(Value::Text(s)) if s == "Resting" => Some(Rest::Resting),
        Some(_) => Some(Rest::Awake),
        None => None,
    };
    GameEvent::Prompt {
        hp: int(ev, "hp"),
        hp_max: int(ev, "maxhp"),
        mana: int(ev, "mana"),
        mana_max: int(ev, "maxmana"),
        mana_kind: mana_kind(ev.text("mtype")),
        resting,
        form,
        exp: int(ev, "exp"),
        wealth: int(ev, "wealth"),
    }
}

impl GameEvent {
    /// Type an event. Total: anything not typed here is `Other`, with the event.
    pub fn from_event(ev: &Event) -> GameEvent {
        use GameEvent as G;
        if ev.cat == "unclassified" {
            return G::Unknown {
                line: ev.line.clone(),
            };
        }
        match ev.name.as_str() {
            "prompt" => prompt(ev),
            "st_name" => G::StatusName {
                name: text(ev, "name"),
                lives: int(ev, "lives"),
                cp: int(ev, "cp"),
            },
            "st_race" => G::StatusRace {
                race: text(ev, "race"),
                exp: int(ev, "exp"),
                perception: skill(ev, "Perception"),
            },
            "st_class" => G::StatusClass {
                class: text(ev, "cls"),
                level: int(ev, "level"),
                stealth: skill(ev, "Stealth"),
            },
            "st_hits" => G::StatusHits {
                hp: int(ev, "hp"),
                hp_max: int(ev, "maxhp"),
                ac: int(ev, "ac"),
                dr: int(ev, "dr"),
                thievery: skill(ev, "Thievery"),
            },
            "st_mana" => G::StatusMana {
                kind: mana_kind(ev.text("mtype")),
                mana: int(ev, "mana"),
                mana_max: int(ev, "maxmana"),
                spellcasting: skill(ev, "Spellcasting"),
                traps: skill(ev, "Traps"),
            },
            "st_str" | "st_int" | "st_wil" | "st_picklocks" | "st_traps" => G::StatusStat {
                stats: labelled(&ev.line),
            },
            "health" => G::Health {
                hp: int(ev, "hp"),
                hp_max: int(ev, "maxhp"),
                mana: int(ev, "mana"),
                mana_max: int(ev, "maxmana"),
            },
            "exp" => G::ExpScreen {
                exp: int(ev, "exp"),
                level: int(ev, "level"),
                needed: int(ev, "left"),
                next_total: int(ev, "total"),
                percent: int(ev, "pct"),
            },
            "room_title" => G::RoomTitle {
                title: text(ev, "name"),
            },
            "room_exits" => G::RoomExits {
                exits: parse_exits(ev.text("exits").unwrap_or_default()),
            },
            "room_dark" => G::RoomDark,
            "combat_engaged" => G::CombatEngaged,
            "combat_off" => G::CombatOff,
            "monster_death" => G::MonsterDeath {
                monsters: match ev.field("monsters") {
                    Some(Value::List(v)) => v.clone(),
                    _ => Vec::new(),
                },
            },
            "exp_gain" => match int(ev, "exp") {
                Some(amount) => G::ExpGain { amount },
                None => G::Other(ev.clone()),
            },
            "took_damage" => match int(ev, "dmg") {
                Some(amount) => G::TookDamage { amount },
                None => G::Other(ev.clone()),
            },
            "healed_paid" => G::Healed,
            "rest_start" => G::Resting { meditating: false },
            "meditate_start" => G::Resting { meditating: true },
            "meditate_done" => G::RestEnd,
            "drops_unconscious" => G::Unconscious {
                who: text(ev, "who"),
            },
            "you_killed" => G::YouKilled,
            "lives_left" => G::Dead {
                lives_left: int(ev, "lives"),
            },
            "no_lives" => G::Dead {
                lives_left: Some(0),
            },
            "st_poisoned" => G::Poisoned,
            "cured_paid" => G::Cured,
            "poison_tick" => G::PoisonTick,
            "blind" => G::Blind,
            "stunned" => G::Stunned,
            "inv_carrying" => {
                let (coins, items) = parse_carrying(ev.text("items").unwrap_or_default());
                G::Inventory { coins, items }
            }
            "inv_nothing" => G::Inventory {
                coins: BTreeMap::new(),
                items: Vec::new(),
            },
            "inv_wealth" => match int(ev, "copper") {
                Some(coins) => G::Wealth { coins },
                None => G::Other(ev.clone()),
            },
            "inv_encumbrance" => G::Encumbrance {
                current: int(ev, "cur"),
                max: int(ev, "max"),
                level: text(ev, "level"),
                percent: int(ev, "pct"),
            },
            _ => G::Other(ev.clone()),
        }
    }
}

impl From<&Event> for GameEvent {
    fn from(ev: &Event) -> Self {
        GameEvent::from_event(ev)
    }
}
