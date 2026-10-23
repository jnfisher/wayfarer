//! MegaMud's formats (`kb/megamud-formats.md`): the room id, `.mp` path files and `Rooms.md`.

use crate::cp437;

/// The game's ten exit directions in record order, as MegaMud types them.
pub const DIRS: [&str; 10] = ["n", "s", "e", "w", "ne", "nw", "se", "sw", "u", "d"];

/// The game's 25 exit types, by `exittypes` code (`mmud_data.py` `EXITTYPES`).
pub const EXIT_TYPES: [&str; 25] = [
    "Normal",
    "Spell",
    "Key",
    "Item",
    "Toll",
    "Action",
    "Hidden",
    "Door",
    "Map Change",
    "Trap",
    "Text",
    "Gate",
    "Remote Action",
    "Class",
    "Race",
    "Level",
    "Timed",
    "Ticket",
    "User Count",
    "Block Guard",
    "Alignment",
    "Delay",
    "Cast",
    "Ability",
    "Spell Trap",
];

/// The name of an exit type code, or `?N` for a code outside the table.
pub fn exit_type_name(code: i64) -> String {
    usize::try_from(code)
        .ok()
        .and_then(|i| EXIT_TYPES.get(i))
        .map_or_else(|| format!("?{code}"), |s| (*s).to_string())
}

/// A step id that matches any room (`FFFFFFFF`).
pub const WILDCARD: u32 = 0xFFFF_FFFF;

/// The 12-bit title hash: `(Σ (i+1)·byte[i]) & 0xFFF` over the title's CP437 bytes.
pub fn title_hash(title: &str) -> u32 {
    cp437::encode(title)
        .iter()
        .enumerate()
        .fold(0u32, |sum, (i, &b)| {
            sum.wrapping_add((i as u32 + 1) * u32::from(b))
        })
        & 0xFFF
}

/// How MegaMud shows an exit type in the room id: 01 plain; 10 Door, Gate and Key; 00 (not
/// shown) Hidden, Text and Remote Action; every other type 01.
pub fn exit_code(exit_type: i64) -> u32 {
    match exit_type {
        2 | 7 | 11 => 0b10,
        6 | 10 | 12 => 0b00,
        _ => 0b01,
    }
}

/// The MegaMud room id of a room: title hash in bits 31..20, two bits per exit (direction `d`
/// at bits `2d..2d+1`). `exits[d]` is the exit type of direction `d`, `None` for no exit.
pub fn room_id(title: &str, exits: &[Option<i64>; 10]) -> u32 {
    let bits = exits.iter().enumerate().fold(0u32, |acc, (d, t)| match t {
        Some(t) => acc | exit_code(*t) << (2 * d),
        None => acc,
    });
    title_hash(title) << 20 | bits
}

/// The direction index a command moves in (`n`, `north`, `up`, ...), if it is one.
pub fn direction(command: &str) -> Option<usize> {
    const LONG: [&str; 10] = [
        "north",
        "south",
        "east",
        "west",
        "northeast",
        "northwest",
        "southeast",
        "southwest",
        "up",
        "down",
    ];
    let c = command.trim().to_ascii_lowercase();
    DIRS.iter()
        .position(|d| *d == c)
        .or_else(|| LONG.iter().position(|d| *d == c))
}

/// One `.mp` file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MpFile {
    pub file: String,
    pub description: String,
    pub author: String,
    /// `CODE:Region:Name` of the start, as written (cosmetic; FROMID is what counts).
    pub start_location: Option<String>,
    pub end_location: Option<String>,
    /// The header line, all eight fields as written.
    pub header: Vec<String>,
    pub from_id: u32,
    pub to_id: u32,
    pub step_count: Option<i64>,
    pub field3: Option<i64>,
    pub cost: Option<i64>,
    pub item: String,
    pub prepath: String,
    pub nextpath: String,
    pub steps: Vec<MpStep>,
    /// Lines after the header that aren't steps (none in the stock set).
    pub bad_lines: Vec<String>,
}

