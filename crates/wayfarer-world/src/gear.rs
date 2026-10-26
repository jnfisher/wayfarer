//! The gear catalogue: every weapon and piece of armour, whether a class and race may use it
//! (now, later or never, and why), and where it comes from: shops and their price, monsters
//! that carry it, quests that give it, rooms it is placed in, and the shops that buy it
//! (`kb/economy.md` "Carrying and wearing items", "Prices", "Shops"). Plus the monster sets a
//! gear plan measures weapons against: a loop's, an area's, or a list of names.
//!
//! [`Catalogue::build`] reads the pack once per class and race; [`Catalogue::for_level`] fills
//! in what depends on the character's level, Charm, Strength and position. [`spellshop`] does
//! the same job for spells.
//!
//! [`spellshop`]: crate::spellshop

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use rusqlite::{Connection, OpenFlags};

use crate::Error;
use crate::monsters::Monsters;
use crate::nav::Key;
use crate::pack::{Room, WorldPack};
use crate::spellshop::unit;

/// Item types: armour and weapons. Type 11 is only for some classes (`user_can_use`).
const ITEM_ARMOUR: i64 = 0;
const ITEM_WEAPON: i64 = 1;
const ITEM_TYPE_11: i64 = 11;
const TYPE_11_CLASSES: [i64; 5] = [1, 2, 3, 4, 9];

/// Item abilities read here (`kb/game-data.md`).
const ABILITY_MAGICAL: i64 = 28;
const ABILITY_ANTI_MAGIC: i64 = 51;
const ABILITY_GOOD: i64 = 97;
const ABILITY_EVIL: i64 = 98;
const ABILITY_NOT_GOOD: i64 = 110;
const ABILITY_NOT_EVIL: i64 = 111;
const ABILITY_NEUTRAL: i64 = 112;
const ABILITY_DELETED_AT_CLEANUP: i64 = 119;
const ABILITY_MIN_LEVEL: i64 = 135;
const ABILITY_MAX_LEVEL: i64 = 136;

/// `worn` of an off-hand item (a shield, a main-gauche).
const WORN_OFF_HAND: i64 = 12;
/// The class weapon kinds of a one-handed fighter: no off-hand items.
const ONE_HANDED_KINDS: [i64; 3] = [0, 2, 4];
/// The weapon kind "Limited" and the two items it may use (`LJNGAME.C:1030-1032`).
const KIND_LIMITED: i64 = 9;
const LIMITED_ITEMS: [i64; 2] = [100, 68];
/// A monster's normal attack (`atktypes`); 2 is a spell, whose `atkmaxs` is a cast level.
const ATTACK_NORMAL: i64 = 1;
/// Shop types: a gang shop buys nothing.
const SHOP_GANG: i64 = 11;
/// Room type of a shop.
const ROOM_SHOP: i64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Weapon,
    Armour,
}

/// Whether a character may use an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Status {
    /// Usable at the character's level.
    Now,
    /// Only the level or the alignment stands in the way.
    Later,
    /// The class, the race, the item's type, Magical against Anti-Magic, or a level passed.
    Never,
}

/// An alignment ability an item asks for. It depends on the character's legal level, which
/// wayfarer does not track, so it never makes an item *never*.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Alignment {
    Good,
    Evil,
    NotGood,
    NotEvil,
    Neutral,
}

impl Alignment {
    fn of_ability(ability: i64) -> Option<Alignment> {
        Some(match ability {
            ABILITY_GOOD => Alignment::Good,
            ABILITY_EVIL => Alignment::Evil,
            ABILITY_NOT_GOOD => Alignment::NotGood,
            ABILITY_NOT_EVIL => Alignment::NotEvil,
            ABILITY_NEUTRAL => Alignment::Neutral,
            _ => return None,
        })
    }
}

/// Why an item is not usable now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Reason {
    /// The item names classes, not this one.
    Class,
    /// The item names races, not this one.
    Race,
    /// The class's weapon kind does not allow the item's weapon type.
    WeaponType,
    /// The armour is heavier than the class may wear.
    ArmourType,
    /// Magical, and the class or race is Anti-Magic.
    Magical,
    /// An off-hand item, and the class fights one-handed.
    OffHand,
    /// Item type 11, not for this class.
    Type11,
    /// Usable from this level (*later*).
    Level(i64),
    /// Usable only up to this level, already passed (*never*).
    MaxLevel(i64),
    /// Needs this alignment (*later*).
    Alignment(Alignment),
}

