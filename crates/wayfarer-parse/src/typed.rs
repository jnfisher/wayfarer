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
use crate::helpers::{Carried, Exit, parse_also_here, parse_carrying, parse_exits};

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

/// Why the game refused a move (`kb/screen-patterns.md` "Movement results").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// `There is no exit in that direction!`: for an exit the data has, the believed position
    /// is wrong (`kb/pitfalls.md` 34).
    NoExit,
    /// `You have progressed too far ...`.
    LevelHigh,
    /// `You have not progressed far enough ...`.
    LevelLow,
    /// `You are not permitted in that room!`.
    NotPermitted,
    /// `You may not go through this exit!` (class and race exits among others).
    Forbidden,
    /// `You are too good/evil to go through this exit!`.
    Alignment,
    NeedSpell,
    NeedItem,
    /// `You do not have enough to cover the toll ...`.
    TollShort,
    /// `You may not pass through that exit at this point in time.`
    Timed,
    /// `You may not enter that room while in combat.` (or during a retaliation period).
    Combat,
    /// `You are not healthy enough to enter that room.`
    Unhealthy,
    /// `... must purchase the ... before you may move through this exit.`
    Purchase,
    /// `There is a closed door in that direction!` (a shut Key exit, or while dragging).
    ClosedDoor,
}

impl Refusal {
    /// The refusal a movement event names, if it is one.
    pub fn of(name: &str) -> Option<Refusal> {
        Some(match name {
            "no_exit" => Refusal::NoExit,
            "exit_level_high" => Refusal::LevelHigh,
            "exit_level_low" => Refusal::LevelLow,
            "not_permitted" => Refusal::NotPermitted,
            "exit_forbidden" => Refusal::Forbidden,
            "exit_alignment" => Refusal::Alignment,
            "need_spell" => Refusal::NeedSpell,
            "need_item" => Refusal::NeedItem,
            "toll_short" => Refusal::TollShort,
            "exit_timed" => Refusal::Timed,
            "exit_combat" => Refusal::Combat,
            "exit_unhealthy" => Refusal::Unhealthy,
            "exit_purchase" => Refusal::Purchase,
            "door_closed_drag" => Refusal::ClosedDoor,
            _ => return None,
        })
    }

