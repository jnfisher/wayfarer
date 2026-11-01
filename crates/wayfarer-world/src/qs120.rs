//! Quicksilver 1.20's spells (C-066, `docs/kb/quicksilver-1.20.md` "Spells"): the Mage,
//! Priest and Druid player spells as 1.20.0 has them (Spell Mod v2.1, protective aura, dark
//! binding, caps at 65, mana flux at 35), the regen line (6000-6020) with its scrolls and shops,
//! and the monster copies (5000-5999) left out. The data is `data/quicksilver-1.20/` (see its
//! `PROVENANCE.md`), embedded at build time; a 1.20 view of the pack's spells is built from it.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::{Value, json};

use crate::Error;
use crate::pack::{Spell, WorldPack};

const SPELLS_CSV: &str = include_str!("../data/quicksilver-1.20/spells.csv");
const SONGS_CSV: &str = include_str!("../data/quicksilver-1.20/songs.csv");
const REGEN_CSV: &str = include_str!("../data/quicksilver-1.20/regen.csv");
const SHOPS_CSV: &str = include_str!("../data/quicksilver-1.20/shops.csv");
const TEXTS_CSV: &str = include_str!("../data/quicksilver-1.20/texts.csv");

/// The spell numbers of the monsters' own copies: never a player's.
pub const MONSTER_COPIES: std::ops::RangeInclusive<i64> = 5000..=5999;
/// Spells outside the table whose only change is the level cap of 65.
pub const CAPPED_ONLY: [i64; 3] = [825, 1379, 1380];
/// The level cap of every Mage, Priest and Druid player spell that grows with level.
pub const LEVEL_CAP: i64 = 65;
/// The stock song the regen songs were made from: their type, resist mode and element, which
/// their rows leave blank, are its.
pub const SONG_TEMPLATE: i64 = 803;
/// Item 121, the scroll that taught detect magic, renamed.
pub const PROTECTIVE_AURA_SCROLL: (i64, &str) = (121, "scroll of protective aura");

/// A CSV text's rows, header first: fields split on commas, a field in double quotes may hold
/// commas, and `""` inside one is a quote.
fn csv(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for line in text.lines().filter(|l| !l.is_empty()) {
        let mut fields = Vec::new();
        let mut field = String::new();
        let mut quoted = false;
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '"' if quoted && chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                '"' => quoted = !quoted,
                ',' if !quoted => fields.push(std::mem::take(&mut field)),
                c => field.push(c),
            }
        }
        fields.push(field);
        rows.push(fields);
    }
    rows
}

/// A CSV's data rows as maps from its header's names.
fn records(text: &str) -> Vec<HashMap<String, String>> {
    let mut rows = csv(text).into_iter();
    let Some(header) = rows.next() else {
        return Vec::new();
    };
    rows.map(|r| header.iter().cloned().zip(r).collect())
        .collect()
}

/// `a/b` as (a, b); `-` or nothing as (0, 0).
fn per_levels(s: &str) -> (i64, i64) {
    match s.split_once('/') {
        Some((a, b)) => (a.trim().parse().unwrap_or(0), b.trim().parse().unwrap_or(0)),
        None => (0, 0),
    }
}

/// A number, `-` or nothing as 0.
fn num(s: &str) -> i64 {
    s.trim().parse().unwrap_or(0)
}

/// A number, or `None` for nothing.
fn opt(s: &str) -> Option<i64> {
    s.trim().parse().ok()
}

/// A magery's number, as the game stores it (`kb/spells.md`): Mage 1, Priest 2, Druid 3,
/// Bard 4; anything else 0 (no magery: the monsters' and the items').
pub fn magery(name: &str) -> i64 {
    match name {
        "Mage" => 1,
        "Priest" => 2,
        "Druid" => 3,
        "Bard" => 4,
        _ => 0,
    }
}

