//! The game texts the generated patterns are built from.
//!
//! A customised world has its own messages, monsters and rooms, so these come from the world's
//! data rather than being compiled in: `GameTexts::from_export` reads the stock-world export
//! (`tests/fixtures/world/`), and the world pack (card C-011) will fill the same struct.

use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use flate2::read::GzDecoder;
use serde::Deserialize;
use serde::de::DeserializeOwned;

/// A WCCMSG record: three texts (to you, to the target, to the room, depending on the role).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Message {
    pub number: i32,
    #[serde(rename = "msgs")]
    pub texts: [String; 3],
}

/// One attack slot of a known monster (struct monster, MAJORMUD.H:1530-1546).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonsterAttack {
    /// `atktypes`: 0 means the slot is unused.
    pub kind: u8,
    pub hit_msg: i32,
    pub defend_msg: i32,
    pub miss_msg: i32,
}

/// The texts a known monster (WCCKNMSR) refers to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonsterTexts {
    pub name: String,
    pub arrive_msg: i32,
    pub death_msg: i32,
    pub attacks: Vec<MonsterAttack>,
}

/// The texts an item (WCCITEMS) refers to: weapon hit and miss verbs, the destroyed text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemTexts {
    pub hit_msg: i32,
    pub miss_msg: i32,
    pub destroy_msg: i32,
}

/// Everything the generated patterns are built from, in record order (order decides between
/// equally good patterns, as in the prototype).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameTexts {
    /// WCCMSG, every record.
    pub messages: Vec<Message>,
    /// WCCKNMSR: names and message numbers.
    pub monsters: Vec<MonsterTexts>,
    /// WCCITEMS: message numbers.
    pub items: Vec<ItemTexts>,
    /// WCCSPELS: each spell's cast-text message number.
    pub spell_msgs: Vec<i32>,
    /// WCCMP001 room names: a line equal to one is a room title.
    pub room_names: Vec<String>,
}

#[derive(Deserialize)]
struct MonsterRow {
    name: String,
    arrivemsg: i32,
    deathmsg: i32,
    atktypes: Vec<u8>,
    hitmsgs: Vec<i32>,
    defmsgs: Vec<i32>,
    missmsgs: Vec<i32>,
}

#[derive(Deserialize)]
struct ItemRow {
    hitmsg: i32,
    missmsg: i32,
    destroymsg: i32,
}

#[derive(Deserialize)]
struct SpellRow {
    msgnum: i32,
}

#[derive(Deserialize)]
struct RoomRow {
    name: String,
}

fn read_table<T: DeserializeOwned>(dir: &Path, table: &str) -> io::Result<Vec<T>> {
    let (path, rd) = open_table(dir, table)?;
    let mut out = Vec::new();
    for (i, line) in rd.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        out.push(serde_json::from_str(&line).map_err(|e| bad_row(&path, i + 1, e))?);
    }
    Ok(out)
}

fn open_table(dir: &Path, table: &str) -> io::Result<(std::path::PathBuf, impl BufRead)> {
    let path = dir.join(format!("{table}.jsonl.gz"));
    let file = File::open(&path)
        .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))?;
    Ok((
        path,
        BufReader::with_capacity(
            1 << 16,
            GzDecoder::new(BufReader::with_capacity(1 << 16, file)),
        ),
    ))
}

fn bad_row(path: &Path, line: usize, e: impl std::fmt::Display) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{}:{line}: {e}", path.display()),
    )
}

/// Room names only. A room row is 1.5 kB of JSON and only its name is needed, so read the
/// `name` string where the exporter writes it (after the four key fields) and parse the whole
/// row only if it is not there.
fn read_room_names(dir: &Path) -> io::Result<Vec<String>> {
    const KEY: &str = r#","name":"#;
    let (path, mut rd) = open_table(dir, "WCCMP001")?;
    let mut out = Vec::new();
    let mut line = String::new();
    let mut n = 0;
    while {
        line.clear();
        rd.read_line(&mut line)? > 0
    } {
        n += 1;
        if line.trim().is_empty() {
            continue;
        }
        let fast = line.find(KEY).filter(|&i| i < 80).and_then(|i| {
            let mut de = serde_json::Deserializer::from_str(&line[i + KEY.len()..]);
            String::deserialize(&mut de).ok()
        });
        match fast {
            Some(name) => out.push(name),
            None => out.push(
                serde_json::from_str::<RoomRow>(&line)
                    .map_err(|e| bad_row(&path, n, e))?
                    .name,
            ),
        }
    }
    Ok(out)
}

impl GameTexts {
    /// Read the world export written by `mmud_data.py export` (one `<TABLE>.jsonl.gz` per
    /// table): WCCMSG, WCCKNMSR, WCCITEMS, WCCSPELS and WCCMP001.
    pub fn from_export(dir: impl AsRef<Path>) -> io::Result<GameTexts> {
        let dir = dir.as_ref();
        let messages = read_table::<Message>(dir, "WCCMSG")?;
        let monsters = read_table::<MonsterRow>(dir, "WCCKNMSR")?
            .into_iter()
            .map(|m| MonsterTexts {
                attacks: (0..m.atktypes.len())
                    .map(|i| MonsterAttack {
                        kind: m.atktypes[i],
                        hit_msg: m.hitmsgs.get(i).copied().unwrap_or(0),
                        defend_msg: m.defmsgs.get(i).copied().unwrap_or(0),
                        miss_msg: m.missmsgs.get(i).copied().unwrap_or(0),
                    })
                    .collect(),
                name: m.name,
                arrive_msg: m.arrivemsg,
                death_msg: m.deathmsg,
            })
            .collect();
        let items = read_table::<ItemRow>(dir, "WCCITEMS")?
            .into_iter()
            .map(|i| ItemTexts {
                hit_msg: i.hitmsg,
                miss_msg: i.missmsg,
                destroy_msg: i.destroymsg,
            })
            .collect();
        let spell_msgs = read_table::<SpellRow>(dir, "WCCSPELS")?
            .into_iter()
            .map(|s| s.msgnum)
            .collect();
        let room_names = read_room_names(dir)?;
        Ok(GameTexts {
            messages,
            monsters,
            items,
            spell_msgs,
            room_names,
        })
    }
}