impl Reason {
    /// Whether the reason only holds the item back for now.
    pub fn is_later(self) -> bool {
        matches!(self, Reason::Level(_) | Reason::Alignment(_))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Usability {
    pub status: Status,
    /// Every reason, *never* ones first; empty when usable now.
    pub reasons: Vec<Reason>,
}

impl Usability {
    fn of(mut reasons: Vec<Reason>) -> Usability {
        reasons.sort_by_key(|r| (r.is_later(), *r));
        let status = if reasons.iter().any(|r| !r.is_later()) {
            Status::Never
        } else if reasons.is_empty() {
            Status::Now
        } else {
            Status::Later
        };
        Usability { status, reasons }
    }
}

/// Where a quest's text block hangs from.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum QuestAt {
    /// A room's command text.
    Room(Key),
    /// A monster's talk text (`ask` or `greet`).
    Monster { number: i64, name: String },
}

/// One way to come by an item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// A shop that stocks it (`maxqty` above 0).
    Shop {
        shop: i64,
        shop_name: String,
        /// The shop's markup, per cent.
        markup: i64,
        /// The rooms that are the shop.
        rooms: Vec<Key>,
        /// In copper, for the character's Charm ([`buy_price`]); 0 until [`Catalogue::for_level`].
        price: u64,
        /// The shop's room nearest the character, and the steps to it; `None` when there is
        /// no way (or before [`Catalogue::for_level`]).
        room: Option<Key>,
        steps: Option<u32>,
    },
    /// A monster that carries it.
    Drop {
        monster: i64,
        name: String,
        /// The chance it drops, per cent (`itempcts`).
        percent: i64,
        max_hp: i64,
        /// Its largest normal hit (`atkmaxs` of its normal attacks).
        max_hit: i64,
        boss: bool,
        /// Hours a boss takes to come back.
        regen_hours: Option<i64>,
        /// A limited monster (`maxalive` above 0) that has never died carries every item on
        /// its list: on a server where it has never been killed, the first kill drops it.
        first_kill: bool,
        /// Where it is: its `genmonster` or `specmon` rooms, else the rooms whose spawn pool
        /// holds it.
        rooms: Vec<Key>,
    },
    /// A text block with `giveitem` (not a `random` table's line).
    Quest {
        block: i64,
        /// The rooms and monsters the block hangs from, when they can be found.
        at: Vec<QuestAt>,
    },
    /// Placed in a room, back on a cold start.
    Placed { room: Key },
}

/// A shop that buys an item, and what it pays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sale {
    pub shop: i64,
    pub shop_name: String,
    pub rooms: Vec<Key>,
    /// In copper, for the character's Charm ([`sell_price`]); 0 until [`Catalogue::for_level`].
    pub price: u64,
}

/// One weapon or piece of armour, for one class and race.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GearItem {
    pub number: i64,
    pub name: String,
    pub kind: Kind,
    /// Where it is worn (`worn`; 0 for a weapon).
    pub slot: i64,
    /// `wpntype` (0 one-handed blunt, 1 two-handed blunt, 2 one-handed sharp, 3 two-handed
    /// sharp) and `armtype`.
    pub weapon_type: i64,
    pub armour_type: i64,
    pub str_req: i64,
    pub cost: i64,
    pub currency: i64,
    /// Before [`Catalogue::for_level`], the class and race checks alone.
    pub usability: Usability,
    pub sources: Vec<Source>,
    pub sales: Vec<Sale>,
    /// At most this many copies in play (`limit`).
    pub limited: Option<i64>,
    pub deleted_at_cleanup: bool,
    pub magical: bool,
    /// A weapon above the character's Strength: by how much. It is usable, each blow costs
    /// more.
    pub str_short: Option<i64>,
}

/// The character the catalogue is read for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wearer {
    pub level: i64,
    pub charm: Option<i64>,
    pub strength: Option<i64>,
}

/// What buying costs at `charm`, in copper: the cost in copper plus the markup, then the Charm
/// discount, each step truncating (`kb/economy.md` "Prices", as [`crate::spellshop`]). With the
/// Charm not known, 50 (neither discount nor surcharge).
pub fn buy_price(cost: i64, currency: i64, markup: i64, charm: Option<i64>) -> u64 {
    let copper = u64::try_from(cost.max(0)).unwrap_or(0) * unit(currency);
    let marked = copper * u64::try_from(100 + markup.max(0)).unwrap_or(100) / 100;
    let factor = u64::try_from(110 - charm.unwrap_or(50) / 5).unwrap_or(100);
    marked * factor / 100
}

/// What a shop pays at `charm`, in copper: the cost in copper × (Cha/2 + 25)%, without the
/// markup (`kb/economy.md` "Prices").
pub fn sell_price(cost: i64, currency: i64, charm: Option<i64>) -> u64 {
    let copper = u64::try_from(cost.max(0)).unwrap_or(0) * unit(currency);
    copper * u64::try_from(charm.unwrap_or(50) / 2 + 25).unwrap_or(0) / 100
}

/// A class or race as `user_can_use` reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct User {
    pub class: i64,
    pub race: i64,
    /// The class's weapon kind: 0-3 one type, 4 one-handed, 5 two-handed, 6 sharp, 7 blunt,
    /// 8 all, 9 Limited.
    pub weapon_kind: i64,
    /// The heaviest `armtype` the class may wear.
    pub armour: i64,
    /// The class or the race has Anti-Magic.
    pub anti_magic: bool,
}

/// What `user_can_use` reads of an item.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Needs {
    pub number: i64,
    pub item_type: i64,
    /// The named classes and races (0s left out).
    pub classes: Vec<i64>,
    pub races: Vec<i64>,
    pub weapon_type: i64,
    pub armour_type: i64,
    pub worn: i64,
    pub magical: bool,
    pub min_level: Option<i64>,
    pub max_level: Option<i64>,
    pub alignments: Vec<Alignment>,
}

/// The weapon types a class weapon kind allows.
fn kind_allows(kind: i64, weapon_type: i64, item: i64) -> bool {
    match kind {
        0..=3 => weapon_type == kind,
        4 => matches!(weapon_type, 0 | 2),
        5 => matches!(weapon_type, 1 | 3),
        6 => matches!(weapon_type, 2 | 3),
        7 => matches!(weapon_type, 0 | 1),
        8 => true,
        KIND_LIMITED => LIMITED_ITEMS.contains(&item),
        _ => false,
    }
}