/// One player spell as 1.20.0 has it (a row of `spells.csv` or `songs.csv`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpellRow {
    pub number: i64,
    pub name: String,
    pub short: String,
    pub magery: i64,
    pub magery_level: i64,
    pub level: i64,
    pub mana: i64,
    pub energy: i64,
    pub difficulty: i64,
    pub target: i64,
    /// `None` when the row leaves it blank (a regen song: [`SONG_TEMPLATE`]'s).
    pub spell_type: Option<i64>,
    /// `None` when the row leaves it blank (a regen song: [`SONG_TEMPLATE`]'s).
    pub resist_mode: Option<i64>,
    /// `None` when the row leaves it blank (a regen song: [`SONG_TEMPLATE`]'s).
    pub element: Option<i64>,
    /// (base, increase, every so many levels).
    pub min: (i64, i64, i64),
    pub max: (i64, i64, i64),
    /// 0 for none.
    pub level_cap: i64,
    pub duration: (i64, i64, i64),
    /// (ability, value) in record order.
    pub abilities: Vec<(i64, i64)>,
    /// The handoff's note: empty when as 1.11p, `changed: ...`, `new (regen line)`, ...
    pub change: String,
}

impl SpellRow {
    fn from(r: &HashMap<String, String>) -> SpellRow {
        let f = |k: &str| r.get(k).map_or("", String::as_str);
        let (mininc, minlvls) = per_levels(f("min+/lvls"));
        let (maxinc, maxlvls) = per_levels(f("max+/lvls"));
        let (durinc, durlvls) = per_levels(f("dur+/lvls"));
        SpellRow {
            number: num(f("num")),
            name: f("name").to_owned(),
            short: f("short").to_owned(),
            magery: magery(f("magery")),
            magery_level: num(f("mlvl")),
            level: num(f("level")),
            mana: num(f("mana")),
            energy: num(f("energy")),
            difficulty: num(f("diff")),
            target: num(f("target")),
            spell_type: opt(f("type")),
            resist_mode: opt(f("resmode")),
            element: opt(f("element")),
            min: (num(f("min")), mininc, minlvls),
            max: (num(f("max")), maxinc, maxlvls),
            level_cap: num(f("cap")),
            duration: (num(f("dur")), durinc, durlvls),
            abilities: f("abilities")
                .split_whitespace()
                .filter_map(|p| {
                    let (a, v) = p.split_once(':')?;
                    Some((a.parse().ok()?, v.parse().ok()?))
                })
                .collect(),
            change: f("change").to_owned(),
        }
    }

    /// The spell's record as the pack keeps it (its `raw` JSON), from the stock record when
    /// there is one: what the table gives replaces it, the rest (the per-level duration, the
    /// message number, the flags) is kept. A field the row leaves blank is the stock record's,
    /// else `template`'s (a regen song's are [`SONG_TEMPLATE`]'s).
    fn raw(&self, stock: Option<&Value>, template: Option<&Value>) -> Value {
        let mut v = stock.cloned().unwrap_or_else(|| json!({}));
        let from = |k: &str, own: Option<i64>| {
            own.or_else(|| stock.and_then(|s| s.get(k)).and_then(Value::as_i64))
                .or_else(|| template.and_then(|t| t.get(k)).and_then(Value::as_i64))
                .unwrap_or(0)
        };
        let (spell_type, resist_mode, element) = (
            from("type", self.spell_type),
            from("resmode", self.resist_mode),
            from("element", self.element),
        );
        let (abilities, abilvals): (Vec<i64>, Vec<i64>) = self.abilities.iter().copied().unzip();
        let set = [
            ("number", json!(self.number)),
            ("name", json!(self.name)),
            ("shortnm", json!(self.short)),
            ("magery", json!(self.magery)),
            ("magelvl", json!(self.magery_level)),
            ("level", json!(self.level)),
            ("mana", json!(self.mana)),
            ("energy", json!(self.energy)),
            ("diff", json!(self.difficulty)),
            ("target", json!(self.target)),
            ("type", json!(spell_type)),
            ("resmode", json!(resist_mode)),
            ("element", json!(element)),
            ("minval", json!(self.min.0)),
            ("mininc", json!(self.min.1)),
            ("minlvls", json!(self.min.2)),
            ("maxval", json!(self.max.0)),
            ("maxinc", json!(self.max.1)),
            ("maxlvls", json!(self.max.2)),
            ("levelcap", json!(self.level_cap)),
            ("duration", json!(self.duration.0)),
            ("durinc", json!(self.duration.1)),
            ("durlvls", json!(self.duration.2)),
            ("abilities", json!(abilities)),
            ("abilvals", json!(abilvals)),
        ];
        if let Value::Object(map) = &mut v {
            for (k, val) in set {
                map.insert(k.to_owned(), val);
            }
            map.entry("durlvl").or_insert(json!(0));
        }
        v
    }
}

