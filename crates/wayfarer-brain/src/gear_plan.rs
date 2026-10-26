//! The gear plan (card C-059; docs/brain.md "Gear plan"): every weapon a character's class and
//! race could ever use, each scored for an attack, a bash and a smash against a set of
//! monsters, from the character's own values or the player's what-ifs, with the buffs the
//! player ticks. Solo play only.
//!
//! The scores are [`wayfarer_world::combat`]'s (a [`Fight`] per weapon, move and monster),
//! the weapons and their sources [`wayfarer_world::gear`]'s [`Catalogue`], built once per class
//! and race and kept. Nothing else is cached: a whole plan takes a few tens of milliseconds.
//! The brain answers on request only.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::Value;
use wayfarer_proto::{
    GearBuff, GearDetailMove, GearFilter, GearInputs, GearMonster, GearMonsterScore, GearMove,
    GearMoveScore, GearRow, GearSource, GearSourceKind, GearStatus, GearWhatIf, RoomKey,
};
use wayfarer_world::combat::{
    self, Buff, ClassRecord, Defender, Fight, Fighter, ItemRecord, MonsterRecord, Move,
    ROUNDS_REPORTED, RaceRecord, STOCK_MAX_ENERGY, Stats, ability, blows_per_round,
};
use wayfarer_world::gear::{
    Alignment, Catalogue, GearItem, Kind, QuestAt, Reason, SetMonster, Source, Status, Wearer,
};
use wayfarer_world::{Key, WorldPack};

use crate::Atlas;

/// The classes that may keep Smash, and the level they may from (`kb/combat.md` "Attack
/// modes").
pub const SMASH_FROM: [(&str, i64); 6] = [
    ("Warrior", 22),
    ("Witchunter", 20),
    ("Paladin", 25),
    ("Cleric", 27),
    ("Warlock", 27),
    ("Ranger", 22),
];

/// A spell's targets a buff may have: Single, Self, Self or Single, Any, and the party ones
/// (`kb/spells.md` "Spell record").
const BUFF_TARGETS: [i64; 6] = [0, 1, 2, 6, 10, 13];

/// What the Anti-Magic character's rows say when buffs are ticked.
const ANTI_MAGIC_NOTE: &str =
    "Anti-Magic resists friendly buffs some of the time: a ticked buff may not take";

/// Item abilities read here.
const ABILITY_MIN_LEVEL: i64 = 135;
/// `type` of a weapon.
const ITEM_WEAPON: i64 = 1;
const ABILITY_STR: i64 = 46;
const ABILITY_AGI: i64 = 48;

/// The weapon and armour records, the classes and races, and the buffs offered: read from the
/// pack the first time a plan is asked for.
struct Records {
    items: HashMap<i64, ItemRecord>,
    /// Lower-case name → number, the first of a name.
    by_name: HashMap<String, i64>,
    classes: HashMap<i64, ClassRecord>,
    races: HashMap<i64, RaceRecord>,
    buffs: Vec<Offered>,
    /// The areas a set may name (`GearPlan::areas`).
    areas: Vec<String>,
}

/// A buff the plan offers.
#[derive(Clone, Debug)]
struct Offered {
    wire: GearBuff,
    buff: Buff,
}

/// The gear planner's data, shared by every session: the records, read once, and a
/// [`Catalogue`] per class and race, built when first asked for.
pub struct Gear {
    pack: Arc<Mutex<WorldPack>>,
    atlas: Arc<Atlas>,
    records: OnceLock<Result<Arc<Records>, String>>,
    catalogues: Mutex<HashMap<(i64, i64), Arc<Catalogue>>>,
}

/// The character a plan is for, as the brain knows it; `None` for what it does not know yet.
#[derive(Clone, Debug, Default)]
pub struct Subject {
    pub class: Option<String>,
    pub race: Option<String>,
    pub level: Option<i64>,
    /// Str, Int, Wil, Agi, Hea, Cha.
    pub stats: [Option<i64>; 6],
    /// The load from `i`: (carried, can carry).
    pub load: Option<(i64, i64)>,
    /// The readied weapon, and the worn items, by name.
    pub readied: Option<String>,
    pub worn: Vec<String>,
}

/// What the player asked for.
#[derive(Clone, Debug, Default)]
pub struct Ask {
    pub what_if: GearWhatIf,
    pub buffs: Vec<String>,
    pub smash: bool,
    pub filter: GearFilter,
}