impl User {
    /// The reasons that do not depend on the level (`user_can_use`, `LJNUTIL.C:9418-9740`).
    pub fn never(&self, n: &Needs) -> Vec<Reason> {
        let mut out = Vec::new();
        if !n.classes.is_empty() && !n.classes.contains(&self.class) {
            out.push(Reason::Class);
        }
        if !n.races.is_empty() && !n.races.contains(&self.race) {
            out.push(Reason::Race);
        }
        if self.anti_magic && n.magical {
            out.push(Reason::Magical);
        }
        // Being named in either list overrides the weapon and armour types.
        let named = n.classes.contains(&self.class) || n.races.contains(&self.race);
        if !named {
            match n.item_type {
                ITEM_WEAPON if !kind_allows(self.weapon_kind, n.weapon_type, n.number) => {
                    out.push(Reason::WeaponType);
                }
                ITEM_ARMOUR => {
                    if n.armour_type > self.armour {
                        out.push(Reason::ArmourType);
                    }
                    if n.worn == WORN_OFF_HAND && ONE_HANDED_KINDS.contains(&self.weapon_kind) {
                        out.push(Reason::OffHand);
                    }
                }
                _ => {}
            }
        }
        if n.item_type == ITEM_TYPE_11 && !TYPE_11_CLASSES.contains(&self.class) {
            out.push(Reason::Type11);
        }
        out
    }

    /// Whether a character at `level` may use the item, and every reason not.
    pub fn usability(&self, n: &Needs, level: i64) -> Usability {
        let mut reasons = self.never(n);
        reasons.extend(level_reasons(n, level));
        Usability::of(reasons)
    }
}

fn level_reasons(n: &Needs, level: i64) -> Vec<Reason> {
    let mut out = Vec::new();
    if let Some(min) = n.min_level.filter(|&m| m > level) {
        out.push(Reason::Level(min));
    }
    if let Some(max) = n.max_level.filter(|&m| m < level) {
        out.push(Reason::MaxLevel(max));
    }
    out.extend(n.alignments.iter().map(|&a| Reason::Alignment(a)));
    out
}

// A build reads a few fields of thousands of records: parsing each whole record as JSON took
// most of its time, so the fields are found in the text, as the export writes them
// (`"key":value`, no spaces).

/// The text after `"key":` in a record, searched from the end (the fields read most, a
/// room's, are near it).
fn after<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let mut to = text.len();
    while let Some(at) = text[..to].rfind(key) {
        let end = at + key.len();
        if at > 0 && text.as_bytes()[at - 1] == b'"' && text[end..].starts_with("\":") {
            return Some(&text[end + 2..]);
        }
        to = at;
    }
    None
}