/// Every player spell row: the 4.5 table, then the four regen songs.
pub fn spell_rows() -> &'static [SpellRow] {
    static ROWS: OnceLock<Vec<SpellRow>> = OnceLock::new();
    ROWS.get_or_init(|| {
        records(SPELLS_CSV)
            .iter()
            .chain(records(SONGS_CSV).iter())
            .map(SpellRow::from)
            .collect()
    })
}

/// A regen-line family: each caster family's line of mana-regeneration self-buffs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    Attunement,
    Devotion,
    Communion,
    Song,
}

impl Family {
    fn from(s: &str) -> Option<Family> {
        Some(match s {
            "attunement" => Family::Attunement,
            "devotion" => Family::Devotion,
            "communion" => Family::Communion,
            "song" => Family::Song,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Family::Attunement => "attunement",
            Family::Devotion => "devotion",
            Family::Communion => "communion",
            Family::Song => "song",
        }
    }
}

/// One spell of the regen line (`regen.csv`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegenSpell {
    pub number: i64,
    pub name: String,
    pub short: String,
    pub family: Family,
    /// `full` (the full casters), `half` (Gypsy, Warlock, Cleric, Missionary) or `song`.
    pub line: String,
    pub level: i64,
    /// What it adds to Alter Mana Regeneration, per cent.
    pub regen_pct: i64,
    /// The classes that can use it.
    pub casters: Vec<String>,
    /// The scroll that teaches it: (item number, name); `None` for serenity's stock scroll.
    pub scroll: Option<(i64, String)>,
    pub shop: i64,
    pub shop_name: String,
    /// The scroll's cost in `currency` (2 gold, 3 platinum), before the shop's markup.
    pub cost: i64,
    pub currency: i64,
}

/// The regen line, in `regen.csv`'s order.
pub fn regen() -> &'static [RegenSpell] {
    static ROWS: OnceLock<Vec<RegenSpell>> = OnceLock::new();
    ROWS.get_or_init(|| {
        records(REGEN_CSV)
            .iter()
            .filter_map(|r| {
                let f = |k: &str| r.get(k).map_or("", String::as_str);
                let item = num(f("scroll_item"));
                Some(RegenSpell {
                    number: num(f("num")),
                    name: f("name").to_owned(),
                    short: f("short").to_owned(),
                    family: Family::from(f("family"))?,
                    line: f("line").to_owned(),
                    level: num(f("level")),
                    regen_pct: num(f("regen_pct")),
                    casters: f("casters").split('/').map(str::to_owned).collect(),
                    scroll: (item > 0).then(|| (item, f("scroll").to_owned())),
                    shop: num(f("shop")),
                    shop_name: f("shop_name").to_owned(),
                    cost: num(f("cost")),
                    currency: num(f("currency")),
                })
            })
            .collect()
    })
}

/// The regen-line spell numbered `number`, if it is one.
pub fn regen_spell(number: i64) -> Option<&'static RegenSpell> {
    regen().iter().find(|r| r.number == number)
}

