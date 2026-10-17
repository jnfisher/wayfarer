//! `import-world`: the stock-world export (and MegaMud's path set) to a world pack.

use std::collections::BTreeSet;
use std::fmt;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use rusqlite::{Connection, Transaction, params};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::mm::{self, MpFile};
use crate::schema::{SCHEMA_VERSION, SQL};
use crate::walk::{self, Graph, Key, Match, Node};
use crate::{Error, cp437};

/// Export tables with stock content, and the pack table each fills.
pub const CONTENT_TABLES: [(&str, &str); 10] = [
    ("WCCMP001", "rooms"),
    ("WCCITEMS", "items"),
    ("WCCKNMSR", "monsters"),
    ("WCCSPELS", "spells"),
    ("WCCCLASS", "classes"),
    ("WCCRACE", "races"),
    ("WCCSHOPS", "shops"),
    ("WCCMSG", "messages"),
    ("WCCTEXT", "texts"),
    ("WCCACTS", "actions"),
];

/// Export tables that hold run-time state, not stock content: noted, not imported.
pub const RUNTIME_TABLES: [&str; 5] = ["WCCACMSR", "WCCITOWN", "WCCUSERS", "WCCGANGS", "WCCBANKS"];

/// What a build produced.
#[derive(Clone, Debug, Default)]
pub struct BuildReport {
    /// (export table, pack table, rows imported, rows in MANIFEST.json).
    pub tables: Vec<(String, String, u64, u64)>,
    /// (export table, rows in MANIFEST.json) for the run-time tables left out.
    pub runtime: Vec<(String, u64)>,
    pub exits: u64,
    pub shop_stock: u64,
    pub megamud: Option<MegaMudReport>,
}

/// MegaMud's path set checked against the rooms.
#[derive(Clone, Debug, Default)]
pub struct MegaMudReport {
    pub zip: String,
    pub waypoints: u64,
    /// Waypoints whose id equals at least one room's `mm_id`.
    pub waypoints_resolved: u64,
    /// ... exactly one room's.
    pub waypoints_unique: u64,
    pub paths: u64,
    pub loops: u64,
    pub walked: u64,
    pub broke: u64,
    pub no_start: u64,
    pub loops_walked: u64,
    pub loops_closed: u64,
    pub steps_exact: u64,
    pub steps_title: u64,
    pub steps_none: u64,
    pub steps_wildcard: u64,
    pub rooms_visited: u64,
}

impl MegaMudReport {
    /// Steps whose title hash matched (exact or title only).
    pub fn steps_title_matched(&self) -> u64 {
        self.steps_exact + self.steps_title
    }
}

fn pct(a: u64, b: u64) -> f64 {
    if b == 0 {
        0.0
    } else {
        100.0 * a as f64 / b as f64
    }
}

impl fmt::Display for BuildReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "tables (rows, MANIFEST.json rows):")?;
        for (src, dst, n, want) in &self.tables {
            let mark = if n == want { "" } else { "  MISMATCH" };
            writeln!(f, "  {dst:<10} {n:>6}  {want:>6}  {src}{mark}")?;
        }
        writeln!(f, "  {:<10} {:>6}          from rooms", "exits", self.exits)?;
        writeln!(
            f,
            "  {:<10} {:>6}          from shops",
            "shop_stock", self.shop_stock
        )?;
        let rt: Vec<String> = self
            .runtime
            .iter()
            .map(|(t, n)| format!("{t} {n}"))
            .collect();
        writeln!(f, "run-time tables, not imported: {}", rt.join(", "))?;
        let Some(m) = &self.megamud else {
            return writeln!(f, "megamud: not imported");
        };
        writeln!(f, "megamud ({}):", m.zip)?;
        writeln!(
            f,
            "  waypoints {}: {} resolve to a game room ({} to exactly one)",
            m.waypoints, m.waypoints_resolved, m.waypoints_unique
        )?;
        writeln!(
            f,
            "  paths {} ({} loops): {} walk to the end, {} break, {} have no start room",
            m.paths, m.loops, m.walked, m.broke, m.no_start
        )?;
        writeln!(
            f,
            "  loops walked to the end {}: {} close in their start room",
            m.loops_walked, m.loops_closed
        )?;
        writeln!(
            f,
            "  steps checked {}: title matches {}, of which full id {} ({:.1}%); \
             no match {}; wildcards {}",
            m.steps_title_matched() + m.steps_none,
            m.steps_title_matched(),
            m.steps_exact,
            pct(m.steps_exact, m.steps_title_matched()),
            m.steps_none,
            m.steps_wildcard
        )?;
        writeln!(f, "  distinct rooms visited {}", m.rooms_visited)
    }
}