impl MpFile {
    /// FROMID equals TOID.
    pub fn is_loop(&self) -> bool {
        self.from_id == self.to_id
    }
}

/// One step line, `ROOMID:FLAGS:COMMAND`.
#[derive(Clone, Debug, PartialEq)]
pub struct MpStep {
    /// The room the command is typed in; [`WILDCARD`] for any room.
    pub id: u32,
    pub flags: u16,
    /// The command as written, `dir[actions]` included.
    pub command: String,
    /// The command without its `[actions]` suffix.
    pub base: String,
    /// The `[a,b,...]` actions: `Some(vec![])` for an empty `[]`, `None` when there is none.
    pub actions: Option<Vec<String>>,
}

/// Why a `.mp` file couldn't be read.
#[derive(Debug, thiserror::Error, PartialEq)]
#[error("{0}")]
pub struct MpError(pub String);

fn hex32(s: &str) -> Option<u32> {
    (s.len() == 8)
        .then(|| u32::from_str_radix(s, 16).ok())
        .flatten()
}

fn bracket_fields(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find(']') else {
            break;
        };
        out.push(without_room_suffix(&rest[open + 1..open + close]).to_string());
        rest = &rest[open + close + 1..];
    }
    out
}

/// A name without the `-MAP ROOM` suffix MegaMud 2.2 adds to path descriptions, locations and
/// waypoint names (`Black Star Key Gather Loop-1 1224`); the ids say where, the name is for show.
fn without_room_suffix(name: &str) -> &str {
    let Some((head, tail)) = name.rsplit_once('-') else {
        return name;
    };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    match tail.split_once(' ') {
        Some((map, room)) if digits(map) && digits(room) => head,
        _ => name,
    }
}

/// Split `dir[a,b]` into (`dir`, `Some(["a", "b"])`).
pub fn split_command(command: &str) -> (String, Option<Vec<String>>) {
    let c = command.trim_end();
    if let (Some(open), true) = (c.find('['), c.ends_with(']')) {
        let inner = &c[open + 1..c.len() - 1];
        let actions = if inner.trim().is_empty() {
            Vec::new()
        } else {
            inner.split(',').map(|a| a.trim().to_string()).collect()
        };
        return (c[..open].trim().to_string(), Some(actions));
    }
    (c.trim().to_string(), None)
}

