//! Reading a world pack. Opening is cheap: nothing is loaded until asked for.

use std::path::Path;

use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, params};
use serde_json::Value;
use wayfarer_parse::{GameTexts, ItemTexts, Message, MonsterAttack, MonsterTexts};

use crate::Error;
use crate::schema::SCHEMA_VERSION;
use crate::walk::Key;

/// An open world pack, read only.
pub struct WorldPack {
    conn: Connection,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Room {
    pub map: i64,
    pub room: i64,
    pub name: String,
    pub light: i64,
    pub room_type: i64,
    pub shop_num: i64,
    pub min_level: i64,
    pub max_level: i64,
    pub gen_monster: i64,
    pub max_monsters: i64,
    pub mon_group: i64,
    pub regen_delay: i64,
    pub spec_mon: i64,
    pub flags: i64,
    pub mm_id: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Exit {
    /// 0-9: N S E W NE NW SE SW U D.
    pub dir: u8,
    pub to_map: i64,
    pub to_room: i64,
    /// The exit type by name: "Normal", "Door", "Text", ...
    pub exit_type: String,
    pub type_code: i64,
    /// `exitpar1s`..`exitpar4s`, raw.
    pub params: [i64; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Monster {
    pub number: i64,
    pub name: String,
    pub level: i64,
    pub exp: i64,
    pub max_hp: i64,
    pub mon_type: i64,
    pub gen_index: i64,
    pub alignment: i64,
    /// The export's full record (JSON).
    pub raw: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub number: i64,
    pub name: String,
    pub item_type: i64,
    pub weight: i64,
    pub cost: i64,
    pub currency: i64,
    pub raw: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Spell {
    pub number: i64,
    pub name: String,
    pub short_name: String,
    pub level: i64,
    pub mana: i64,
    pub spell_type: i64,
    pub target: i64,
    pub raw: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shop {
    pub number: i64,
    pub name: String,
    pub shop_type: i64,
    pub min_level: i64,
    pub max_level: i64,
    pub markup: i64,
    pub train_class: i64,
    /// (item number, maximum quantity) per stocked slot, in record order.
    pub stock: Vec<(i64, i64)>,
    pub raw: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Waypoint {
    pub code: String,
    pub mm_id: u32,
    pub flags: u32,
    pub is_shop: bool,
    pub is_bank: bool,
    pub is_trainer: bool,
    pub is_boss: bool,
    pub min_level: i64,
    pub max_level: i64,
    pub class: i64,
    pub group: String,
    pub name: String,
    /// The game rooms whose `mm_id` equals the waypoint's.
    pub rooms: Vec<(i64, i64)>,
}

/// How a path's best walk through the rooms went.
#[derive(Clone, Debug, PartialEq)]
pub struct Verification {
    pub start_candidates: i64,
    pub start: Option<(i64, i64)>,
    pub end: Option<(i64, i64)>,
    pub walked: bool,
    pub closes: bool,
    pub steps_checked: i64,
    pub steps_exact: i64,
    pub steps_title: i64,
    pub steps_none: i64,
    pub steps_wildcard: i64,
    pub broke_at: Option<i64>,
    pub broke_reason: Option<String>,
}

/// One MegaMud `.mp` file.
#[derive(Clone, Debug, PartialEq)]
pub struct MmPath {
    pub file: String,
    pub description: String,
    pub author: String,
    pub from_id: u32,
    pub to_id: u32,
    pub is_loop: bool,
    pub item: String,
    pub prepath: String,
    pub nextpath: String,
    pub verification: Verification,
}

/// One step of a path.
#[derive(Clone, Debug, PartialEq)]
pub struct MmStep {
    pub mm_id: u32,
    pub flags: u16,
    pub command: String,
    pub base: String,
    pub actions: Option<Vec<String>>,
    /// The room the walk typed it in, and how the id matched ("exact", "title", ...).
    pub walk: Option<((i64, i64), String)>,
}

fn room_row(r: &Row) -> rusqlite::Result<Room> {
    Ok(Room {
        map: r.get(0)?,
        room: r.get(1)?,
        name: r.get(2)?,
        light: r.get(3)?,
        room_type: r.get(4)?,
        shop_num: r.get(5)?,
        min_level: r.get(6)?,
        max_level: r.get(7)?,
        gen_monster: r.get(8)?,
        max_monsters: r.get(9)?,
        mon_group: r.get(10)?,
        regen_delay: r.get(11)?,
        spec_mon: r.get(12)?,
        flags: r.get(13)?,
        mm_id: r.get(14)?,
    })
}

const ROOM_COLS: &str = "map, room, name, light, type, shop_num, min_level, max_level, \
                         gen_monster, max_monsters, mon_group, regen_delay, spec_mon, flags, mm_id";

const MONSTER_COLS: &str = "number, name, level, exp, max_hp, mon_type, gen_index, alignment, raw";

fn monster_row(r: &Row) -> rusqlite::Result<Monster> {
    Ok(Monster {
        number: r.get(0)?,
        name: r.get(1)?,
        level: r.get(2)?,
        exp: r.get(3)?,
        max_hp: r.get(4)?,
        mon_type: r.get(5)?,
        gen_index: r.get(6)?,
        alignment: r.get(7)?,
        raw: r.get(8)?,
    })
}

fn path_row(r: &Row) -> rusqlite::Result<MmPath> {
    let pair = |a: Option<i64>, b: Option<i64>| a.zip(b);
    Ok(MmPath {
        file: r.get("file")?,
        description: r.get("description")?,
        author: r.get("author")?,
        from_id: r.get("from_id")?,
        to_id: r.get("to_id")?,
        is_loop: r.get("is_loop")?,
        item: r.get("item")?,
        prepath: r.get("prepath")?,
        nextpath: r.get("nextpath")?,
        verification: Verification {
            start_candidates: r.get("start_candidates")?,
            start: pair(r.get("start_map")?, r.get("start_room")?),
            end: pair(r.get("end_map")?, r.get("end_room")?),
            walked: r.get("walked")?,
            closes: r.get("closes")?,
            steps_checked: r.get("steps_checked")?,
            steps_exact: r.get("steps_exact")?,
            steps_title: r.get("steps_title")?,
            steps_none: r.get("steps_none")?,
            steps_wildcard: r.get("steps_wildcard")?,
            broke_at: r.get("broke_at")?,
            broke_reason: r.get("broke_reason")?,
        },
    })
}

/// A field of a `raw` record.
fn raw_field<'a>(v: &'a Value, table: &str, field: &str) -> Result<&'a Value, Error> {
    v.get(field)
        .ok_or_else(|| Error::Format(format!("{table}: raw record without {field:?}")))
}

fn raw_i32(v: &Value, table: &str, field: &str) -> Result<i32, Error> {
    raw_field(v, table, field)?
        .as_i64()
        .and_then(|n| i32::try_from(n).ok())
        .ok_or_else(|| Error::Format(format!("{table}: {field:?} is not a 32-bit integer")))
}

fn raw_ints(v: &Value, table: &str, field: &str) -> Result<Vec<i64>, Error> {
    raw_field(v, table, field)?
        .as_array()
        .and_then(|a| a.iter().map(Value::as_i64).collect())
        .ok_or_else(|| Error::Format(format!("{table}: {field:?} is not an integer list")))
}

impl WorldPack {
    /// Open a pack read only and check its schema version.
    pub fn open(path: &Path) -> Result<WorldPack, Error> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let pack = WorldPack { conn };
        let version: Option<String> = pack.meta("schema_version")?;
        if version.as_deref() != Some(&SCHEMA_VERSION.to_string()) {
            return Err(Error::Format(format!(
                "{}: schema version {version:?}, this build reads {SCHEMA_VERSION}",
                path.display()
            )));
        }
        Ok(pack)
    }

    /// A `pack_meta` value: `schema_version`, `builder`, `source.<TABLE>.file_sha256`, ...
    pub fn meta(&self, key: &str) -> Result<Option<String>, Error> {
        Ok(self
            .conn
            .query_row("SELECT value FROM pack_meta WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn room(&self, map: i64, room: i64) -> Result<Option<Room>, Error> {
        Ok(self
            .conn
            .prepare_cached(&format!(
                "SELECT {ROOM_COLS} FROM rooms WHERE map = ?1 AND room = ?2"
            ))?
            .query_row(params![map, room], room_row)
            .optional()?)
    }

    /// The export's full record of a room (JSON).
    pub fn room_raw(&self, map: i64, room: i64) -> Result<Option<String>, Error> {
        Ok(self
            .conn
            .prepare_cached("SELECT raw FROM rooms WHERE map = ?1 AND room = ?2")?
            .query_row(params![map, room], |r| r.get(0))
            .optional()?)
    }

    /// A room's exits, in direction order.
    pub fn exits(&self, map: i64, room: i64) -> Result<Vec<Exit>, Error> {
        let mut st = self.conn.prepare_cached(
            "SELECT dir, to_map, to_room, type, type_code, par1, par2, par3, par4 FROM exits \
             WHERE from_map = ?1 AND from_room = ?2 ORDER BY dir",
        )?;
        let rows = st.query_map(params![map, room], |r| {
            Ok(Exit {
                dir: r.get(0)?,
                to_map: r.get(1)?,
                to_room: r.get(2)?,
                exit_type: r.get(3)?,
                type_code: r.get(4)?,
                params: [r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?],
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    fn rooms_where(&self, cond: &str, arg: impl rusqlite::ToSql) -> Result<Vec<Room>, Error> {
        let mut st = self.conn.prepare_cached(&format!(
            "SELECT {ROOM_COLS} FROM rooms WHERE {cond} ORDER BY map, room"
        ))?;
        let rows = st.query_map([arg], room_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Rooms with exactly this name.
    pub fn rooms_by_name(&self, name: &str) -> Result<Vec<Room>, Error> {
        self.rooms_where("name = ?1", name)
    }

    /// Rooms whose MegaMud room id is `mm_id`.
    pub fn rooms_by_mm_id(&self, mm_id: u32) -> Result<Vec<Room>, Error> {
        self.rooms_where("mm_id = ?1", mm_id)
    }

    /// Every room, by key.
    pub fn all_rooms(&self) -> Result<Vec<Room>, Error> {
        let mut st = self
            .conn
            .prepare(&format!("SELECT {ROOM_COLS} FROM rooms ORDER BY map, room"))?;
        let rows = st.query_map([], room_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Every exit with the room it leaves, by room key and direction.
    pub fn all_exits(&self) -> Result<Vec<(Key, Exit)>, Error> {
        let mut st = self.conn.prepare(
            "SELECT from_map, from_room, dir, to_map, to_room, type, type_code, par1, par2, par3, \
             par4 FROM exits ORDER BY from_map, from_room, dir",
        )?;
        let rows = st.query_map([], |r| {
            Ok((
                (r.get(0)?, r.get(1)?),
                Exit {
                    dir: r.get(2)?,
                    to_map: r.get(3)?,
                    to_room: r.get(4)?,
                    exit_type: r.get(5)?,
                    type_code: r.get(6)?,
                    params: [r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?],
                },
            ))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn monster(&self, number: i64) -> Result<Option<Monster>, Error> {
        Ok(self
            .conn
            .prepare_cached(&format!(
                "SELECT {MONSTER_COLS} FROM monsters WHERE number = ?1"
            ))?
            .query_row([number], monster_row)
            .optional()?)
    }

    /// Every monster, by number.
    pub fn all_monsters(&self) -> Result<Vec<Monster>, Error> {
        let mut st = self.conn.prepare(&format!(
            "SELECT {MONSTER_COLS} FROM monsters ORDER BY number"
        ))?;
        let rows = st.query_map([], monster_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn item(&self, number: i64) -> Result<Option<Item>, Error> {
        Ok(self
            .conn
            .prepare_cached(
                "SELECT number, name, type, weight, cost, currency, raw FROM items \
                 WHERE number = ?1",
            )?
            .query_row([number], |r| {
                Ok(Item {
                    number: r.get(0)?,
                    name: r.get(1)?,
                    item_type: r.get(2)?,
                    weight: r.get(3)?,
                    cost: r.get(4)?,
                    currency: r.get(5)?,
                    raw: r.get(6)?,
                })
            })
            .optional()?)
    }

    pub fn spell(&self, number: i64) -> Result<Option<Spell>, Error> {
        Ok(self
            .conn
            .prepare_cached(
                "SELECT number, name, short_name, level, mana, type, target, raw FROM spells \
                 WHERE number = ?1",
            )?
            .query_row([number], |r| {
                Ok(Spell {
                    number: r.get(0)?,
                    name: r.get(1)?,
                    short_name: r.get(2)?,
                    level: r.get(3)?,
                    mana: r.get(4)?,
                    spell_type: r.get(5)?,
                    target: r.get(6)?,
                    raw: r.get(7)?,
                })
            })
            .optional()?)
    }

    pub fn shop(&self, number: i64) -> Result<Option<Shop>, Error> {
        let shop = self
            .conn
            .prepare_cached(
                "SELECT number, name, type, min_level, max_level, markup, train_class, raw \
                 FROM shops WHERE number = ?1",
            )?
            .query_row([number], |r| {
                Ok(Shop {
                    number: r.get(0)?,
                    name: r.get(1)?,
                    shop_type: r.get(2)?,
                    min_level: r.get(3)?,
                    max_level: r.get(4)?,
                    markup: r.get(5)?,
                    train_class: r.get(6)?,
                    stock: Vec::new(),
                    raw: r.get(7)?,
                })
            })
            .optional()?;
        let Some(mut shop) = shop else {
            return Ok(None);
        };
        let mut st = self
            .conn
            .prepare_cached("SELECT item, max_qty FROM shop_stock WHERE shop = ?1 ORDER BY slot")?;
        shop.stock = st
            .query_map([number], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        Ok(Some(shop))
    }

    /// Every shop's type, by number (`kb/economy.md`: 7 bank, 8 training, ...).
    pub fn shop_types(&self) -> Result<Vec<(i64, i64)>, Error> {
        let mut st = self
            .conn
            .prepare_cached("SELECT number, type FROM shops ORDER BY number")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Every class, (number, name), by number.
    pub fn classes(&self) -> Result<Vec<(i64, String)>, Error> {
        self.names("classes")
    }

    /// Every class's (name, magery, magery level), by number.
    pub fn class_magery(&self) -> Result<Vec<(String, i64, i64)>, Error> {
        let mut st = self
            .conn
            .prepare_cached("SELECT name, raw FROM classes ORDER BY number")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (name, raw) = row?;
            let v: Value = serde_json::from_str(&raw)
                .map_err(|e| Error::Format(format!("classes: raw record of {name}: {e}")))?;
            out.push((
                name,
                raw_field(&v, "classes", "magery")?.as_i64().unwrap_or(0),
                raw_field(&v, "classes", "magelvl")?.as_i64().unwrap_or(0),
            ));
        }
        Ok(out)
    }

    /// Every spell, by number.
    pub fn spells(&self) -> Result<Vec<Spell>, Error> {
        let mut st = self.conn.prepare_cached(
            "SELECT number, name, short_name, level, mana, type, target, raw FROM spells \
             ORDER BY number",
        )?;
        let rows = st.query_map([], |r| {
            Ok(Spell {
                number: r.get(0)?,
                name: r.get(1)?,
                short_name: r.get(2)?,
                level: r.get(3)?,
                mana: r.get(4)?,
                spell_type: r.get(5)?,
                target: r.get(6)?,
                raw: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Every race, (number, name), by number.
    pub fn races(&self) -> Result<Vec<(i64, String)>, Error> {
        self.names("races")
    }

    fn names(&self, table: &str) -> Result<Vec<(i64, String)>, Error> {
        let mut st = self
            .conn
            .prepare_cached(&format!("SELECT number, name FROM {table} ORDER BY number"))?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// A message (WCCMSG): its three texts.
    pub fn message(&self, number: i64) -> Result<Option<[String; 3]>, Error> {
        Ok(self
            .conn
            .prepare_cached("SELECT msg1, msg2, msg3 FROM messages WHERE number = ?1")?
            .query_row([number], |r| Ok([r.get(0)?, r.get(1)?, r.get(2)?]))
            .optional()?)
    }

    /// A text block (WCCTEXT): its parts in order, joined; `None` if there is no part 0.
    pub fn text_block(&self, number: i64) -> Result<Option<String>, Error> {
        let mut st = self
            .conn
            .prepare_cached("SELECT part, text FROM texts WHERE number = ?1 ORDER BY part")?;
        let parts: Vec<(i64, String)> = st
            .query_map([number], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        if parts.first().is_none_or(|(p, _)| *p != 0) {
            return Ok(None);
        }
        // Like display_long_text: stop at the first missing part.
        let mut out = String::new();
        for (i, (part, text)) in parts.iter().enumerate() {
            if *part != i as i64 {
                break;
            }
            out.push_str(text);
        }
        Ok(Some(out))
    }

    /// The game texts the parser's generated patterns are built from (`wayfarer_parse::
    /// GameTexts`), equal to `GameTexts::from_export` on the export the pack was built from.
    /// Each table is read in the export's record order (`_id`, in `raw`): the order decides
    /// between equally good patterns, and a table keyed by `number` is stored in number order.
    pub fn game_texts(&self) -> Result<GameTexts, Error> {
        let mut st = self.conn.prepare(
            "SELECT number, msg1, msg2, msg3 FROM messages ORDER BY json_extract(raw, '$._id')",
        )?;
        let messages: Vec<Message> = st
            .query_map([], |r| {
                Ok(Message {
                    number: r.get(0)?,
                    texts: [r.get(1)?, r.get(2)?, r.get(3)?],
                })
            })?
            .collect::<Result<_, _>>()?;

        let raws = |table: &str| -> Result<Vec<Value>, Error> {
            let mut st = self.conn.prepare(&format!("SELECT raw FROM {table}"))?;
            let mut rows = st
                .query_map([], |r| r.get::<_, String>(0))?
                .map(|raw| {
                    let v: Value = serde_json::from_str(&raw?)
                        .map_err(|e| Error::Format(format!("{table}: raw is not JSON: {e}")))?;
                    let id = raw_field(&v, table, "_id")?.as_i64().unwrap_or(i64::MAX);
                    Ok((id, v))
                })
                .collect::<Result<Vec<_>, Error>>()?;
            rows.sort_by_key(|(id, _)| *id);
            Ok(rows.into_iter().map(|(_, v)| v).collect())
        };

        let monsters = raws("monsters")?
            .iter()
            .map(|m| {
                let t = "monsters";
                let (kinds, hit, def, miss) = (
                    raw_ints(m, t, "atktypes")?,
                    raw_ints(m, t, "hitmsgs")?,
                    raw_ints(m, t, "defmsgs")?,
                    raw_ints(m, t, "missmsgs")?,
                );
                let at = |v: &[i64], i: usize| v.get(i).map_or(0, |&n| n as i32);
                Ok(MonsterTexts {
                    name: raw_field(m, t, "name")?
                        .as_str()
                        .ok_or_else(|| Error::Format(format!("{t}: \"name\" is not a string")))?
                        .to_owned(),
                    arrive_msg: raw_i32(m, t, "arrivemsg")?,
                    death_msg: raw_i32(m, t, "deathmsg")?,
                    attacks: (0..kinds.len())
                        .map(|i| MonsterAttack {
                            kind: kinds[i] as u8,
                            hit_msg: at(&hit, i),
                            defend_msg: at(&def, i),
                            miss_msg: at(&miss, i),
                        })
                        .collect(),
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let items = raws("items")?
            .iter()
            .map(|i| {
                Ok(ItemTexts {
                    hit_msg: raw_i32(i, "items", "hitmsg")?,
                    miss_msg: raw_i32(i, "items", "missmsg")?,
                    destroy_msg: raw_i32(i, "items", "destroymsg")?,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let spell_msgs = raws("spells")?
            .iter()
            .map(|s| raw_i32(s, "spells", "msgnum"))
            .collect::<Result<Vec<_>, Error>>()?;
        // rooms keeps its rowid, the insertion (file) order
        let mut st = self.conn.prepare("SELECT name FROM rooms ORDER BY rowid")?;
        let room_names: Vec<String> = st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;

        Ok(GameTexts::from_records(
            messages, monsters, items, spell_msgs, room_names,
        ))
    }

    /// MegaMud's named waypoints (`Rooms.md`), in file order.
    pub fn waypoints(&self) -> Result<Vec<Waypoint>, Error> {
        let mut st = self.conn.prepare_cached(
            "SELECT code, mm_id, flags, is_shop, is_bank, is_trainer, is_boss, min_level, \
             max_level, class, grp, name FROM mm_waypoints ORDER BY line",
        )?;
        let mut out: Vec<Waypoint> = st
            .query_map([], |r| {
                Ok(Waypoint {
                    code: r.get(0)?,
                    mm_id: r.get(1)?,
                    flags: r.get(2)?,
                    is_shop: r.get(3)?,
                    is_bank: r.get(4)?,
                    is_trainer: r.get(5)?,
                    is_boss: r.get(6)?,
                    min_level: r.get(7)?,
                    max_level: r.get(8)?,
                    class: r.get(9)?,
                    group: r.get(10)?,
                    name: r.get(11)?,
                    rooms: Vec::new(),
                })
            })?
            .collect::<Result<_, _>>()?;
        let mut st = self
            .conn
            .prepare_cached("SELECT map, room FROM mm_waypoint_rooms WHERE code = ?1")?;
        for w in &mut out {
            w.rooms = st
                .query_map([&w.code], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<_, _>>()?;
        }
        Ok(out)
    }

    /// Every MegaMud path file, by file name.
    pub fn paths(&self) -> Result<Vec<MmPath>, Error> {
        let mut st = self
            .conn
            .prepare_cached("SELECT * FROM mm_paths ORDER BY file")?;
        let rows = st.query_map([], path_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The loops (FROMID = TOID) with their verification, by file name.
    pub fn loops(&self) -> Result<Vec<MmPath>, Error> {
        let mut st = self
            .conn
            .prepare_cached("SELECT * FROM mm_loops ORDER BY file")?;
        let rows = st.query_map([], path_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The steps of one path file, in order.
    pub fn path_steps(&self, file: &str) -> Result<Vec<MmStep>, Error> {
        let mut st = self.conn.prepare_cached(
            "SELECT mm_id, flags, command, base, actions, walk_map, walk_room, id_match \
             FROM mm_path_steps WHERE file = ?1 ORDER BY step",
        )?;
        let rows = st.query_map([file], |r| {
            let actions: Option<String> = r.get(4)?;
            let at: Option<(i64, i64)> = r.get::<_, Option<i64>>(5)?.zip(r.get(6)?);
            let m: Option<String> = r.get(7)?;
            Ok(MmStep {
                mm_id: r.get(0)?,
                flags: r.get(1)?,
                command: r.get(2)?,
                base: r.get(3)?,
                actions: actions.and_then(|a| serde_json::from_str(&a).ok()),
                walk: at.zip(m),
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}