fn io_err(path: &Path) -> impl FnOnce(std::io::Error) -> Error + '_ {
    move |source| Error::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn fmt_err(msg: impl Into<String>) -> Error {
    Error::Format(msg.into())
}

/// A JSON record from the export, with the line it came from.
struct Rec {
    v: Value,
    line: String,
    table: &'static str,
}

impl Rec {
    fn get(&self, field: &str) -> Result<&Value, Error> {
        self.v
            .get(field)
            .ok_or_else(|| fmt_err(format!("{}: record without {field:?}", self.table)))
    }

    fn int(&self, field: &str) -> Result<i64, Error> {
        self.get(field)?
            .as_i64()
            .ok_or_else(|| fmt_err(format!("{}: {field:?} is not an integer", self.table)))
    }

    fn str(&self, field: &str) -> Result<&str, Error> {
        self.get(field)?
            .as_str()
            .ok_or_else(|| fmt_err(format!("{}: {field:?} is not a string", self.table)))
    }

    fn ints(&self, field: &str) -> Result<Vec<i64>, Error> {
        self.get(field)?
            .as_array()
            .and_then(|a| a.iter().map(Value::as_i64).collect())
            .ok_or_else(|| fmt_err(format!("{}: {field:?} is not an integer list", self.table)))
    }

    fn strs(&self, field: &str) -> Result<Vec<&str>, Error> {
        self.get(field)?
            .as_array()
            .and_then(|a| a.iter().map(Value::as_str).collect())
            .ok_or_else(|| fmt_err(format!("{}: {field:?} is not a string list", self.table)))
    }
}

/// The records of one export table, in file order.
fn read_table(dir: &Path, table: &'static str) -> Result<Vec<Rec>, Error> {
    let path = dir.join(format!("{table}.jsonl.gz"));
    let file = File::open(&path).map_err(io_err(&path))?;
    let mut out = Vec::new();
    for line in BufReader::new(flate2::read::GzDecoder::new(file)).lines() {
        let line = line.map_err(io_err(&path))?;
        if line.trim().is_empty() {
            continue;
        }
        let v =
            serde_json::from_str(&line).map_err(|e| fmt_err(format!("{}: {e}", path.display())))?;
        out.push(Rec { v, line, table });
    }
    Ok(out)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Standard base64 (the export's byte spans).
fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let s = s.trim_end_matches('=').as_bytes();
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    for chunk in s.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= val(c)? << (18 - 6 * i);
        }
        let bytes = n.to_be_bytes();
        out.extend_from_slice(&bytes[1..chunk.len()]);
    }
    Some(out)
}

/// A WCCTEXT record's text: the stored bytes up to the first stored 0x00 or 0x20 (a decoded
/// NUL), at most 1,999 of them, each minus 0x20, as CP437 (`kb/game-data.md`).
pub fn decode_text(stored: &[u8]) -> String {
    let bytes: Vec<u8> = stored
        .iter()
        .take(1999)
        .take_while(|&&b| b != 0x00 && b != 0x20)
        .map(|&b| b.wrapping_sub(0x20))
        .collect();
    cp437::decode(&bytes)
}