/// A regen shop (`shops.csv`): its markup and its stock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegenShop {
    pub shop: i64,
    pub name: String,
    pub markup_pct: i64,
    pub quantity: i64,
    pub restock: i64,
    pub chance_pct: i64,
}

pub fn regen_shops() -> &'static [RegenShop] {
    static ROWS: OnceLock<Vec<RegenShop>> = OnceLock::new();
    ROWS.get_or_init(|| {
        records(SHOPS_CSV)
            .iter()
            .map(|r| {
                let f = |k: &str| r.get(k).map_or("", String::as_str);
                RegenShop {
                    shop: num(f("shop")),
                    name: f("name").to_owned(),
                    markup_pct: num(f("markup_pct")),
                    quantity: num(f("quantity")),
                    restock: num(f("restock")),
                    chance_pct: num(f("chance_pct")),
                }
            })
            .collect()
    })
}

/// A regen family's lines (`texts.csv`): the cast line to the caster and to the room (`%s` the
/// caster), the line when it ends, and how it shows on the caster's status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegenTexts {
    pub family: Family,
    pub cast_you: String,
    pub cast_room: String,
    pub ends: String,
    pub status: String,
}

pub fn regen_texts() -> &'static [RegenTexts] {
    static ROWS: OnceLock<Vec<RegenTexts>> = OnceLock::new();
    ROWS.get_or_init(|| {
        records(TEXTS_CSV)
            .iter()
            .filter_map(|r| {
                let f = |k: &str| r.get(k).map_or("", String::as_str).to_owned();
                Some(RegenTexts {
                    family: Family::from(&f("family"))?,
                    cast_you: f("cast_you"),
                    cast_room: f("cast_room"),
                    ends: f("ends"),
                    status: f("status"),
                })
            })
            .collect()
    })
}

/// The texts of `family`.
pub fn texts_of(family: Family) -> Option<&'static RegenTexts> {
    regen_texts().iter().find(|t| t.family == family)
}