/// Parse a `.mp` file's bytes (Latin-1 text, CRLF or LF, optional `\x1a` end-of-file byte).
pub fn parse_mp(file: &str, bytes: &[u8]) -> Result<MpFile, MpError> {
    let end = bytes.iter().position(|&b| b == 0x1a).unwrap_or(bytes.len());
    let text: String = bytes[..end].iter().map(|&b| b as char).collect();
    let mut lines = text
        .split('\n')
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.trim().is_empty())
        .peekable();

    let mut mp = MpFile {
        file: file.to_string(),
        ..MpFile::default()
    };
    let mut brackets = Vec::new();
    while let Some(l) = lines.next_if(|l| l.starts_with('[')) {
        brackets.push(l);
    }
    let mut locations: Vec<String> = Vec::new();
    match brackets.first() {
        // A lone location bracket with no description line.
        Some(first) if brackets.len() == 1 && bracket_fields(first).len() == 1 => {
            let f = bracket_fields(first).remove(0);
            if f.contains(':') {
                locations.push(f);
            } else {
                mp.description = f;
            }
        }
        Some(first) => {
            let f = bracket_fields(first);
            mp.description = f.first().cloned().unwrap_or_default();
            mp.author = f.get(1).cloned().unwrap_or_default();
            locations.extend(
                brackets[1..]
                    .iter()
                    .filter_map(|b| bracket_fields(b).into_iter().next()),
            );
        }
        None => {}
    }
    let mut locations = locations.into_iter();
    mp.start_location = locations.next();
    mp.end_location = locations.next();

    let header = lines
        .next()
        .ok_or_else(|| MpError(format!("{file}: no header line")))?;
    mp.header = header.split(':').map(str::to_string).collect();
    let field = |i: usize| mp.header.get(i).map_or("", |s| s.trim());
    mp.from_id = hex32(field(0)).ok_or_else(|| MpError(format!("{file}: bad FROMID")))?;
    mp.to_id = hex32(field(1)).ok_or_else(|| MpError(format!("{file}: bad TOID")))?;
    mp.step_count = field(2).parse().ok();
    mp.field3 = field(3).parse().ok();
    mp.cost = field(4).parse().ok();
    mp.item = field(5).to_string();
    mp.prepath = field(6).to_string();
    mp.nextpath = field(7).to_string();

    for l in lines {
        let mut parts = l.splitn(3, ':');
        let (id, flags, cmd) = (parts.next(), parts.next(), parts.next());
        let parsed = match (id.and_then(hex32), flags, cmd) {
            (Some(id), Some(f), Some(cmd)) if f.len() == 4 => u16::from_str_radix(f, 16)
                .ok()
                .map(|flags| (id, flags, cmd)),
            _ => None,
        };
        match parsed {
            Some((id, flags, cmd)) => {
                let (base, actions) = split_command(cmd);
                mp.steps.push(MpStep {
                    id,
                    flags,
                    command: cmd.to_string(),
                    base,
                    actions,
                });
            }
            None => mp.bad_lines.push(l.to_string()),
        }
    }
    Ok(mp)
}

/// One `Rooms.md` line: `ROOMID:FLAGS:MINLV:MAXLV:CLASS:CODE:Group:Display name`.
#[derive(Clone, Debug, PartialEq)]
pub struct Waypoint {
    pub id: u32,
    pub flags: u32,
    pub min_level: i64,
    pub max_level: i64,
    pub class: i64,
    pub code: String,
    pub group: String,
    pub name: String,
}

impl Waypoint {
    pub fn is_shop(&self) -> bool {
        self.flags & 0x02 != 0
    }
    pub fn is_bank(&self) -> bool {
        self.flags & 0x04 != 0
    }
    pub fn is_trainer(&self) -> bool {
        self.flags & 0x08 != 0
    }
    pub fn is_boss(&self) -> bool {
        self.flags & 0x10 != 0
    }
}