/// An integer field: `"cmdtext":12`.
fn field_int(text: &str, key: &str) -> Option<i64> {
    let rest = after(text, key)?;
    let end = rest
        .find(|c: char| c != '-' && !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// An integer list field: `"placed":[1,2,0]`.
fn field_ints(text: &str, key: &str) -> Option<Vec<i64>> {
    field_ints_of(after(text, key)?)
}

/// An integer list at the start of a text: `[1,2,0]`.
fn field_ints_of(text: &str) -> Option<Vec<i64>> {
    let rest = text.strip_prefix('[')?;
    let body = &rest[..rest.find(']')?];
    if body.trim().is_empty() {
        return Some(Vec::new());
    }
    body.split(',').map(|n| n.trim().parse().ok()).collect()
}

/// An integer field, 0 when missing.
fn int(text: &str, key: &str) -> i64 {
    field_int(text, key).unwrap_or(0)
}

/// An integer list field, empty when missing.
fn ints(text: &str, key: &str) -> Vec<i64> {
    field_ints(text, key).unwrap_or_default()
}

/// One item as the catalogue keeps it: the class and race checks done, the level ones to do.
#[derive(Clone, Debug)]
struct Entry {
    item: GearItem,
    needs: Needs,
}

/// Every weapon and piece of armour for one class and race, with its sources. Build it once
/// and keep it: the planner asks often.
#[derive(Clone, Debug)]
pub struct Catalogue {
    pub user: User,
    entries: Vec<Entry>,
}

impl Catalogue {
    /// Read every weapon and armour piece (about 900 of the 1,950 items) and where each comes
    /// from, for class `class` and race `race` by number.
    pub fn build(pack: &WorldPack, class: i64, race: i64) -> Result<Catalogue, Error> {
        let user = user(pack, class, race)?;
        let mut items = pack.items_of_type(ITEM_ARMOUR)?;
        items.extend(pack.items_of_type(ITEM_WEAPON)?);
        items.sort_by_key(|i| i.number);
        let wanted: HashSet<i64> = items.iter().map(|i| i.number).collect();

        let scan = Scan::read(pack)?;
        let mut sources: HashMap<i64, Vec<Source>> = HashMap::new();
        let mut sales: HashMap<i64, Vec<Sale>> = HashMap::new();
        shops(pack, &scan, &wanted, &mut sources, &mut sales)?;
        drops(&scan, &wanted, &mut sources);
        quests(&scan, &wanted, &mut sources);
        placed(&scan, &wanted, &mut sources);

        let mut entries = Vec::with_capacity(items.len());
        for item in items {
            let v = item.raw.as_str();
            let abilities = ints(v, "abilities");
            let values = ints(v, "abilvals");
            let ability = |a: i64| {
                abilities
                    .iter()
                    .position(|&x| x == a)
                    .map(|i| values.get(i).copied().unwrap_or(0))
            };
            let has = |a: i64| abilities.contains(&a);
            let named =
                |f: &str| -> Vec<i64> { ints(v, f).into_iter().filter(|&n| n != 0).collect() };
            let needs = Needs {
                number: item.number,
                item_type: item.item_type,
                classes: named("classes"),
                races: named("races"),
                weapon_type: int(v, "wpntype"),
                armour_type: int(v, "armtype"),
                worn: int(v, "worn"),
                magical: has(ABILITY_MAGICAL),
                min_level: ability(ABILITY_MIN_LEVEL),
                max_level: ability(ABILITY_MAX_LEVEL),
                alignments: abilities
                    .iter()
                    .filter_map(|&a| Alignment::of_ability(a))
                    .collect(),
            };
            let limit = int(v, "limit");
            let gear = GearItem {
                number: item.number,
                name: item.name.clone(),
                kind: if item.item_type == ITEM_WEAPON {
                    Kind::Weapon
                } else {
                    Kind::Armour
                },
                slot: needs.worn,
                weapon_type: needs.weapon_type,
                armour_type: needs.armour_type,
                str_req: int(v, "strreq"),
                cost: item.cost,
                currency: item.currency,
                usability: Usability::of(user.never(&needs)),
                sources: sources.remove(&item.number).unwrap_or_default(),
                sales: sales.remove(&item.number).unwrap_or_default(),
                limited: (limit > 0).then_some(limit),
                deleted_at_cleanup: has(ABILITY_DELETED_AT_CLEANUP),
                magical: needs.magical,
                str_short: None,
            };
            entries.push(Entry { item: gear, needs });
        }
        Ok(Catalogue { user, entries })
    }

    /// Every item, by number, before the level checks.
    pub fn items(&self) -> impl Iterator<Item = &GearItem> {
        self.entries.iter().map(|e| &e.item)
    }

    /// One item by name (any case), before the level checks.
    pub fn named(&self, name: &str) -> Option<&GearItem> {
        self.items().find(|i| i.name.eq_ignore_ascii_case(name))
    }

    /// Every item for a character: the level checks, the prices at its Charm, the Strength
    /// it lacks, and the nearest room of each shop. `reach` gives the steps to the nearest of
    /// some rooms and which it is, as in [`crate::spellshop::plan`].
    pub fn for_level(
        &self,
        who: Wearer,
        reach: impl Fn(&[Key]) -> Option<(u32, Key)>,
    ) -> Vec<GearItem> {
        self.entries
            .iter()
            .map(|e| {
                let mut item = e.item.clone();
                item.usability = self.user.usability(&e.needs, who.level);
                item.str_short = match (item.kind, who.strength) {
                    (Kind::Weapon, Some(s)) if item.str_req > s => Some(item.str_req - s),
                    _ => None,
                };
                for source in &mut item.sources {
                    if let Source::Shop {
                        markup,
                        rooms,
                        price,
                        room,
                        steps,
                        ..
                    } = source
                    {
                        *price = buy_price(item.cost, item.currency, *markup, who.charm);
                        (*steps, *room) = match reach(rooms) {
                            Some((n, k)) => (Some(n), Some(k)),
                            None => (None, rooms.first().copied()),
                        };
                    }
                }
                for sale in &mut item.sales {
                    sale.price = sell_price(item.cost, item.currency, who.charm);
                }
                item
            })
            .collect()
    }
}

/// The class's and race's fields `user_can_use` reads.
fn user(pack: &WorldPack, class: i64, race: i64) -> Result<User, Error> {
    let raw = |table: &str, number: i64| -> Result<String, Error> {
        let raw: Option<String> = pack
            .conn()
            .prepare_cached(&format!("SELECT raw FROM {table} WHERE number = ?1"))?
            .query_row([number], |r| r.get(0))
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                e => Err(e),
            })?;
        raw.ok_or_else(|| Error::Format(format!("{table}: no number {number}")))
    };
    let c = raw("classes", class)?;
    let r = raw("races", race)?;
    let anti_magic = ints(&c, "abilities")
        .into_iter()
        .chain(ints(&r, "abilities"))
        .any(|a| a == ABILITY_ANTI_MAGIC);
    Ok(User {
        class,
        race,
        weapon_kind: int(&c, "weapon"),
        armour: int(&c, "armour"),
        anti_magic,
    })
}

/// The bytes at the end of a room's record read for its command text and placed items: the
/// export writes `cmdtext` 226 bytes from the end at most, `placed` after it. A record without
/// them there is read whole.
const ROOM_TAIL: i64 = 320;

/// What a build reads of the world, in one pass over the rooms: reading the rooms is most of
/// a build's time (26,720 records of about 1.5 kB each), so only the end of each record is
/// looked at.
struct Scan {
    /// Every room, with the items placed in it and its command text (0 for none).
    rooms: Vec<(Room, Vec<i64>, i64)>,
    monsters: Monsters,
    /// The fields of each monster's record read here, by number.
    records: HashMap<i64, Record>,
    /// Every text block with its link ([`text_blocks`]).
    blocks: BTreeMap<i64, (i64, String)>,
}

/// A monster's fields the catalogue reads.
struct Record {
    items: Vec<i64>,
    item_pcts: Vec<i64>,
    attack_types: Vec<i64>,
    attack_maxes: Vec<i64>,
    max_alive: i64,
    kill_date: i64,
    talk_text: i64,
}