fn insert_rooms(tx: &Transaction, recs: &[Rec], graph: &mut Graph) -> Result<u64, Error> {
    let mut room = tx.prepare(
        "INSERT INTO rooms VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, \
         ?15, ?16)",
    )?;
    let mut exit =
        tx.prepare("INSERT INTO exits VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)")?;
    let mut exits_n = 0;
    for r in recs {
        let (map, num) = (r.int("mapnum")?, r.int("roomnum")?);
        let name = r.str("name")?;
        let dests = r.ints("exits")?;
        let types = r.ints("exittypes")?;
        let pars = [
            r.ints("exitpar1s")?,
            r.ints("exitpar2s")?,
            r.ints("exitpar3s")?,
            r.ints("exitpar4s")?,
        ];
        if dests.len() != 10 || types.len() != 10 || pars.iter().any(|p| p.len() != 10) {
            return Err(fmt_err(format!("room {map}/{num}: exits are not 10 wide")));
        }
        let mut shown = [None; 10];
        let mut links = Vec::new();
        for d in 0..10 {
            if dests[d] == 0 {
                continue;
            }
            shown[d] = Some(types[d]);
            let to_map = if types[d] == 8 && pars[0][d] != 0 {
                pars[0][d]
            } else {
                map
            };
            links.push((d, (to_map, dests[d])));
            exit.execute(params![
                map,
                num,
                d as i64,
                to_map,
                dests[d],
                mm::exit_type_name(types[d]),
                types[d],
                pars[0][d],
                pars[1][d],
                pars[2][d],
                pars[3][d]
            ])?;
            exits_n += 1;
        }
        let mm_id = mm::room_id(name, &shown);
        room.execute(params![
            map,
            num,
            name,
            r.int("light")?,
            r.int("type")?,
            r.int("shopnum")?,
            r.int("minlevel")?,
            r.int("maxlevel")?,
            r.int("genmonster")?,
            r.int("maxmonsters")?,
            r.int("mongroup")?,
            r.int("regendelay")?,
            r.int("specmon")?,
            r.int("flags")?,
            mm_id,
            r.line
        ])?;
        graph.insert((map, num), Node { mm_id, links });
    }
    graph.finish();
    Ok(exits_n)
}

fn insert_simple(tx: &Transaction, table: &str, recs: &[Rec]) -> Result<u64, Error> {
    let mut extra = 0;
    match table {
        "items" => {
            let mut st = tx.prepare("INSERT INTO items VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)")?;
            for r in recs {
                st.execute(params![
                    r.int("number")?,
                    r.str("name")?,
                    r.int("type")?,
                    r.int("weight")?,
                    r.int("cost")?,
                    r.int("currency")?,
                    r.line
                ])?;
            }
        }
        "monsters" => {
            let mut st =
                tx.prepare("INSERT INTO monsters VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)")?;
            for r in recs {
                st.execute(params![
                    r.int("number")?,
                    r.str("name")?,
                    r.int("level")?,
                    r.int("exp")?,
                    r.int("maxhp")?,
                    r.int("montype")?,
                    r.int("genindex")?,
                    r.int("alignment")?,
                    r.line
                ])?;
            }
        }
        "spells" => {
            let mut st =
                tx.prepare("INSERT INTO spells VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)")?;
            for r in recs {
                st.execute(params![
                    r.int("number")?,
                    r.str("name")?,
                    r.str("shortnm")?,
                    r.int("level")?,
                    r.int("mana")?,
                    r.int("type")?,
                    r.int("target")?,
                    r.line
                ])?;
            }
        }
        "classes" | "races" => {
            let mut st = tx.prepare(&format!("INSERT INTO {table} VALUES (?1, ?2, ?3)"))?;
            for r in recs {
                st.execute(params![r.int("number")?, r.str("name")?, r.line])?;
            }
        }
        "shops" => {
            let mut st = tx.prepare("INSERT INTO shops VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)")?;
            let mut stock = tx.prepare("INSERT INTO shop_stock VALUES (?1, ?2, ?3, ?4)")?;
            for r in recs {
                let n = r.int("number")?;
                st.execute(params![
                    n,
                    r.str("name")?,
                    r.int("type")?,
                    r.int("minlevel")?,
                    r.int("maxlevel")?,
                    r.int("markup")?,
                    r.int("trainclass")?,
                    r.line
                ])?;
                let (items, maxq) = (r.ints("items")?, r.ints("maxqty")?);
                for (slot, (item, max)) in items.iter().zip(&maxq).enumerate() {
                    if *item != 0 {
                        stock.execute(params![n, slot as i64, item, max])?;
                        extra += 1;
                    }
                }
            }
        }
        "messages" => {
            let mut st = tx.prepare("INSERT INTO messages VALUES (?1, ?2, ?3, ?4, ?5)")?;
            for r in recs {
                let m = r.strs("msgs")?;
                let m = |i: usize| m.get(i).copied().unwrap_or("");
                st.execute(params![r.int("number")?, m(0), m(1), m(2), r.line])?;
            }
        }
        "texts" => {
            let mut st = tx.prepare("INSERT INTO texts VALUES (?1, ?2, ?3, ?4, ?5)")?;
            for r in recs {
                let stored = base64_decode(r.str("text_raw")?)
                    .ok_or_else(|| fmt_err("WCCTEXT: text_raw is not base64"))?;
                st.execute(params![
                    r.int("number")?,
                    r.int("part")?,
                    r.int("link")?,
                    decode_text(&stored),
                    r.line
                ])?;
            }
        }
        "actions" => {
            let mut st = tx.prepare("INSERT INTO actions VALUES (?1, ?2, ?3)")?;
            for r in recs {
                st.execute(params![r.int("_id")?, r.str("name")?, r.line])?;
            }
        }
        other => return Err(fmt_err(format!("no importer for {other}"))),
    }
    Ok(extra)
}