/// Parse `Rooms.md` (Latin-1 text); lines that don't fit are returned in the second list.
pub fn parse_rooms_md(bytes: &[u8]) -> (Vec<Waypoint>, Vec<String>) {
    let text: String = bytes.iter().map(|&b| b as char).collect();
    let mut out = Vec::new();
    let mut bad = Vec::new();
    for l in text.split('\n').map(|l| l.trim_end_matches(['\r', '\x1a'])) {
        if l.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = l.splitn(8, ':').collect();
        let wp = (f.len() == 8)
            .then(|| {
                Some(Waypoint {
                    id: hex32(f[0])?,
                    flags: u32::from_str_radix(f[1], 16).ok()?,
                    min_level: f[2].parse().ok()?,
                    max_level: f[3].parse().ok()?,
                    class: f[4].parse().ok()?,
                    code: f[5].to_string(),
                    group: f[6].to_string(),
                    name: without_room_suffix(f[7]).to_string(),
                })
            })
            .flatten();
        match wp {
            Some(wp) => out.push(wp),
            None => bad.push(l.to_string()),
        }
    }
    (out, bad)
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: Option<i64> = None;

    #[test]
    fn title_hash_is_the_weighted_byte_sum() {
        // "ab": 1*97 + 2*98 = 293
        assert_eq!(title_hash("ab"), 293);
        assert_eq!(title_hash(""), 0);
    }

    // Ids computed by `routecheck.py --id` / `room_id()` from the same export, for rooms whose
    // exits are all of types where routecheck and the KB table agree.
    #[test]
    fn room_ids_match_routecheck() {
        // 1/1 Town Gates: N, S Block Guard (01), E Gate (10), W Normal (01).
        assert_eq!(
            room_id(
                "Town Gates",
                &[Some(19), Some(19), Some(11), Some(0), N, N, N, N, N, N]
            ),
            0x4EB0_0065
        );
        // 3/606: W Key (10), NE Normal (01). Also Rooms.md's AALY.
        assert_eq!(
            room_id(
                "Dark Alleyway, Dead End",
                &[N, N, N, Some(2), Some(0), N, N, N, N, N]
            ),
            0xCAB0_0180
        );
        // 1/2: N, S Normal.
        assert_eq!(
            room_id(
                "Lucky Strike Casino",
                &[Some(0), Some(0), N, N, N, N, N, N, N, N]
            ),
            0x7990_0005
        );
        // 1/2307, routecheck.py's own example id: E Normal.
        assert_eq!(
            room_id("Spell Shop", &[N, N, Some(0), N, N, N, N, N, N, N]),
            0x4BA0_0010
        );
    }

    #[test]
    fn exit_codes_follow_the_kb_table() {
        let mut exits = [None; 10];
        for (t, want) in [
            (0, 1),
            (7, 2),
            (11, 2),
            (2, 2),
            (6, 0),
            (10, 0),
            (12, 0),
            (5, 1),
        ] {
            exits[0] = Some(t);
            assert_eq!(room_id("", &exits), want, "type {t}");
        }
        // Direction d sits at bits 2d..2d+1: Down is bits 18..19.
        let mut exits = [None; 10];
        exits[9] = Some(7);
        assert_eq!(room_id("", &exits), 0b10 << 18);
    }

    #[test]
    fn mp_loop_with_header_and_steps() {
        let src =
            b"[Black Star Key Gather Loop][Winterhawk]\r\n[SLMB:Slums:Outside Black House]\r\n\
                    A0700025:A0700025:2:-1:0:::\r\nA0700025:0000:s\r\nA0700045:0001:n\r\n";
        let mp = parse_mp("SLMBLOOP.mp", src).unwrap();
        assert_eq!(mp.description, "Black Star Key Gather Loop");
        assert_eq!(mp.author, "Winterhawk");
        assert_eq!(
            mp.start_location.as_deref(),
            Some("SLMB:Slums:Outside Black House")
        );
        assert_eq!(mp.end_location, None);
        assert!(mp.is_loop());
        assert_eq!(mp.header.len(), 8);
        assert_eq!(
            (mp.step_count, mp.field3, mp.cost),
            (Some(2), Some(-1), Some(0))
        );
        assert_eq!(mp.steps.len(), 2);
        assert_eq!(mp.steps[1].id, 0xA070_0045);
        assert_eq!(mp.steps[1].flags, 1);
        assert_eq!(mp.steps[1].base, "n");
        assert!(mp.bad_lines.is_empty());
    }

    #[test]
    fn v2_2_room_suffixes_are_dropped_from_names() {
        let src = b"[Black Star Key Gather Loop-1 1224][Winterhawk]\r\n\
                    [SLMB:Slums:Outside Black House-1 1224]\r\n\
                    A0700025:A0700025:1:-1:0:::\r\nA0700025:0000:s\r\n";
        let mp = parse_mp("SLMBLOOP.mp", src).unwrap();
        assert_eq!(mp.description, "Black Star Key Gather Loop");
        assert_eq!(
            mp.start_location.as_deref(),
            Some("SLMB:Slums:Outside Black House")
        );
        let (wps, bad) = parse_rooms_md(
            b"CAB00180:00004040:0:0:0:AALY:Ancient Ruin:Ancient Ruin Dark Alley-3 606\r\n",
        );
        assert!(bad.is_empty());
        assert_eq!(wps[0].name, "Ancient Ruin Dark Alley");
        // Only a trailing `-<map> <room>` goes.
        for keep in ["Orc Camp-Upper", "Loop-1", "Arena -1 2b", "Gnoll Den- 4 5"] {
            assert_eq!(without_room_suffix(keep), keep);
        }
    }

    #[test]
    fn mp_point_to_point_with_item_prepath_and_nextpath() {
        let src = b"[][]\n[AAAA:Town:Start]\n[BBBB:Town:End]\n\
                    00100001:00200001:1:-1:997:rusty iron key:ARCHSLEZ.mp:FNG2LOOP\n\
                    00100001:0000:e\n\x1a";
        let mp = parse_mp("AAAABBBB.mp", src).unwrap();
        assert_eq!((mp.description.as_str(), mp.author.as_str()), ("", ""));
        assert_eq!(mp.start_location.as_deref(), Some("AAAA:Town:Start"));
        assert_eq!(mp.end_location.as_deref(), Some("BBBB:Town:End"));
        assert!(!mp.is_loop());
        assert_eq!(mp.cost, Some(997));
        assert_eq!(mp.item, "rusty iron key");
        assert_eq!(mp.prepath, "ARCHSLEZ.mp");
        assert_eq!(mp.nextpath, "FNG2LOOP");
        assert_eq!(mp.steps.len(), 1);
    }

    #[test]
    fn mp_step_actions() {
        let (b, a) = split_command("w[sea w]");
        assert_eq!((b.as_str(), a), ("w", Some(vec!["sea w".to_string()])));
        let (b, a) = split_command("s[n,pull lever,s]");
        assert_eq!(b, "s");
        assert_eq!(a.unwrap(), ["n", "pull lever", "s"]);
        // Empty [] is an empty action list, not "no actions".
        let (b, a) = split_command("e[]");
        assert_eq!((b.as_str(), a), ("e", Some(vec![])));
        let (b, a) = split_command("give spider silk to dhelvanen");
        assert_eq!((b.as_str(), a), ("give spider silk to dhelvanen", None));
    }

    #[test]
    fn mp_wildcard_step_and_bad_lines() {
        let src = b"[d][a]\r\n00100001:00100001:2:-1:0:::\r\nFFFFFFFF:0080:n\r\nnot a step\r\n";
        let mp = parse_mp("X.mp", src).unwrap();
        assert_eq!(mp.steps[0].id, WILDCARD);
        assert_eq!(mp.steps[0].flags, 0x80);
        assert_eq!(mp.bad_lines, ["not a step"]);
    }

    #[test]
    fn mp_without_header_is_an_error() {
        assert!(parse_mp("X.mp", b"[d][a]\r\n").is_err());
        assert!(parse_mp("X.mp", b"[d][a]\r\nzz:yy:1:-1:0:::\r\n").is_err());
    }

    #[test]
    fn rooms_md_line() {
        let (wps, bad) = parse_rooms_md(
            b"CAB00180:00004040:0:0:0:AALY:Ancient Ruin:Ancient Ruin Dark Alley\r\n\
              00000001:0000000A:0:9:4:TRCL:Trainers:Cleric: Guild\r\nbroken\r\n",
        );
        assert_eq!(bad, ["broken"]);
        assert_eq!(wps.len(), 2);
        assert_eq!(wps[0].id, 0xCAB0_0180);
        assert_eq!(wps[0].code, "AALY");
        assert_eq!(wps[0].group, "Ancient Ruin");
        assert!(!wps[0].is_shop());
        assert!(wps[1].is_shop() && wps[1].is_trainer() && !wps[1].is_bank());
        assert_eq!((wps[1].max_level, wps[1].class), (9, 4));
        assert_eq!(wps[1].name, "Cleric: Guild");
    }

    #[test]
    fn directions() {
        assert_eq!(direction("NE"), Some(4));
        assert_eq!(direction("down"), Some(9));
        assert_eq!(direction("go path"), None);
    }
}