/// Every room, with its placed items and command text, in table order (by key, the index's
/// order, would read the table a row at a time). Only the fields the spawn pass and the shops
/// read are filled in: the name, light, `regendelay`, flags and MegaMud id are left empty.
fn read_rooms(conn: &Connection) -> Result<Vec<(Room, Vec<i64>, i64)>, Error> {
    let mut st = conn.prepare(
        "SELECT map, room, type, shop_num, min_level, max_level, gen_monster, max_monsters, \
         mon_group, spec_mon, CAST(substr(CAST(raw AS BLOB), ?1) AS TEXT) FROM rooms",
    )?;
    let rows = st.query_map([-ROOM_TAIL], |r| {
        let room = Room {
            map: r.get(0)?,
            room: r.get(1)?,
            name: String::new(),
            light: 0,
            room_type: r.get(2)?,
            shop_num: r.get(3)?,
            min_level: r.get(4)?,
            max_level: r.get(5)?,
            gen_monster: r.get(6)?,
            max_monsters: r.get(7)?,
            mon_group: r.get(8)?,
            regen_delay: 0,
            spec_mon: r.get(9)?,
            flags: 0,
            mm_id: 0,
        };
        Ok((room, tail_fields(r.get_ref(10)?.as_bytes()?)))
    })?;
    let mut whole = conn.prepare("SELECT raw FROM rooms WHERE map = ?1 AND room = ?2")?;
    let mut rooms = Vec::new();
    for row in rows {
        let (room, fields) = row?;
        let (placed, cmdtext) = match fields {
            Some(f) => f,
            // Not in the tail: the whole record.
            None => {
                let raw: String = whole.query_row([room.map, room.room], |r| r.get(0))?;
                (ints(&raw, "placed"), int(&raw, "cmdtext"))
            }
        };
        let placed = placed.into_iter().filter(|&i| i != 0).collect();
        rooms.push((room, placed, cmdtext));
    }
    Ok(rooms)
}

/// A room's placed items and command text from the end of its record, scanning back from
/// the end once for both keys (26,720 records: the general search took longer than the read).
fn tail_fields(tail: &[u8]) -> Option<(Vec<i64>, i64)> {
    const PLACED: &[u8] = b"\"placed\":";
    const CMDTEXT: &[u8] = b"\"cmdtext\":";
    let (mut placed, mut cmdtext) = (None, None);
    for at in (0..tail.len()).rev().filter(|&i| tail[i] == b'"') {
        let rest = &tail[at..];
        if placed.is_none() && rest.starts_with(PLACED) {
            placed = Some(at + PLACED.len());
        } else if cmdtext.is_none() && rest.starts_with(CMDTEXT) {
            cmdtext = Some(at + CMDTEXT.len());
        }
        if placed.is_some() && cmdtext.is_some() {
            break;
        }
    }
    // Both values are ASCII, whatever the tail began with.
    let text = |at: usize| {
        let list = tail.get(at) == Some(&b'[');
        let end = tail[at..]
            .iter()
            .position(|&b| {
                if list {
                    b == b']'
                } else {
                    b == b',' || b == b'}'
                }
            })
            .map_or(tail.len(), |n| at + n + usize::from(list));
        std::str::from_utf8(&tail[at..end]).ok()
    };
    let placed = text(placed?)?;
    let cmdtext = text(cmdtext?)?;
    Some((field_ints_of(placed)?, cmdtext.parse().ok()?))
}

impl Scan {
    fn read(pack: &WorldPack) -> Result<Scan, Error> {
        // The rooms on a connection of their own, beside the monsters and texts: the two halves
        // take about as long. A pack with no file (none yet) reads them here.
        let path = pack
            .conn()
            .path()
            .filter(|p| !p.is_empty())
            .map(ToOwned::to_owned);
        std::thread::scope(|s| {
            let rooms = path.map(|path| {
                s.spawn(move || {
                    let conn = Connection::open_with_flags(
                        path,
                        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
                    )?;
                    read_rooms(&conn)
                })
            });
            let monsters = Monsters::load(pack)?;
            let records = monsters
                .iter()
                .map(|m| {
                    let v = m.raw.as_str();
                    let record = Record {
                        items: ints(v, "items"),
                        item_pcts: ints(v, "itempcts"),
                        attack_types: ints(v, "atktypes"),
                        attack_maxes: ints(v, "atkmaxs"),
                        max_alive: int(v, "maxalive"),
                        kill_date: int(v, "killdate"),
                        talk_text: int(v, "talktext"),
                    };
                    (m.number, record)
                })
                .collect();
            let blocks = text_blocks(pack)?;
            let rooms = match rooms {
                Some(h) => h.join().unwrap_or_else(|p| std::panic::resume_unwind(p))?,
                None => read_rooms(pack.conn())?,
            };
            Ok(Scan {
                rooms,
                monsters,
                records,
                blocks,
            })
        })
    }

    /// Each shop's rooms, by shop number.
    fn shop_rooms(&self) -> BTreeMap<i64, Vec<Key>> {
        let mut out: BTreeMap<i64, Vec<Key>> = BTreeMap::new();
        for (r, _, _) in &self.rooms {
            if r.room_type == ROOM_SHOP && r.shop_num > 0 {
                out.entry(r.shop_num).or_default().push((r.map, r.room));
            }
        }
        out
    }
}

/// The shops that sell (`maxqty` above 0) and buy (any slot; not a gang shop) each item.
fn shops(
    pack: &WorldPack,
    scan: &Scan,
    wanted: &HashSet<i64>,
    sources: &mut HashMap<i64, Vec<Source>>,
    sales: &mut HashMap<i64, Vec<Sale>>,
) -> Result<(), Error> {
    let rooms = scan.shop_rooms();
    for (number, shop_type) in pack.shop_types()? {
        let Some(shop) = pack.shop(number)? else {
            continue;
        };
        let shop_rooms = rooms.get(&number).cloned().unwrap_or_default();
        let mut sold = HashSet::new();
        let mut bought = HashSet::new();
        for &(item, max_qty) in &shop.stock {
            if !wanted.contains(&item) {
                continue;
            }
            if max_qty > 0 && sold.insert(item) {
                sources.entry(item).or_default().push(Source::Shop {
                    shop: number,
                    shop_name: shop.name.clone(),
                    markup: shop.markup,
                    rooms: shop_rooms.clone(),
                    price: 0,
                    room: None,
                    steps: None,
                });
            }
            if shop_type != SHOP_GANG && bought.insert(item) {
                sales.entry(item).or_default().push(Sale {
                    shop: number,
                    shop_name: shop.name.clone(),
                    rooms: shop_rooms.clone(),
                    price: 0,
                });
            }
        }
    }
    Ok(())
}