/// The pack's spells as a 1.20 server has them: each row of the table replaces the spell of
/// its number (or is added, for the regen line), the three spells whose only change is the cap
/// are capped at 65, and the monsters' copies are left out.
pub fn spells(pack: &WorldPack) -> Result<Vec<Spell>, Error> {
    let mut stock: Vec<Spell> = pack.spells()?;
    stock.retain(|s| !MONSTER_COPIES.contains(&s.number));
    let mut by_number: HashMap<i64, usize> = stock
        .iter()
        .enumerate()
        .map(|(i, s)| (s.number, i))
        .collect();
    for &n in &CAPPED_ONLY {
        if let Some(&i) = by_number.get(&n)
            && let Ok(mut v) = serde_json::from_str::<Value>(&stock[i].raw)
        {
            let cap = v.get("levelcap").and_then(Value::as_i64).unwrap_or(0);
            if cap == 0 || cap > LEVEL_CAP {
                v["levelcap"] = json!(LEVEL_CAP);
                stock[i].raw = v.to_string();
            }
        }
    }
    let template = by_number
        .get(&SONG_TEMPLATE)
        .and_then(|&i| serde_json::from_str::<Value>(&stock[i].raw).ok());
    for row in spell_rows() {
        let old = by_number
            .get(&row.number)
            .and_then(|&i| serde_json::from_str::<Value>(&stock[i].raw).ok());
        let raw = row.raw(old.as_ref(), template.as_ref());
        let spell = Spell {
            number: row.number,
            name: row.name.clone(),
            short_name: row.short.clone(),
            level: row.level,
            mana: row.mana,
            spell_type: raw.get("type").and_then(Value::as_i64).unwrap_or(0),
            target: row.target,
            raw: raw.to_string(),
        };
        match by_number.get(&row.number) {
            Some(&i) => stock[i] = spell,
            None => {
                by_number.insert(row.number, stock.len());
                stock.push(spell);
            }
        }
    }
    stock.sort_by_key(|s| s.number);
    Ok(stock)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_csv_splitter_takes_quotes() {
        assert_eq!(
            csv("a,\"b, c\",\"\",\"d\"\"e\"\nx"),
            [vec!["a", "b, c", "", "d\"e"], vec!["x"]]
        );
        assert_eq!(per_levels("3/1"), (3, 1));
        assert_eq!(per_levels("-"), (0, 0));
    }

    #[test]
    fn every_row_reads() {
        let rows = spell_rows();
        // 219 Mage, Priest and Druid spells and 17 regen spells in 4.5's table, 4 songs built.
        assert_eq!(rows.len(), 240);
        assert_eq!(
            rows.iter()
                .filter(|r| r.change.starts_with("changed"))
                .count(),
            132
        );
        assert!(rows.iter().all(|r| r.number > 0 && !r.name.is_empty()));
        let mm = rows.iter().find(|r| r.number == 1).unwrap();
        assert_eq!(
            (
                mm.name.as_str(),
                mm.short.as_str(),
                mm.magery,
                mm.magery_level
            ),
            ("magic missile", "mmis", 1, 1)
        );
        assert_eq!((mm.min, mm.max, mm.level_cap), ((6, 1, 1), (15, 1, 1), 9));
        assert_eq!(mm.abilities, [(17, 0)]);
        let pa = rows.iter().find(|r| r.number == 24).unwrap();
        assert_eq!(pa.name, "protective aura");
        let db = rows.iter().find(|r| r.number == 1056).unwrap();
        assert_eq!(db.name, "dark binding");
        let flux = rows.iter().find(|r| r.number == 406).unwrap();
        assert_eq!((flux.level, flux.mana, flux.duration.0), (35, 12, 180));
        assert!(rows.iter().all(|r| !MONSTER_COPIES.contains(&r.number)));
        let song = rows.iter().find(|r| r.number == 6020).unwrap();
        assert_eq!((song.magery, song.level, song.mana), (4, 50, 16));
        assert_eq!(song.spell_type, None, "blank: the template song's");
        assert!(song.abilities.contains(&(115, 5107)));
    }

    #[test]
    fn the_regen_line() {
        let r = regen();
        assert_eq!(r.len(), 25);
        let gatt = regen_spell(6002).unwrap();
        assert_eq!(
            (
                gatt.family,
                gatt.level,
                gatt.regen_pct,
                gatt.shop,
                gatt.cost
            ),
            (Family::Attunement, 50, 200, 111, 5000)
        );
        assert_eq!(
            gatt.scroll,
            Some((5102, "scroll of greater attunement".into()))
        );
        let comm = regen_spell(6007).unwrap();
        assert_eq!(comm.shop_name, "Jorah's Tower, Druid Spells");
        assert_eq!(
            regen_spell(1377).unwrap().scroll,
            None,
            "serenity's is stock"
        );
        let rapture = regen_spell(6018).unwrap();
        assert_eq!((rapture.cost, rapture.currency), (50, 3));
        assert_eq!(regen_spell(6009).unwrap().casters, ["Gypsy", "Warlock"]);
        // Every regen spell has its row in the spell table.
        for s in r {
            assert!(
                spell_rows().iter().any(|row| row.number == s.number),
                "{}",
                s.name
            );
        }
        // Every shop it names is a regen shop.
        for s in r {
            assert!(
                regen_shops().iter().any(|sh| sh.shop == s.shop),
                "{}",
                s.shop
            );
        }
        assert_eq!(regen_shops().len(), 8);
        let t = texts_of(Family::Song).unwrap();
        assert_eq!(t.cast_you, "You hum a slow, dreaming melody.");
        assert_eq!(t.ends, "The melody in your mind fades.");
        assert_eq!(regen_texts().len(), 4);
    }
}