    /// A few words for a narration: "no exit", "level too high", ...
    pub fn describe(self) -> &'static str {
        match self {
            Refusal::NoExit => "no exit",
            Refusal::LevelHigh => "level too high",
            Refusal::LevelLow => "level too low",
            Refusal::NotPermitted => "not permitted",
            Refusal::Forbidden => "forbidden",
            Refusal::Alignment => "wrong alignment",
            Refusal::NeedSpell => "needs a spell",
            Refusal::NeedItem => "needs an item",
            Refusal::TollShort => "toll not covered",
            Refusal::Timed => "closed at this time",
            Refusal::Combat => "not while in combat",
            Refusal::Unhealthy => "not healthy enough",
            Refusal::Purchase => "not purchased",
            Refusal::ClosedDoor => "a closed door",
        }
    }
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

    /// A move the game did not take (the refusals of `kb/screen-patterns.md` "Movement
    /// results").
    MoveRefused {
        reason: Refusal,
    },
    /// `door_closed`: a move into a shut door or gate. `The door is Closed!` (capital C) is
    /// shut and unlocked, `The door is closed!` locked (`kb/navigation.md`).
    DoorClosed {
        locked: bool,
    },
    /// `door_opened` (`The door is now open.`) and `door_already_open`.
    DoorOpened,
    /// `door_locked`: `open` on a locked door (`The door is locked.`).
    OpenRefused,
    /// `bash_ok` (`You bashed the door open.`) or `bash_fail`.
    BashResult {
        ok: bool,
    },
    /// The answer to `search <dir>`: `search_found_exit` (`You found an exit to the north!`)
    /// or `search_nothing_dir` (`You notice nothing different to the north.`). `dir` as the
    /// game names it: `north`, ..., `up` (`upwards`, `above you`), `down`.
    SearchResult {
        found: bool,
        dir: String,
    },

    /// `room_also_here`: who else is in the room, as listed. `players` are the capitalised
    /// single words (a named monster looks the same: check the monster names); `monsters` the
    /// rest, with their adjectives (`large giant rat`).
    AlsoHere {
        players: Vec<String>,
        monsters: Vec<String>,
    },
    /// `monster_arrives` (the monster's own text) or `monster_arrives_generic`: a monster came
    /// in. `name` as shown, without its article (`thin giant rat`).
    MonsterArrives {
        name: String,
    },
    /// `monster_leaves` or `monster_leaves_generic`.
    MonsterLeaves {
        name: String,
    },
    /// Someone attacked this character: `attacks_you` (`moves to attack you!`), or a monster's
    /// blow, miss, dodge or deflect at it (`mon_hits_you`, `mon_miss_you`, ... and the built-in
    /// forms). `name` without its article.
    AttackedBy {
        name: String,
    },
    /// `target_missing`: `You do not see rat here!` (`target` empty for `You don't see your
    /// target here!`).
    YouDoNotSee {
        target: String,
    },
    /// `no_effect_weapon`: `Your weapon has no effect against this monster!`.
    NoEffect,
    /// `rest_sick`: `You are too sick to rest!` (poisoned; or to meditate).
    TooSick,

    CombatEngaged,
    CombatOff,
    /// `you_hit`: the character's own blow did damage (`You smack giant rat for 4 damage!`).
    /// `target` as shown, without an article.
    YouHit {
        target: String,
        damage: i64,
    },
    /// `you_miss` (`You swipe at thin carrion beast!`) or `you_dodged` (`You swing at orc who
    /// dodges your attack!`): the character's own swing did nothing.
    YouMiss {
        target: String,
    },
    /// `you_glance`: the character's own swing hit, but the target's armour took all the
    /// damage (`Your swing at gigantic black ooze hits, but glances off its armour.`).
    SwingGlanced {
        target: String,
    },
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

    /// `room_items`: `You notice 9 copper farthings, torch here.`: the coins on the floor, by
    /// the name the game gives them (`gold crowns`).
    RoomItems {
        coins: BTreeMap<String, u64>,
        /// The other things lying there, counted.
        items: Vec<Carried>,
    },
    /// `took_item`: `You took torch.`
    ItemTaken {
        item: String,
    },
    /// `wear_ok`: `You are now wearing padded vest.`
    Worn {
        item: String,
    },
    /// `wield_ok`: `You are now holding quarterstaff.`
    Wielded {
        item: String,
    },
    /// `wear_fail`: `You may not wear that item!`, `You have no more room to wear that item!`
    /// or `<item> may not be worn!`.
    WearFailed,
    /// `weapon_forbidden`: `You may not use that weapon.` or `<item> is not a weapon!`.
    NotWielded,
    /// `picked_up_coins`: `You picked up 9 copper farthings`.
    CoinsPicked {
        n: u64,
        coin: String,
    },
    /// `too_much`: `You cannot carry that much!`
    CannotCarry,
    /// `spells_columns`: the header of the `spells` list, whose rows follow.
    SpellsHeader,
    /// `spell_row`: `  1   2    vine  vine strike`.
    SpellRow {
        level: i64,
        mana: i64,
        short: String,
        name: String,
    },
    /// `spells_none`: `You have no spells.`
    SpellsNone,
    /// `spell_cast_you` or `you_cast_generic` with damage: `You cast vine strike at acid slime for 7 damage!`
    SpellCast {
        spell: String,
        target: Option<String>,
        damage: i64,
    },
    /// `no_mana`: `You do not have enough mana to cast that spell.`
    NoMana,
    /// `already_this_round`: one general spell per round.
    AlreadyCast,
    /// `cast_fail`: `You attempt to cast vine strike, but fail.`
    CastFailed,

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