/// The zip to read MegaMud's files from: `path` itself, or the one `.zip` in a directory.
pub fn find_megamud_zip(path: &Path) -> Result<PathBuf, Error> {
    if !path.is_dir() {
        return Ok(path.to_path_buf());
    }
    let mut zips: Vec<PathBuf> = fs::read_dir(path)
        .map_err(io_err(path))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("zip")))
        .collect();
    match zips.len() {
        1 => Ok(zips.remove(0)),
        n => Err(fmt_err(format!(
            "{}: expected one .zip, found {n}",
            path.display()
        ))),
    }
}

/// The files the pack takes from MegaMud's zip.
struct MegaMudFiles {
    rooms_md: Vec<u8>,
    /// `Default/*.mp` as (file name, bytes), sorted by name.
    mps: Vec<(String, Vec<u8>)>,
}

fn read_megamud(zip_path: &Path) -> Result<MegaMudFiles, Error> {
    let file = File::open(zip_path).map_err(io_err(zip_path))?;
    let mut zip = zip::ZipArchive::new(file)?;
    let mut names: Vec<String> = zip
        .file_names()
        .map(|n| n.map(|n| n.into_owned()))
        .collect::<Result<_, _>>()?;
    names.sort();
    let mut rooms = None;
    let mut mps = Vec::new();
    for name in names {
        let Some(base) = name.strip_prefix("Default/") else {
            continue;
        };
        let is_mp = base.to_ascii_lowercase().ends_with(".mp") && !base.contains('/');
        if !(is_mp || base.eq_ignore_ascii_case("Rooms.md")) {
            continue;
        }
        let mut bytes = Vec::new();
        zip.by_name(&name)?
            .read_to_end(&mut bytes)
            .map_err(io_err(zip_path))?;
        if is_mp {
            mps.push((base.to_string(), bytes));
        } else {
            rooms = Some(bytes);
        }
    }
    let rooms_md = rooms.ok_or_else(|| fmt_err("MegaMud zip: no Default/Rooms.md"))?;
    Ok(MegaMudFiles { rooms_md, mps })
}