/// A plan's answer, the session aside.
#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub inputs: GearInputs,
    pub monsters: Vec<GearMonster>,
    pub buffs_offered: Vec<GearBuff>,
    pub rows: Vec<GearRow>,
    pub missing: Vec<String>,
    pub notes: Vec<String>,
    pub areas: Vec<String>,
}

/// A detail's answer, the session aside.
#[derive(Clone, Debug, Default)]
pub struct Detail {
    pub name: String,
    pub inputs: GearInputs,
    pub monsters: Vec<GearMonster>,
    pub load_percent: Option<i64>,
    pub moves: Vec<GearDetailMove>,
    pub missing: Vec<String>,
    pub notes: Vec<String>,
}

/// The values a plan scores with, all known.
struct Values {
    class: i64,
    race: i64,
    level: i64,
    stats: Stats,
    charm: i64,
    /// The load, per cent, and whether it came from the what-if (no weapon swapped in).
    load: LoadFrom,
    worn: Vec<i64>,
    buffs: Vec<Buff>,
    smash: bool,
}

#[derive(Clone, Copy, Debug)]
enum LoadFrom {
    WhatIf(i64),
    /// Carried and capacity, and the readied weapon's weight when it is known.
    Live {
        carried: i64,
        capacity: i64,
        readied: Option<i64>,
    },
}

impl LoadFrom {
    fn base(self) -> i64 {
        match self {
            LoadFrom::WhatIf(p) => p,
            LoadFrom::Live {
                carried, capacity, ..
            } => percent(carried, capacity),
        }
    }

    /// The load with `weapon` readied in place of the readied one.
    fn with(self, weapon: &ItemRecord) -> i64 {
        match self {
            LoadFrom::WhatIf(p) => p,
            LoadFrom::Live {
                carried,
                capacity,
                readied: Some(w),
            } => percent((carried - w + weapon.weight).max(0), capacity),
            LoadFrom::Live {
                carried, capacity, ..
            } => percent(carried, capacity),
        }
    }
}

fn percent(carried: i64, capacity: i64) -> i64 {
    if capacity > 0 {
        carried * 100 / capacity
    } else {
        0
    }
}

/// A monster of the set, ready to fight.
struct Foe {
    wire: GearMonster,
    defender: Defender,
    /// Its weight in the averages: the rooms that spawn it, at least 1.
    weight: f64,
}

impl Gear {
    pub fn new(pack: Arc<Mutex<WorldPack>>, atlas: Arc<Atlas>) -> Gear {
        Gear {
            pack,
            atlas,
            records: OnceLock::new(),
            catalogues: Mutex::new(HashMap::new()),
        }
    }

    fn records(&self) -> Result<Arc<Records>, String> {
        self.records
            .get_or_init(|| {
                let pack = self.pack.lock().unwrap_or_else(|p| p.into_inner());
                Records::load(&pack, &self.atlas)
                    .map(Arc::new)
                    .map_err(|e| format!("gear plan: the world pack: {e}"))
            })
            .clone()
    }

    /// The catalogue of a class and race, built the first time.
    fn catalogue(&self, class: i64, race: i64) -> Result<Arc<Catalogue>, String> {
        let mut cats = self.catalogues.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(c) = cats.get(&(class, race)) {
            return Ok(c.clone());
        }
        let pack = self.pack.lock().unwrap_or_else(|p| p.into_inner());
        let cat = Catalogue::build(&pack, class, race)
            .map(Arc::new)
            .map_err(|e| format!("gear plan: the gear catalogue: {e}"))?;
        cats.insert((class, race), cat.clone());
        Ok(cat)
    }

    /// The monsters of an area (`gear::area_monsters`).
    pub fn area_monsters(&self, name: &str) -> Result<Vec<SetMonster>, String> {
        let pack = self.pack.lock().unwrap_or_else(|p| p.into_inner());
        wayfarer_world::gear::area_monsters(&pack, &self.atlas.monsters, name)
            .map_err(|e| format!("gear plan: the area {name:?}: {e}"))
    }

    /// The monsters of some rooms' spawn pools.
    pub fn room_monsters(&self, rooms: &[Key]) -> Vec<SetMonster> {
        let rooms = rooms.iter().filter_map(|&k| self.atlas.graph.room(k));
        wayfarer_world::gear::monsters_of_rooms(&self.atlas.monsters, rooms)
    }