/// The monsters that carry each item, with where they are.
fn drops(scan: &Scan, wanted: &HashSet<i64>, sources: &mut HashMap<i64, Vec<Source>>) {
    let monsters = &scan.monsters;
    // (monster, item, percent), and the carriers.
    let mut carried: Vec<(i64, i64, i64)> = Vec::new();
    let mut carriers = HashSet::new();
    for m in monsters.iter() {
        let Some(v) = scan.records.get(&m.number) else {
            continue;
        };
        for (i, &item) in v.items.iter().enumerate() {
            if item != 0 && wanted.contains(&item) {
                carried.push((m.number, item, v.item_pcts.get(i).copied().unwrap_or(0)));
                carriers.insert(m.number);
            }
        }
    }
    let rooms = carrier_rooms(scan, &carriers);
    for (number, item, percent) in carried {
        let (Some(m), Some(v)) = (monsters.get(number), scan.records.get(&number)) else {
            continue;
        };
        let max_hit = v
            .attack_maxes
            .iter()
            .zip(&v.attack_types)
            .filter(|&(_, &k)| k == ATTACK_NORMAL)
            .map(|(&hit, _)| hit)
            .max()
            .unwrap_or(0);
        let entry = Source::Drop {
            monster: number,
            name: m.name.clone(),
            percent,
            max_hp: m.max_hp,
            max_hit,
            boss: monsters.is_boss(number),
            regen_hours: monsters.regen_hours(number),
            first_kill: v.max_alive > 0 && v.kill_date == 0,
            rooms: rooms.get(&number).cloned().unwrap_or_default(),
        };
        let list = sources.entry(item).or_default();
        if !list.contains(&entry) {
            list.push(entry);
        }
    }
}

/// The fields of a room the spawn pass reads (`Monsters::generated`): `maxmonsters` above 0,
/// type, `genmonster`, `mongroup`, `minlevel`, `maxlevel`.
type SpawnKind = (bool, i64, i64, i64, i64, i64);

/// Where each monster of `of` is: the rooms that name it (`genmonster`, `specmon`), else the
/// rooms whose spawn pool holds it.
fn carrier_rooms(scan: &Scan, of: &HashSet<i64>) -> HashMap<i64, Vec<Key>> {
    let mut named: HashMap<i64, BTreeSet<Key>> = HashMap::new();
    let mut pooled: HashMap<i64, BTreeSet<Key>> = HashMap::new();
    // What the spawn pass makes depends on these fields alone, and most rooms share them with
    // others: each kind of room is worked out once.
    let mut made: HashMap<SpawnKind, Vec<i64>> = HashMap::new();
    for (room, _, _) in &scan.rooms {
        let key = (room.map, room.room);
        for n in [room.gen_monster, room.spec_mon] {
            if of.contains(&n) {
                named.entry(n).or_default().insert(key);
            }
        }
        let kind = (
            room.max_monsters > 0,
            room.room_type,
            room.gen_monster,
            room.mon_group,
            room.min_level,
            room.max_level,
        );
        let generated = made.entry(kind).or_insert_with(|| {
            scan.monsters
                .generated(room)
                .into_iter()
                .map(|m| m.number)
                .filter(|n| of.contains(n))
                .collect()
        });
        for &n in generated.iter().chain([&room.spec_mon]) {
            if of.contains(&n) {
                pooled.entry(n).or_default().insert(key);
            }
        }
    }
    of.iter()
        .map(|&n| {
            let rooms = named.remove(&n).or_else(|| pooled.remove(&n));
            (n, rooms.into_iter().flatten().collect())
        })
        .collect()
}

/// Every text block, its parts joined from 0 (as `WorldPack::text_block`), with its link.
fn text_blocks(pack: &WorldPack) -> Result<BTreeMap<i64, (i64, String)>, Error> {
    let mut st = pack
        .conn()
        .prepare("SELECT number, part, link, text FROM texts ORDER BY number, part")?;
    let rows = st.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, String>(3)?,
        ))
    })?;
    let mut out: BTreeMap<i64, (i64, String)> = BTreeMap::new();
    let mut next = HashMap::new();
    for row in rows {
        let (number, part, link, text) = row?;
        if part == 0 {
            out.insert(number, (link, text));
            next.insert(number, 1);
        } else if next.get(&number) == Some(&part) {
            if let Some((_, t)) = out.get_mut(&number) {
                t.push_str(&text);
            }
            next.insert(number, part + 1);
        }
    }
    Ok(out)
}

/// A `random` table's line: `NN:` (chests, gambling rewards).
fn random_line(line: &str) -> bool {
    let line = line.trim_start();
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    digits > 0 && line[digits..].starts_with(':')
}