fn insert_megamud(
    tx: &Transaction,
    graph: &Graph,
    zip_path: &Path,
) -> Result<MegaMudReport, Error> {
    let MegaMudFiles {
        rooms_md,
        mps: mp_files,
    } = read_megamud(zip_path)?;
    let mut rep = MegaMudReport {
        zip: zip_path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
        ..MegaMudReport::default()
    };

    let (waypoints, bad) = mm::parse_rooms_md(&rooms_md);
    if !bad.is_empty() {
        return Err(fmt_err(format!(
            "Rooms.md: {} unreadable lines, first {:?}",
            bad.len(),
            bad[0]
        )));
    }
    let mut wp = tx.prepare(
        "INSERT INTO mm_waypoints VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
    )?;
    let mut wpr = tx.prepare("INSERT INTO mm_waypoint_rooms VALUES (?1, ?2, ?3)")?;
    for (line, w) in waypoints.iter().enumerate() {
        wp.execute(params![
            w.code,
            line as i64 + 1,
            w.id,
            w.flags,
            w.is_shop(),
            w.is_bank(),
            w.is_trainer(),
            w.is_boss(),
            w.min_level,
            w.max_level,
            w.class,
            w.group,
            w.name
        ])?;
        let rooms = graph.with_id(w.id);
        for (m, r) in rooms {
            wpr.execute(params![w.code, m, r])?;
        }
        rep.waypoints += 1;
        rep.waypoints_resolved += u64::from(!rooms.is_empty());
        rep.waypoints_unique += u64::from(rooms.len() == 1);
    }

    let mut path = tx.prepare(
        "INSERT INTO mm_paths VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, \
         ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28)",
    )?;
    let mut step =
        tx.prepare("INSERT INTO mm_path_steps VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)")?;
    let mut visited: BTreeSet<Key> = BTreeSet::new();
    for (file, bytes) in &mp_files {
        let mp: MpFile = mm::parse_mp(file, bytes).map_err(|e| fmt_err(e.0))?;
        let (best, candidates) = walk::best_walk(graph, &mp);
        rep.paths += 1;
        rep.loops += u64::from(mp.is_loop());
        let count = |m: Match| best.as_ref().map_or(0, |w| w.count(m) as u64);
        let walked = best.as_ref().is_some_and(|w| w.broke.is_none());
        let closes = walked && mp.is_loop() && best.as_ref().is_some_and(|w| w.end == w.start);
        match &best {
            None => rep.no_start += 1,
            Some(_) if walked => rep.walked += 1,
            Some(_) => rep.broke += 1,
        }
        if walked && mp.is_loop() {
            rep.loops_walked += 1;
            rep.loops_closed += u64::from(closes);
        }
        rep.steps_exact += count(Match::Exact);
        rep.steps_title += count(Match::Title);
        rep.steps_none += count(Match::None);
        rep.steps_wildcard += count(Match::Wildcard);
        if let Some(w) = &best {
            visited.extend(w.steps.iter().map(|(k, _)| *k));
            visited.insert(w.end);
        }
        let broke = best.as_ref().and_then(|w| w.broke.clone());
        path.execute(params![
            mp.file,
            mp.description,
            mp.author,
            mp.start_location,
            mp.end_location,
            mp.header.join(":"),
            mp.from_id,
            mp.to_id,
            mp.is_loop(),
            mp.step_count,
            mp.cost,
            mp.item,
            mp.prepath,
            mp.nextpath,
            candidates as i64,
            best.as_ref().map(|w| w.start.0),
            best.as_ref().map(|w| w.start.1),
            best.as_ref().map(|w| w.end.0),
            best.as_ref().map(|w| w.end.1),
            walked,
            closes,
            best.as_ref().map_or(0, |w| w.steps.len() as i64),
            count(Match::Exact) as i64,
            count(Match::Title) as i64,
            count(Match::None) as i64,
            count(Match::Wildcard) as i64,
            broke.as_ref().map(|(i, _)| *i as i64),
            broke.map(|(_, why)| why),
        ])?;
        for (i, s) in mp.steps.iter().enumerate() {
            let at = best.as_ref().and_then(|w| w.steps.get(i));
            let actions = s
                .actions
                .as_ref()
                .map(|a| Value::from(a.clone()).to_string());
            step.execute(params![
                mp.file,
                i as i64,
                s.id,
                s.flags,
                s.command,
                s.base,
                actions,
                at.map(|(k, _)| k.0),
                at.map(|(k, _)| k.1),
                at.map(|(_, m)| m.as_str()),
            ])?;
        }
    }
    rep.rooms_visited = visited.len() as u64;
    Ok(rep)
}