    /// Monsters by name, and the names that match none.
    pub fn named_monsters(&self, names: &[String]) -> (Vec<SetMonster>, Vec<String>) {
        wayfarer_world::gear::named_monsters(&self.atlas.monsters, names)
    }

    /// The buffs offered.
    pub fn buffs_offered(&self) -> Vec<GearBuff> {
        self.records()
            .map(|r| r.buffs.iter().map(|b| b.wire.clone()).collect())
            .unwrap_or_default()
    }

    /// A character's plan against `set`. `reach` gives the steps to the nearest of some rooms
    /// and which it is, for the shops. An `Err` is a fault of the pack, not of the request.
    pub fn plan(
        &self,
        who: &Subject,
        ask: &Ask,
        set: &[SetMonster],
        reach: impl Fn(&[Key]) -> Option<(u32, Key)>,
    ) -> Result<Plan, String> {
        let records = self.records()?;
        let mut plan = Plan {
            buffs_offered: records.buffs.iter().map(|b| b.wire.clone()).collect(),
            areas: records.areas.clone(),
            ..Plan::default()
        };
        let values = match self.values(&records, who, ask, &mut plan.inputs, &mut plan.notes) {
            Ok(v) => v,
            Err(missing) => {
                plan.missing = missing;
                return Ok(plan);
            }
        };
        let foes = self.foes(&records, set);
        plan.monsters = foes.iter().map(|f| f.wire.clone()).collect();
        if foes.is_empty() {
            plan.missing.push("monsters".to_owned());
            return Ok(plan);
        }
        let cat = self.catalogue(values.class, values.race)?;
        let who_wears = Wearer {
            level: values.level,
            charm: Some(values.charm),
            strength: Some(values.stats.str),
        };
        let mut items: Vec<GearItem> = cat
            .for_level(who_wears, reach)
            .into_iter()
            .filter(|i| i.kind == Kind::Weapon)
            .filter(|i| match ask.filter {
                GearFilter::Now => i.usability.status == Status::Now,
                GearFilter::Later => i.usability.status == Status::Later,
                GearFilter::All => true,
            })
            .collect();
        items.sort_by(|a, b| {
            (a.usability.status, a.name.to_lowercase(), a.number).cmp(&(
                b.usability.status,
                b.name.to_lowercase(),
                b.number,
            ))
        });
        let note = (cat.user.anti_magic && !values.buffs.is_empty()).then_some(ANTI_MAGIC_NOTE);
        let mut jobs = Vec::new();
        for item in &items {
            let Some(record) = records.items.get(&item.number) else {
                continue;
            };
            let mut row = self.row(item, record);
            if item.usability.status != Status::Never {
                let load = values.load.with(record);
                row.load_percent = Some(load);
                row.note = note.map(str::to_owned);
                jobs.push((plan.rows.len(), fighter(&records, &values, record, load)));
            }
            plan.rows.push(row);
        }
        let mut best: Option<(usize, Rank)> = None;
        for (i, scores) in score_all(&jobs, &foes, moves(values.smash)) {
            let row = &mut plan.rows[i];
            let fastest = scores.iter().min_by(|a, b| a.1.cmp(&b.1));
            if let Some((mv, rank)) = fastest.map(|(s, r)| (s.mv, *r)) {
                row.best_move = Some(mv);
                if best.is_none_or(|(_, b)| rank < b) {
                    best = Some((i, rank));
                }
            }
            row.moves = scores.into_iter().map(|(s, _)| s).collect();
        }
        if let Some((i, _)) = best {
            plan.rows[i].best = true;
        }
        Ok(plan)
    }