/// The items a block's lines give (`giveitem <n>`), leaving out `random` tables.
fn given(text: &str) -> Vec<i64> {
    let mut out = Vec::new();
    for line in text.lines().filter(|l| !random_line(l)) {
        for clause in line.split(':') {
            let clause = clause.trim();
            let Some(rest) = clause
                .get(..8)
                .filter(|p| p.eq_ignore_ascii_case("giveitem"))
                .map(|_| &clause[8..])
            else {
                continue;
            };
            if let Some(n) = rest.split_whitespace().next().and_then(|n| n.parse().ok())
                && !out.contains(&n)
            {
                out.push(n);
            }
        }
    }
    out
}

/// The answers of a talk block (`KEYWORDS:text number` lines, `kb/navigation.md` "ASK").
fn answers(text: &str) -> Vec<i64> {
    text.lines()
        .filter_map(|l| l.rsplit_once(':'))
        .filter_map(|(_, n)| n.trim().parse().ok())
        .filter(|&n| n > 0)
        .collect()
}

/// The quests that give each item, with the rooms and monsters their blocks hang from: a
/// room's command text, a monster's talk text, or a chain of links and talk answers to one.
fn quests(scan: &Scan, wanted: &HashSet<i64>, sources: &mut HashMap<i64, Vec<Source>>) {
    let blocks = &scan.blocks;
    let mut giving: Vec<(i64, Vec<i64>)> = Vec::new();
    for (&number, (_, text)) in blocks {
        let items: Vec<i64> = given(text)
            .into_iter()
            .filter(|i| wanted.contains(i))
            .collect();
        if !items.is_empty() {
            giving.push((number, items));
        }
    }
    if giving.is_empty() {
        return;
    }
    // Who each block hangs from: the roots, and the blocks that lead to it.
    let mut roots: HashMap<i64, Vec<QuestAt>> = HashMap::new();
    for (room, _, text) in &scan.rooms {
        if *text > 0 {
            roots
                .entry(*text)
                .or_default()
                .push(QuestAt::Room((room.map, room.room)));
        }
    }
    let mut talk = HashSet::new();
    for m in scan.monsters.iter() {
        let text = scan.records.get(&m.number).map_or(0, |v| v.talk_text);
        if text > 0 {
            talk.insert(text);
            roots.entry(text).or_default().push(QuestAt::Monster {
                number: m.number,
                name: m.name.clone(),
            });
        }
    }
    let mut parents: HashMap<i64, Vec<i64>> = HashMap::new();
    for (&number, (link, text)) in blocks {
        if *link > 0 {
            parents.entry(*link).or_default().push(number);
        }
        if talk.contains(&number) {
            for answer in answers(text) {
                parents.entry(answer).or_default().push(number);
            }
        }
    }
    for (block, items) in giving {
        let mut at: Vec<QuestAt> = Vec::new();
        let mut seen = HashSet::from([block]);
        let mut queue = VecDeque::from([block]);
        while let Some(b) = queue.pop_front() {
            for r in roots.get(&b).into_iter().flatten() {
                if !at.contains(r) {
                    at.push(r.clone());
                }
            }
            for &p in parents.get(&b).into_iter().flatten() {
                if seen.insert(p) {
                    queue.push_back(p);
                }
            }
        }
        at.sort();
        for item in items {
            sources.entry(item).or_default().push(Source::Quest {
                block,
                at: at.clone(),
            });
        }
    }
}

/// The rooms each item is placed in (`placed[]`).
fn placed(scan: &Scan, wanted: &HashSet<i64>, sources: &mut HashMap<i64, Vec<Source>>) {
    for (room, list, _) in &scan.rooms {
        let mut seen = HashSet::new();
        for &item in list {
            if wanted.contains(&item) && seen.insert(item) {
                sources.entry(item).or_default().push(Source::Placed {
                    room: (room.map, room.room),
                });
            }
        }
    }
}

/// One monster of a set a weapon is measured against.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetMonster {
    pub number: i64,
    pub name: String,
    pub boss: bool,
    /// How many of the set's rooms can spawn it.
    pub rooms: usize,
}

/// The monsters of some rooms' spawn pools, each once, in the order first met, bosses kept.
pub fn monsters_of_rooms<'a>(
    monsters: &Monsters,
    rooms: impl IntoIterator<Item = &'a Room>,
) -> Vec<SetMonster> {
    let mut out: Vec<SetMonster> = Vec::new();
    for room in rooms {
        for m in monsters.spawn_pool(room) {
            match out.iter_mut().find(|s| s.number == m.number) {
                Some(s) => s.rooms += 1,
                None => out.push(SetMonster {
                    number: m.number,
                    name: m.name.clone(),
                    boss: monsters.is_boss(m.number),
                    rooms: 1,
                }),
            }
        }
    }
    out
}

/// A loop's monsters: those of its stop rooms' spawn pools (`MmLoop::stops`, `Loop::rooms`).
pub fn loop_monsters(
    pack: &WorldPack,
    monsters: &Monsters,
    stops: &[Key],
) -> Result<Vec<SetMonster>, Error> {
    let mut rooms = Vec::new();
    for &(map, room) in stops {
        rooms.extend(pack.room(map, room)?);
    }
    Ok(monsters_of_rooms(monsters, &rooms))
}

/// An area's monsters: the spawn pools of the rooms named `name`, or whose name starts with
/// it followed by a comma (`Slum Street` takes `Slum Street, Bend`), any case.
pub fn area_monsters(
    pack: &WorldPack,
    monsters: &Monsters,
    name: &str,
) -> Result<Vec<SetMonster>, Error> {
    let name = name.trim().to_lowercase();
    let rooms: Vec<Room> = pack
        .all_rooms()?
        .into_iter()
        .filter(|r| {
            let n = r.name.to_lowercase();
            n == name
                || n.strip_prefix(&name)
                    .is_some_and(|rest| rest.starts_with(','))
        })
        .collect();
    Ok(monsters_of_rooms(monsters, &rooms))
}