/// A monster's name as a line shows it, without the article a sentence starts with (`The
/// large giant rat`, `A thin giant rat`).
fn monster_name(ev: &Event, field: &str) -> String {
    let name = ev.text(field).unwrap_or_default().trim();
    ["The ", "the ", "An ", "an ", "A ", "a "]
        .iter()
        .find_map(|a| name.strip_prefix(a))
        .unwrap_or(name)
        .to_owned()
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
            "door_closed" => G::DoorClosed {
                locked: !ev.line.contains("is Closed!"),
            },
            "door_opened" | "door_already_open" => G::DoorOpened,
            "door_locked" => G::OpenRefused,
            "bash_ok" => G::BashResult { ok: true },
            "bash_fail" => G::BashResult { ok: false },
            "search_found_exit" | "search_nothing_dir" => G::SearchResult {
                found: ev.name == "search_found_exit",
                dir: match ev.text("dir") {
                    Some(d) if !d.is_empty() => d.to_ascii_lowercase(),
                    _ if ev.line.contains("upwards") || ev.line.contains("above you") => {
                        "up".to_owned()
                    }
                    _ => "down".to_owned(),
                },
            },
            name if Refusal::of(name).is_some() => G::MoveRefused {
                reason: Refusal::of(name).expect("a refusal"),
            },
            "room_also_here" => {
                let (players, monsters) = parse_also_here(ev.text("who").unwrap_or_default())
                    .into_iter()
                    .partition::<Vec<_>, _>(|p| p.player);
                G::AlsoHere {
                    players: players.into_iter().map(|p| p.name).collect(),
                    monsters: monsters.into_iter().map(|p| p.name).collect(),
                }
            }
            "monster_arrives" | "monster_arrives_generic" => G::MonsterArrives {
                name: monster_name(ev, "mon"),
            },
            "monster_leaves" | "monster_leaves_generic" => G::MonsterLeaves {
                name: monster_name(ev, "mon"),
            },
            "attacks_you" => G::AttackedBy {
                name: monster_name(ev, "who"),
            },
            "mon_hits_you"
            | "mon_hits_you_generic"
            | "mon_miss_you"
            | "mon_dodge_you"
            | "mon_deflect_you"
            | "mon_miss_builtin"
            | "mon_dodge_builtin"
            | "mon_deflect_builtin" => G::AttackedBy {
                name: monster_name(ev, "mon"),
            },
            "target_missing" => G::YouDoNotSee {
                target: text(ev, "target"),
            },
            "no_effect_weapon" => G::NoEffect,
            "rest_sick" => G::TooSick,
            "combat_engaged" => G::CombatEngaged,
            "combat_off" => G::CombatOff,
            "you_hit" => match int(ev, "dmg") {
                Some(damage) => G::YouHit {
                    target: monster_name(ev, "target"),
                    damage,
                },
                None => G::Other(ev.clone()),
            },
            "you_miss" | "you_dodged" => G::YouMiss {
                target: monster_name(ev, "target"),
            },
            "you_glance" => G::SwingGlanced {
                target: monster_name(ev, "target"),
            },
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
            "room_items" => {
                let (coins, items) = parse_carrying(ev.text("items").unwrap_or_default());
                G::RoomItems { coins, items }
            }
            "took_item" => G::ItemTaken {
                item: text(ev, "item"),
            },
            "wear_ok" => G::Worn {
                item: text(ev, "item"),
            },
            "wield_ok" => G::Wielded {
                item: text(ev, "item"),
            },
            "wear_fail" => G::WearFailed,
            "weapon_forbidden" => G::NotWielded,
            "picked_up_coins" => match int(ev, "n") {
                Some(n) => G::CoinsPicked {
                    n: u64::try_from(n).unwrap_or(0),
                    coin: text(ev, "coin"),
                },
                None => G::Other(ev.clone()),
            },
            "too_much" => G::CannotCarry,
            "spells_columns" => G::SpellsHeader,
            "spell_row" => match (int(ev, "level"), int(ev, "mana")) {
                (Some(level), Some(mana)) => G::SpellRow {
                    level,
                    mana,
                    short: text(ev, "short"),
                    name: text(ev, "name"),
                },
                _ => G::Other(ev.clone()),
            },
            "spells_none" => G::SpellsNone,
            // The spell's own message (`spell_cast_you`, from the spell records' texts) or the
            // generic one.
            "you_cast_generic" | "spell_cast_you" => match int(ev, "dmg") {
                Some(damage) => G::SpellCast {
                    spell: text(ev, "spell"),
                    target: ev
                        .text("target")
                        .map(str::to_owned)
                        .filter(|t| !t.is_empty()),
                    damage,
                },
                None => G::Other(ev.clone()),
            },
            "no_mana" => G::NoMana,
            "already_this_round" => G::AlreadyCast,
            "cast_fail" => G::CastFailed,
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