    /// One weapon against each monster of `set`, every number per monster and per move.
    pub fn detail(
        &self,
        who: &Subject,
        ask: &Ask,
        item: i64,
        set: &[SetMonster],
    ) -> Result<Detail, String> {
        let records = self.records()?;
        let mut d = Detail::default();
        let values = match self.values(&records, who, ask, &mut d.inputs, &mut d.notes) {
            Ok(v) => v,
            Err(missing) => {
                d.missing = missing;
                return Ok(d);
            }
        };
        let Some(record) = records
            .items
            .get(&item)
            .filter(|r| r.item_type == ITEM_WEAPON)
        else {
            d.notes.push(format!("no weapon number {item}"));
            return Ok(d);
        };
        d.name = record.name.clone();
        let foes = self.foes(&records, set);
        d.monsters = foes.iter().map(|f| f.wire.clone()).collect();
        if foes.is_empty() {
            d.missing.push("monsters".to_owned());
            return Ok(d);
        }
        let cat = self.catalogue(values.class, values.race)?;
        let who_wears = Wearer {
            level: values.level,
            charm: Some(values.charm),
            strength: Some(values.stats.str),
        };
        if let Some(i) = cat
            .for_level(who_wears, |_| None)
            .into_iter()
            .find(|i| i.number == item)
        {
            let reasons = i.usability.reasons;
            if !reasons.is_empty() {
                let why: Vec<String> = reasons.iter().map(|&r| reason_text(r)).collect();
                d.notes
                    .push(format!("{} is not usable now: {}", d.name, why.join(", ")));
            }
            if cat.user.anti_magic && !values.buffs.is_empty() {
                d.notes.push(ANTI_MAGIC_NOTE.to_owned());
            }
        }
        let load = values.load.with(record);
        d.load_percent = Some(load);
        let fighter = fighter(&records, &values, record, load);
        for &mv in moves(values.smash) {
            let cost = fighter.cost(mv);
            d.moves.push(GearDetailMove {
                mv: wire_move(mv),
                energy: cost,
                blows_per_round: blows_per_round(cost, fighter.max_energy, ROUNDS_REPORTED),
                monsters: foes
                    .iter()
                    .map(|f| monster_score(&Fight::new(&fighter, &f.defender, mv), f))
                    .collect(),
            });
        }
        Ok(d)
    }

    /// The values to score with, the inputs as the wire shows them, and the buffs ticked;
    /// `Err` with what is missing.
    fn values(
        &self,
        records: &Records,
        who: &Subject,
        ask: &Ask,
        inputs: &mut GearInputs,
        notes: &mut Vec<String>,
    ) -> Result<Values, Vec<String>> {
        let w = &ask.what_if;
        let mut from_what_if = Vec::new();
        let mut pick = |name: &str, what_if: Option<i64>, live: Option<i64>| {
            if what_if.is_some() {
                from_what_if.push(name.to_owned());
            }
            what_if.or(live)
        };
        let level = pick("level", w.level, who.level);
        let [s, i, wi, a, h, c] = who.stats;
        let str = pick("str", w.str, s);
        let int = pick("int", w.int, i);
        let wil = pick("wil", w.wil, wi);
        let agi = pick("agi", w.agi, a);
        let hea = pick("hea", w.hea, h);
        let cha = pick("cha", w.cha, c);
        let readied = who
            .readied
            .as_ref()
            .and_then(|n| records.by_name.get(&n.to_lowercase()))
            .and_then(|n| records.items.get(n))
            .filter(|r| r.item_type == ITEM_WEAPON);
        let load = match (w.load_percent, who.load) {
            (Some(p), _) => {
                from_what_if.push("load_percent".to_owned());
                Some(LoadFrom::WhatIf(p))
            }
            (None, Some((carried, capacity))) => Some(LoadFrom::Live {
                carried,
                capacity,
                readied: readied.map(|r| r.weight),
            }),
            (None, None) => None,
        };
        let atlas = &self.atlas;
        let class = who.class.as_ref().and_then(|n| atlas.classes.get(n));
        let race = who.race.as_ref().and_then(|n| atlas.races.get(n));

        // The buffs ticked, by name in any case.
        let mut buffs = Vec::new();
        let mut buff_names = Vec::new();
        for name in &ask.buffs {
            match records
                .buffs
                .iter()
                .find(|b| b.wire.name.eq_ignore_ascii_case(name))
            {
                Some(b) => {
                    buffs.push(b.buff);
                    buff_names.push(b.wire.name.clone());
                }
                None => notes.push(format!("no buff {name:?}")),
            }
        }
        let smash = ask.smash
            && who.class.as_deref().is_some_and(|c| {
                SMASH_FROM
                    .iter()
                    .any(|&(k, from)| k == c && level.is_some_and(|l| l >= from))
            });

        *inputs = GearInputs {
            class: who.class.clone(),
            race: who.race.clone(),
            level,
            str,
            int,
            wil,
            agi,
            hea,
            cha,
            load_percent: load.map(LoadFrom::base),
            readied: match load {
                Some(LoadFrom::Live {
                    readied: Some(_), ..
                }) => who.readied.clone(),
                _ => None,
            },
            what_if: from_what_if,
            buffs: buff_names,
            smash,
        };

        // Class, race, level and the stats all come from the status screen (`st`).
        let mut missing = Vec::new();
        let screen = [level, str, int, agi, cha];
        if class.is_none() || race.is_none() || screen.iter().any(Option::is_none) {
            missing.push("stats".to_owned());
        }
        if load.is_none() {
            missing.push("load".to_owned());
        }
        let (
            Some(&class),
            Some(&race),
            Some(level),
            Some(str),
            Some(int),
            Some(agi),
            Some(cha),
            Some(load),
        ) = (class, race, level, str, int, agi, cha, load)
        else {
            return Err(missing);
        };
        if !records.classes.contains_key(&class) || !records.races.contains_key(&race) {
            return Err(vec!["stats".to_owned()]);
        }
        let worn = who
            .worn
            .iter()
            .filter_map(|n| records.by_name.get(&n.to_lowercase()).copied())
            .collect();
        Ok(Values {
            class,
            race,
            level,
            stats: Stats { str, int, agi, cha },
            charm: cha,
            load,
            worn,
            buffs,
            smash,
        })
    }