/// Monsters by name (as `Monsters::by_name`), each once; the names that match none come back
/// as the second list.
pub fn named_monsters<S: AsRef<str>>(
    monsters: &Monsters,
    names: &[S],
) -> (Vec<SetMonster>, Vec<String>) {
    let mut out: Vec<SetMonster> = Vec::new();
    let mut unknown = Vec::new();
    for name in names {
        let found = monsters.by_name(name.as_ref());
        if found.is_empty() {
            unknown.push(name.as_ref().to_owned());
        }
        for m in found {
            if !out.iter().any(|s| s.number == m.number) {
                out.push(SetMonster {
                    number: m.number,
                    name: m.name.clone(),
                    boss: monsters.is_boss(m.number),
                    rooms: 0,
                });
            }
        }
    }
    (out, unknown)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(weapon_kind: i64, armour: i64, anti_magic: bool) -> User {
        User {
            class: 4,
            race: 2,
            weapon_kind,
            armour,
            anti_magic,
        }
    }

    fn weapon(number: i64, weapon_type: i64) -> Needs {
        Needs {
            number,
            item_type: ITEM_WEAPON,
            weapon_type,
            ..Needs::default()
        }
    }

    #[test]
    fn a_weapon_kind_allows_its_weapon_types() {
        let allowed = |kind| {
            (0..4)
                .filter(|&t| kind_allows(kind, t, 1))
                .collect::<Vec<_>>()
        };
        assert_eq!(allowed(0), [0]);
        assert_eq!(allowed(3), [3]);
        assert_eq!(allowed(4), [0, 2]);
        assert_eq!(allowed(5), [1, 3]);
        assert_eq!(allowed(6), [2, 3]);
        assert_eq!(allowed(7), [0, 1]);
        assert_eq!(allowed(8), [0, 1, 2, 3]);
        // Limited: the quarterstaff and item 68, whatever their type.
        assert!(allowed(KIND_LIMITED).is_empty());
        assert!(kind_allows(KIND_LIMITED, 1, 100) && kind_allows(KIND_LIMITED, 3, 68));
    }

    #[test]
    fn naming_the_class_or_race_overrides_the_weapon_and_armour_types() {
        let blunt = user(7, 2, false);
        let sword = weapon(5, 2);
        assert_eq!(blunt.never(&sword), [Reason::WeaponType]);
        let named = Needs {
            classes: vec![4],
            ..sword.clone()
        };
        assert!(blunt.never(&named).is_empty());
        let other_class = Needs {
            classes: vec![9],
            races: vec![2],
            ..sword
        };
        // The race list names the Dwarf: the type is fine, but the class list is not.
        assert_eq!(blunt.never(&other_class), [Reason::Class]);
        let shield = Needs {
            item_type: ITEM_ARMOUR,
            armour_type: 5,
            worn: WORN_OFF_HAND,
            ..Needs::default()
        };
        assert_eq!(
            user(4, 2, false).never(&shield),
            [Reason::ArmourType, Reason::OffHand]
        );
        assert!(user(5, 6, false).never(&shield).is_empty());
    }

    #[test]
    fn level_and_alignment_are_later_and_the_rest_never() {
        let u = user(8, 9, true);
        let mut n = weapon(5, 0);
        n.min_level = Some(15);
        n.alignments = vec![Alignment::Evil];
        let at = |n: &Needs, level| u.usability(n, level);
        assert_eq!(
            at(&n, 10),
            Usability {
                status: Status::Later,
                reasons: vec![Reason::Level(15), Reason::Alignment(Alignment::Evil)],
            }
        );
        n.alignments.clear();
        assert_eq!(at(&n, 15).status, Status::Now);
        n.max_level = Some(12);
        assert_eq!(at(&n, 15).reasons, [Reason::MaxLevel(12)]);
        assert_eq!(at(&n, 15).status, Status::Never);
        n.max_level = None;
        n.magical = true;
        assert_eq!(at(&n, 10).status, Status::Never);
        assert_eq!(at(&n, 10).reasons, [Reason::Magical, Reason::Level(15)]);
        let type_11 = Needs {
            item_type: ITEM_TYPE_11,
            ..Needs::default()
        };
        assert!(u.never(&type_11).is_empty());
        let mage = User { class: 12, ..u };
        assert_eq!(mage.never(&type_11), [Reason::Type11]);
    }

    #[test]
    fn buying_adds_the_markup_then_charm_and_selling_pays_by_charm() {
        // 25 gold at a 100% markup, Charm 75 (95%).
        assert_eq!(buy_price(25, 2, 100, Some(75)), 4750);
        assert_eq!(buy_price(25, 2, 0, None), 2500);
        // 180 gold at Charm 75: 62%.
        assert_eq!(sell_price(180, 2, Some(75)), 11_160);
        assert_eq!(sell_price(180, 2, None), 9000);
    }

    #[test]
    fn a_quest_line_gives_items_and_a_random_table_does_not() {
        let text = "failability 50 850:class 2:minlevel 15 851:giveitem 646:text 400\n\
                    10:giveitem 12\n\
                    open chest:GIVEITEM 7 1:giveitem 646\n";
        assert_eq!(given(text), [646, 7]);
        assert!(random_line(" 55:giveitem 3") && !random_line("sit throne:giveitem 3"));
        assert_eq!(answers("sword:398\nwitchunter:395\nhello\n"), [398, 395]);
    }
}