/// Build a pack at `out` from the export in `export` and, if given, the MegaMud zip (or the
/// directory holding it). The pack is written beside `out` and renamed over it at the end.
pub fn build(export: &Path, megamud: Option<&Path>, out: &Path) -> Result<BuildReport, Error> {
    let manifest_path = export.join("MANIFEST.json");
    let manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).map_err(io_err(&manifest_path))?)
            .map_err(|e| fmt_err(format!("{}: {e}", manifest_path.display())))?;
    let table_meta = |t: &str| -> Result<(u64, String), Error> {
        let m = &manifest["tables"][t];
        let rows = m["rows"].as_u64();
        let sha = m["file_sha256"].as_str();
        match (rows, sha) {
            (Some(r), Some(s)) => Ok((r, s.to_string())),
            _ => Err(fmt_err(format!(
                "MANIFEST.json: no rows/file_sha256 for {t}"
            ))),
        }
    };
    let zip_path = megamud.map(find_megamud_zip).transpose()?;

    let partial = PathBuf::from(format!("{}.partial", out.display()));
    if partial.exists() {
        fs::remove_file(&partial).map_err(io_err(&partial))?;
    }
    let mut conn = Connection::open(&partial)?;
    conn.execute_batch("PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF;")?;
    conn.execute_batch(SQL)?;
    let tx = conn.transaction()?;
    let mut report = BuildReport::default();
    let mut meta: Vec<(String, String)> = vec![
        ("schema_version".into(), SCHEMA_VERSION.to_string()),
        (
            "builder".into(),
            format!("wayfarer-world {}", env!("CARGO_PKG_VERSION")),
        ),
    ];
    for key in [
        "date",
        "post_update",
        "module_dll_sha256",
        "dist_zip_sha256",
    ] {
        if let Some(v) = manifest.get(key) {
            let v = v.as_str().map_or_else(|| v.to_string(), str::to_string);
            meta.push((format!("export.{key}"), v));
        }
    }

    let mut graph = Graph::default();
    for (src, dst) in CONTENT_TABLES {
        let (want, sha) = table_meta(src)?;
        let file = export.join(format!("{src}.jsonl.gz"));
        let got_sha = sha256_hex(&fs::read(&file).map_err(io_err(&file))?);
        if got_sha != sha {
            return Err(fmt_err(format!(
                "{}: SHA-256 differs from MANIFEST.json",
                file.display()
            )));
        }
        meta.push((format!("source.{src}.file_sha256"), sha));
        let recs = read_table(export, src)?;
        if dst == "rooms" {
            report.exits = insert_rooms(&tx, &recs, &mut graph)?;
        } else {
            report.shop_stock += insert_simple(&tx, dst, &recs)?;
        }
        report
            .tables
            .push((src.into(), dst.into(), recs.len() as u64, want));
    }
    for t in RUNTIME_TABLES {
        report.runtime.push((t.into(), table_meta(t)?.0));
    }
    if let Some(zip) = &zip_path {
        let bytes = fs::read(zip).map_err(io_err(zip))?;
        meta.push(("megamud.zip_sha256".into(), sha256_hex(&bytes)));
        let rep = insert_megamud(&tx, &graph, zip)?;
        meta.push(("megamud.zip".into(), rep.zip.clone()));
        report.megamud = Some(rep);
    }
    {
        let mut st = tx.prepare("INSERT INTO pack_meta VALUES (?1, ?2)")?;
        for (k, v) in &meta {
            st.execute(params![k, v])?;
        }
    }
    tx.commit()?;
    conn.close().map_err(|(_, e)| e)?;
    fs::rename(&partial, out).map_err(io_err(out))?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips_known_strings() {
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(base64_decode("aGk=").unwrap(), b"hi");
        assert_eq!(base64_decode("YWJj").unwrap(), b"abc");
        assert!(base64_decode("a*b=").is_none());
    }

    #[test]
    fn text_ends_at_a_stored_nul_or_space() {
        // "Hi\r" stored +0x20, then a stored NUL.
        assert_eq!(decode_text(&[0x68, 0x89, 0x2D, 0x00, 0x41]), "Hi\r");
        // A stored 0x20 (a decoded NUL) ends it too.
        assert_eq!(decode_text(&[0x68, 0x20, 0x41]), "H");
        // CP437 above 0x80: 0xC4 '─' stored as 0xE4.
        assert_eq!(decode_text(&[0xE4]), "─");
        assert_eq!(decode_text(&[0x41; 2500]).len(), 1999);
    }
}