    /// The set's monsters, each with its defender: its record's numbers plus its weapon's and
    /// worn item's abilities.
    fn foes(&self, records: &Records, set: &[SetMonster]) -> Vec<Foe> {
        set.iter()
            .filter_map(|s| {
                let m = self.atlas.monsters.get(s.number)?;
                let rec = MonsterRecord::from_raw(&m.raw).ok()?;
                let items: Vec<&ItemRecord> = [rec.weapon, rec.wornitem]
                    .iter()
                    .filter_map(|n| records.items.get(n))
                    .collect();
                let defender = Defender::new(&rec, &items);
                Some(Foe {
                    wire: GearMonster {
                        number: rec.number,
                        name: rec.name.clone(),
                        hp: rec.maxhp,
                        ac: defender.dv,
                        dr: defender.dr,
                        dodge: defender.dodge,
                        rooms: u32::try_from(s.rooms).unwrap_or(u32::MAX),
                        boss: s.boss,
                    },
                    defender,
                    weight: s.rooms.max(1) as f64,
                })
            })
            .collect()
    }

    /// A row's catalogue part: the weapon, who may use it, where it comes from.
    fn row(&self, item: &GearItem, record: &ItemRecord) -> GearRow {
        GearRow {
            item: item.number,
            name: item.name.clone(),
            min_damage: record.mindmg,
            max_damage: record.maxdmg,
            speed: record.speed,
            str_req: record.strreq,
            str_short: item.str_short,
            min_level: record
                .abilities
                .iter()
                .find(|(a, _)| *a == ABILITY_MIN_LEVEL)
                .map(|&(_, v)| v),
            two_handed: matches!(item.weapon_type, 1 | 3),
            status: match item.usability.status {
                Status::Now => GearStatus::Now,
                Status::Later => GearStatus::Later,
                Status::Never => GearStatus::Never,
            },
            reasons: item
                .usability
                .reasons
                .iter()
                .map(|&r| reason_text(r))
                .collect(),
            sources: item.sources.iter().map(|s| self.source(s)).collect(),
            limited: item.limited,
            deleted_at_cleanup: item.deleted_at_cleanup,
            load_percent: None,
            moves: Vec::new(),
            best_move: None,
            best: false,
            note: None,
        }
    }

    fn source(&self, s: &Source) -> GearSource {
        let room_name = |k: Key| {
            self.atlas
                .graph
                .room(k)
                .map_or_else(String::new, |r| r.name.clone())
        };
        let mut out = GearSource {
            kind: GearSourceKind::Placed,
            name: String::new(),
            price: None,
            percent: None,
            room: None,
            steps: None,
            boss: false,
            first_kill: false,
        };
        match s {
            Source::Shop {
                shop_name,
                price,
                room,
                steps,
                ..
            } => {
                out.kind = GearSourceKind::Shop;
                out.name = shop_name.clone();
                out.price = Some(*price);
                out.room = room.map(room_key);
                out.steps = *steps;
            }
            Source::Drop {
                name,
                percent,
                boss,
                first_kill,
                rooms,
                ..
            } => {
                out.kind = GearSourceKind::Drop;
                out.name = name.clone();
                out.percent = Some(*percent);
                out.room = rooms.first().copied().map(room_key);
                out.boss = *boss;
                out.first_kill = *first_kill;
            }
            Source::Quest { at, .. } => {
                out.kind = GearSourceKind::Quest;
                // Who gives it, else where.
                let monster = at.iter().find_map(|a| match a {
                    QuestAt::Monster { name, .. } => Some(name.clone()),
                    QuestAt::Room(_) => None,
                });
                let room = at.iter().find_map(|a| match a {
                    QuestAt::Room(k) => Some(*k),
                    QuestAt::Monster { .. } => None,
                });
                out.name = monster.unwrap_or_else(|| room.map(room_name).unwrap_or_default());
                out.room = room.map(room_key);
            }
            Source::Placed { room } => {
                out.name = room_name(*room);
                out.room = Some(room_key(*room));
            }
        }
        out
    }
}

impl Records {
    fn load(pack: &WorldPack, atlas: &Atlas) -> Result<Records, wayfarer_world::Error> {
        let mut items = HashMap::new();
        let mut by_name = HashMap::new();
        for t in [combat::ITEM_ARMOUR, ITEM_WEAPON] {
            for i in pack.items_of_type(t)? {
                let rec = ItemRecord::from_raw(&i.raw)?;
                by_name.entry(rec.name.to_lowercase()).or_insert(rec.number);
                items.insert(rec.number, rec);
            }
        }
        let classes = pack
            .classes_raw()?
            .iter()
            .map(|(n, raw)| Ok((*n, ClassRecord::from_raw(raw)?)))
            .collect::<Result<_, wayfarer_world::Error>>()?;
        let races = pack
            .races_raw()?
            .iter()
            .map(|(n, raw)| Ok((*n, RaceRecord::from_raw(raw)?)))
            .collect::<Result<_, wayfarer_world::Error>>()?;
        Ok(Records {
            items,
            by_name,
            classes,
            races,
            buffs: offered_buffs(pack, &atlas.class_magery)?,
            areas: spawn_areas(atlas),
        })
    }
}

/// The names, up to a comma, of the rooms that spawn monsters, sorted: what an `{area}` set
/// may name. A name's rooms are looked at until one spawns.
fn spawn_areas(atlas: &Atlas) -> Vec<String> {
    let mut areas = std::collections::BTreeSet::new();
    for node in atlas.graph.nodes() {
        let area = node.room.name.split(',').next().unwrap_or("").trim();
        if area.is_empty() || areas.contains(area) {
            continue;
        }
        if !atlas.monsters.spawn_pool(&node.room).is_empty() {
            areas.insert(area.to_owned());
        }
    }
    areas.into_iter().collect()
}

/// Every lasting spell of a class's magery that is cast on a player or the party and raises
/// AV, damage, the critical chance, Str or Agi, or lowers the energy a blow costs. A slot
/// whose value is 0 takes the spell's strength at the level it is learned (the middle of its
/// range). Spells of level 0 are left out: no character learns them (the items' spells, such
/// as cross of vengeance).
fn offered_buffs(
    pack: &WorldPack,
    magery: &HashMap<String, (i64, i64)>,
) -> Result<Vec<Offered>, wayfarer_world::Error> {
    // By name: those of one name that do the same are one offer, those that differ are told
    // apart by their classes.
    let mut by_name: BTreeMap<String, Vec<(Buff, Vec<String>)>> = BTreeMap::new();
    for s in pack.spells()? {
        let Ok(v) = serde_json::from_str::<Value>(&s.raw) else {
            continue;
        };
        let int = |k: &str| v.get(k).and_then(Value::as_i64).unwrap_or(0);
        let ints = |k: &str| -> Vec<i64> {
            v.get(k)
                .and_then(Value::as_array)
                .map(|l| l.iter().filter_map(Value::as_i64).collect())
                .unwrap_or_default()
        };
        let (m, m_level) = (int("magery"), int("magelvl"));
        if m == 0 || s.level <= 0 || int("duration") <= 0 || !BUFF_TARGETS.contains(&s.target) {
            continue;
        }
        let mut classes: Vec<String> = magery
            .iter()
            .filter(|(_, (cm, cl))| *cm == m && *cl >= m_level)
            .map(|(name, _)| name.clone())
            .collect();
        if classes.is_empty() {
            continue;
        }
        classes.sort();
        let strength = {
            let l = match int("levelcap") {
                cap if cap > 0 => s.level.min(cap),
                _ => s.level,
            };
            let at = |base: i64, inc: i64, lvls: i64| {
                if lvls > 0 {
                    base + inc * l / lvls
                } else {
                    base
                }
            };
            let hi = at(int("maxval"), int("maxinc"), int("maxlvls"));
            let lo = at(int("minval"), int("mininc"), int("minlvls")).min(hi);
            (lo + hi) / 2
        };
        let values = ints("abilvals");
        let mut buff = Buff::default();
        for (i, &a) in ints("abilities").iter().enumerate() {
            let value = match values.get(i).copied().unwrap_or(0) {
                0 => strength,
                v => v,
            };
            match a {
                ability::AV_BLESS | ability::AV_CURSE | ability::AV_THIRD => {
                    buff.av = if buff.av == 0 {
                        value
                    } else {
                        buff.av.max(value)
                    };
                }
                ability::DAMAGE => buff.max_damage += value,
                ability::CRITICAL => buff.crit += value,
                ABILITY_STR => buff.str += value,
                ABILITY_AGI => buff.agi += value,
                ability::ENERGY_USE => buff.energy_pct = value,
                _ => {}
            }
        }
        let raises = buff.av > 0
            || buff.max_damage > 0
            || buff.crit > 0
            || buff.str > 0
            || buff.agi > 0
            || (buff.energy_pct > 0 && buff.energy_pct < 100);
        if !raises {
            continue;
        }
        let same = by_name.entry(s.name.clone()).or_default();
        match same.iter_mut().find(|(b, _)| *b == buff) {
            Some((_, cs)) => {
                cs.extend(classes);
                cs.sort();
                cs.dedup();
            }
            None => same.push((buff, classes)),
        }
    }
    let mut out = Vec::new();
    for (spell, kinds) in by_name {
        let several = kinds.len() > 1;
        for (buff, classes) in kinds {
            let name = if several {
                format!("{spell} ({})", classes.join("/"))
            } else {
                spell.clone()
            };
            out.push(Offered {
                wire: GearBuff {
                    effect: format!("{spell}: {}", effect(&buff)),
                    name,
                    classes,
                },
                buff,
            });
        }
    }
    Ok(out)
}

/// What a buff does, in words: `+5 AV, +1 max damage`.
fn effect(b: &Buff) -> String {
    let mut parts = Vec::new();
    let signed = |v: i64, what: &str| format!("{v:+} {what}");
    if b.av != 0 {
        parts.push(signed(b.av, "AV"));
    }
    if b.max_damage != 0 {
        parts.push(signed(b.max_damage, "max damage"));
    }
    if b.crit != 0 {
        parts.push(signed(b.crit, "crit"));
    }
    if b.str != 0 {
        parts.push(signed(b.str, "Str"));
    }
    if b.agi != 0 {
        parts.push(signed(b.agi, "Agi"));
    }
    if b.energy_pct != 0 {
        parts.push(format!("blows cost {}%", b.energy_pct));
    }
    parts.join(", ")
}

/// The moves scored: attack and bash, and smash when asked for and allowed.
fn moves(smash: bool) -> &'static [Move] {
    if smash { &Move::ALL } else { &Move::ALL[..2] }
}

fn fighter(records: &Records, v: &Values, weapon: &ItemRecord, load: i64) -> Fighter {
    let worn = v.worn.iter().filter_map(|n| records.items.get(n)).collect();
    Fighter::new(&combat::Character {
        class: &records.classes[&v.class],
        race: &records.races[&v.race],
        level: v.level,
        stats: v.stats,
        weapon: Some(weapon),
        worn,
        encumbrance: load,
        max_energy: STOCK_MAX_ENERGY,
        buffs: v.buffs.clone(),
    })
}

/// How a move or row ranks: fewer expected rounds first, then the higher chance of a kill in
/// round 1.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Rank {
    rounds: f64,
    kill1: f64,
}

impl Eq for Rank {}

impl PartialOrd for Rank {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Rank {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.rounds
            .total_cmp(&other.rounds)
            .then(other.kill1.total_cmp(&self.kill1))
    }
}

/// Every fighter's moves against the set, each fighter's scores beside its row's index. The
/// fights are most of a plan's time, so the fighters are shared among threads, one per core.
fn score_all(
    jobs: &[(usize, Fighter)],
    foes: &[Foe],
    moves: &[Move],
) -> Vec<(usize, Vec<(GearMoveScore, Rank)>)> {
    let one =
        |(i, f): &(usize, Fighter)| (*i, moves.iter().map(|&mv| score(f, foes, mv)).collect());
    let threads = std::thread::available_parallelism().map_or(1, usize::from);
    if threads < 2 || jobs.len() < 2 {
        return jobs.iter().map(one).collect();
    }
    let chunk = jobs.len().div_ceil(threads);
    std::thread::scope(|s| {
        let handles: Vec<_> = jobs
            .chunks(chunk)
            .map(|part| s.spawn(move || part.iter().map(one).collect::<Vec<_>>()))
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap_or_else(|p| std::panic::resume_unwind(p)))
            .collect()
    })
}

/// One move against the set, averaged over it by the monsters' weights.
fn score(f: &Fighter, foes: &[Foe], mv: Move) -> (GearMoveScore, Rank) {
    let cost = f.cost(mv);
    let total: f64 = foes.iter().map(|x| x.weight).sum();
    let (mut hit, mut crit, mut dmg1, mut kill1, mut kill2, mut rounds) =
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    let mut capped = false;
    for foe in foes {
        let fight = Fight::new(f, &foe.defender, mv);
        let w = foe.weight / total;
        hit += w * fight.blow.p_hit;
        crit += w * fight.blow.p_crit;
        dmg1 += w * fight.expected_damage_round1;
        kill1 += w * fight.p_kill_by_round[0];
        kill2 += w * fight.p_kill_by_round[1];
        rounds += w * fight.expected_rounds;
        capped |= fight.rounds_capped;
    }
    let score = GearMoveScore {
        mv: wire_move(mv),
        energy: cost,
        blows: blows_per_round(cost, f.max_energy, 3),
        hit: thousandths(hit),
        crit: thousandths(crit),
        damage_round1: tenths(dmg1),
        kill_round1: thousandths(kill1),
        kill_round2: thousandths(kill2),
        rounds: hundredths(rounds),
        rounds_capped: capped,
    };
    (score, Rank { rounds, kill1 })
}

fn monster_score(fight: &Fight, foe: &Foe) -> GearMonsterScore {
    let b = &fight.blow;
    GearMonsterScore {
        monster: foe.wire.number,
        hit: thousandths(b.p_hit),
        crit: thousandths(b.p_crit),
        dodge: thousandths(b.p_dodge),
        damage: vec![b.damage.0, b.damage.1],
        crit_damage: b.crit_damage.map(|(lo, hi)| vec![lo, hi]),
        damage_per_blow: tenths(fight.expected_damage_per_blow),
        damage_round1: tenths(fight.expected_damage_round1),
        damage_round2: tenths(fight.expected_damage_round2),
        kill_by_round: fight
            .p_kill_by_round
            .iter()
            .map(|&p| thousandths(p))
            .collect(),
        rounds: hundredths(fight.expected_rounds),
        rounds_capped: fight.rounds_capped,
        knocks_down: fight.down_blow.is_some(),
    }
}

fn scaled(x: f64, by: f64) -> u32 {
    // Rounded, and never below 0 (a sum of chances can come out a hair under).
    (x * by).round().clamp(0.0, f64::from(u32::MAX)) as u32
}

fn thousandths(p: f64) -> u32 {
    scaled(p, 1000.0)
}

fn tenths(x: f64) -> u32 {
    scaled(x, 10.0)
}

fn hundredths(x: f64) -> u32 {
    scaled(x, 100.0)
}

pub(crate) fn wire_move(mv: Move) -> GearMove {
    match mv {
        Move::Attack => GearMove::Attack,
        Move::Bash => GearMove::Bash,
        Move::Smash => GearMove::Smash,
    }
}

fn room_key(k: Key) -> RoomKey {
    RoomKey {
        map: k.0,
        room: k.1,
    }
}

/// A reason in words, as the planner shows it.
fn reason_text(r: Reason) -> String {
    match r {
        Reason::Class => "class".to_owned(),
        Reason::Race => "race".to_owned(),
        Reason::WeaponType => "weapon type".to_owned(),
        Reason::ArmourType => "armour type".to_owned(),
        Reason::Magical => "magical".to_owned(),
        Reason::OffHand => "off-hand".to_owned(),
        Reason::Type11 => "item type".to_owned(),
        Reason::Level(n) => format!("level {n}"),
        Reason::MaxLevel(n) => format!("max level {n}"),
        Reason::Alignment(a) => format!(
            "alignment {}",
            match a {
                Alignment::Good => "good",
                Alignment::Evil => "evil",
                Alignment::NotGood => "not good",
                Alignment::NotEvil => "not evil",
                Alignment::Neutral => "neutral",
            }
        ),
    }
}
