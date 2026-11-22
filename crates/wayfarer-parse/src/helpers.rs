//! Splitting the list fields of room and inventory events (`patterns.py` field helpers).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;

/// One exit of an `Obvious exits:` list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Exit {
    /// `north` ... `down`; `None` for an exit shown by its own text with no direction in it
    /// (`a hole`).
    pub dir: Option<String>,
    /// `normal`, `door`, `gate`, `trap door`, `secret` or `custom`.
    pub kind: String,
    /// `open` or `closed` for doors and gates; the whole text for a `custom` exit.
    pub state: Option<String>,
}

fn exit_rx() -> &'static Regex {
    static RX: OnceLock<Regex> = OnceLock::new();
    RX.get_or_init(|| {
        Regex::new(
            r"^(?:(?:(?P<state>closed|open) (?P<kind>door|gate|trap door) |(?P<secret>[Ss]ecret passage) )?(?P<dir>north|south|east|west|northeast|northwest|southeast|southwest|up|down|above|below|East))$",
        )
        .expect("compiles")
    })
}

/// A direction as the exits line spells it, as the command names it: trap doors' `above` and
/// `below` are `up` and `down`.
fn direction(d: &str) -> String {
    match d.to_lowercase().as_str() {
        "above" => "up".to_owned(),
        "below" => "down".to_owned(),
        d => d.to_owned(),
    }
}

/// `parse_exits`: `closed door north, up, open trap door below` to exits. Trap doors name up
/// and down as above and below.
pub fn parse_exits(exits: &str) -> Vec<Exit> {
    let mut out = Vec::new();
    if exits.trim() == "NONE!!!" {
        return out;
    }
    let parts: Vec<&str> = exits.split(',').collect();
    for (i, part) in parts.iter().enumerate() {
        let part = part.trim().trim_end_matches('.');
        if part.is_empty() {
            // An empty slot between two commas is an exit the game printed with no name and no
            // direction: an opened Hidden exit whose name message is blank (`north, , west`,
            // C-076). A comma at either end is no slot.
            if i > 0 && i + 1 < parts.len() {
                out.push(Exit {
                    dir: None,
                    kind: "custom".into(),
                    state: Some(String::new()),
                });
            }
            continue;
        }
        let Some(c) = exit_rx().captures(part) else {
            // A Hidden exit opened shows by its own name with the direction last (`exitpar4`'s
            // message: `cramped passage south`, `crumbling hole north`, `open stone door east`).
            let dir = part
                .rsplit(' ')
                .next()
                .filter(|_| part.contains(' '))
                .and_then(|w| exit_rx().captures(w))
                .map(|c| direction(&c["dir"]));
            out.push(Exit {
                dir,
                kind: "custom".into(),
                state: Some(part.to_owned()),
            });
            continue;
        };
        let d = direction(&c["dir"]);
        let kind = match (c.name("kind"), c.name("secret")) {
            (Some(k), _) => k.as_str().to_owned(),
            (None, Some(_)) => "secret".to_owned(),
            (None, None) => "normal".to_owned(),
        };
        out.push(Exit {
            dir: Some(d),
            kind,
            state: c.name("state").map(|m| m.as_str().to_owned()),
        });
    }
    out
}

/// One item of a `You are carrying` list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Carried {
    pub name: String,
    /// Where it is worn or held: `Torso`, `Weapon Hand`, `Readied/3`, ...
    pub slot: Option<String>,
    pub count: u32,
}

const COIN: &str = "runic coins?|platinum pieces?|gold crowns?|silver nobles?|copper farthings?";

fn coin_rx() -> &'static Regex {
    static RX: OnceLock<Regex> = OnceLock::new();
    RX.get_or_init(|| Regex::new(&format!(r"^(?P<n>\d+) (?P<coin>{COIN})$")).expect("compiles"))
}

fn slot_rx() -> &'static Regex {
    static RX: OnceLock<Regex> = OnceLock::new();
    RX.get_or_init(|| {
        Regex::new(
            r"^(?P<name>.+?) \((?P<slot>Two handed|Weapon Hand|Readied/\d+|Nowhere|Worn|Head|Hands|Finger|Feet|Arms|Back|Neck|Legs|Waist|Torso|Off-Hand|Wrist|Ears|Eyes|Face)\)$",
        )
        .expect("compiles")
    })
}

fn count_rx() -> &'static Regex {
    static RX: OnceLock<Regex> = OnceLock::new();
    RX.get_or_init(|| Regex::new(r"^(\d+) (.+)$").expect("compiles"))
}

/// `parse_carrying`: the `You are carrying` list to coins by name and items. Slots are the
/// game's worn locations (LJNTABLE.C:273), `Readied/N`, `Two handed` and `Weapon Hand`.
pub fn parse_carrying(items: &str) -> (BTreeMap<String, u64>, Vec<Carried>) {
    let mut coins = BTreeMap::new();
    let mut out = Vec::new();
    if matches!(items.trim(), "Nothing!" | "Nothing") {
        return (coins, out);
    }
    for part in items.split(", ") {
        let part = part.trim();
        if let Some(c) = coin_rx().captures(part) {
            coins.insert(c["coin"].to_owned(), c["n"].parse().unwrap_or(0));
        } else if let Some(c) = slot_rx().captures(part) {
            out.push(Carried {
                name: c["name"].to_owned(),
                slot: Some(c["slot"].to_owned()),
                count: 1,
            });
        } else if let Some(c) = count_rx().captures(part) {
            out.push(Carried {
                name: c[2].to_owned(),
                slot: None,
                count: c[1].parse().unwrap_or(1),
            });
        } else {
            out.push(Carried {
                name: part.to_owned(),
                slot: None,
                count: 1,
            });
        }
    }
    (coins, out)
}

/// What the colour of a name in `Also here:` says of the monster's alignment (C-129). The
/// game colours good and neutral names cyan (`ESC[0;36m`), lawful ones grey (`ESC[0;37m`:
/// lawful good for a viewer under 40 evil points, lawful evil for an outlaw) and every evil
/// one in the player's "here names" palette colour, which may be no colour of its own at all
/// (`display/room.rs` `monster_colour`, mirroring `LJNDISP.C`; `kb/combat.md`). So cyan and
/// grey are the game's, and anything else, or nothing, is evil.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameColour {
    GoodOrNeutral,
    Lawful,
    Evil,
}

/// The alignment an SGR's parameters (`1;35`) say: bright anything is the names colour (evil);
/// plain cyan good or neutral; plain grey (white) lawful; else evil.
pub fn name_colour(params: &str) -> NameColour {
    let codes: Vec<u32> = params.split(';').filter_map(|c| c.parse().ok()).collect();
    if codes.contains(&1) {
        return NameColour::Evil;
    }
    match codes.iter().rev().find(|c| (30..=37).contains(*c)) {
        Some(36) => NameColour::GoodOrNeutral,
        Some(37) => NameColour::Lawful,
        _ => NameColour::Evil,
    }
}

/// One name of an `Also here:` list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Present {
    pub name: String,
    /// A capitalised single word: a player, or a named monster (check the monster names).
    pub player: bool,
    /// `Hidden`, `Charmed`, `star` (the outlaw/evil mark).
    pub flags: Vec<String>,
    /// What its colour says of its alignment, when the line's colours are known (C-129).
    pub colour: Option<NameColour>,
}

/// `parse_also_here`: the `Also here:` list to names (LJNDISP.C:2576,2723).
pub fn parse_also_here(who: &str) -> Vec<Present> {
    parse_also_here_coloured(who, &[])
}

/// [`parse_also_here`] with the list's colours, `(byte offset into who, SGR parameters)` as
/// [`crate::Event::colours`] has them for the line: each name takes the colour in force at
/// its first character. With no colours, no name has one.
pub fn parse_also_here_coloured(who: &str, colours: &[(usize, String)]) -> Vec<Present> {
    let mut out = Vec::new();
    let mut at = 0;
    for part in who.split(", ") {
        let start = at + (part.len() - part.trim_start().len());
        at += part.len() + 2;
        let mut part = part.trim();
        let mut flags = Vec::new();
        for (suffix, flag) in [
            ("(Hidden)", "Hidden"),
            (" (Charmed)", "Charmed"),
            ("*", "star"),
        ] {
            if let Some(p) = part.strip_suffix(suffix) {
                flags.push(flag.to_owned());
                part = p;
            }
        }
        let player = part.chars().next().is_some_and(char::is_uppercase) && !part.contains(' ');
        let colour = colours
            .iter()
            .rfind(|(o, _)| *o <= start)
            .map(|(_, p)| name_colour(p));
        out.push(Present {
            name: part.to_owned(),
            player,
            flags,
            colour,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exits() {
        let e = parse_exits(
            "closed door north, up, open trap door below, Secret passage east, a hole.",
        );
        assert_eq!(
            e[0],
            Exit {
                dir: Some("north".into()),
                kind: "door".into(),
                state: Some("closed".into())
            }
        );
        assert_eq!(
            e[1],
            Exit {
                dir: Some("up".into()),
                kind: "normal".into(),
                state: None
            }
        );
        assert_eq!(
            e[2],
            Exit {
                dir: Some("down".into()),
                kind: "trap door".into(),
                state: Some("open".into())
            }
        );
        assert_eq!(e[3].kind, "secret");
        assert_eq!(
            e[4],
            Exit {
                dir: None,
                kind: "custom".into(),
                state: Some("a hole".into())
            }
        );
        assert!(parse_exits("NONE!!!").is_empty());
    }

    #[test]
    fn an_opened_hidden_exit_by_its_own_name_keeps_its_direction() {
        // The Crypt's knight statue (1:1250) after `push eye`: `exitpar4`'s "cramped passage %s".
        let e = parse_exits("cramped passage south, northeast, northwest");
        assert_eq!(
            e[0],
            Exit {
                dir: Some("south".into()),
                kind: "custom".into(),
                state: Some("cramped passage south".into())
            }
        );
        assert_eq!(e[1].dir.as_deref(), Some("northeast"));
        for (text, dir) in [
            ("crumbling hole north", "north"),
            ("open stone door southwest", "southwest"),
            ("concealed doorway up", "up"),
        ] {
            assert_eq!(parse_exits(text)[0].dir.as_deref(), Some(dir), "{text}");
        }
        // No direction word last: no direction.
        assert_eq!(parse_exits("a hole")[0].dir, None);
        assert_eq!(parse_exits("south wind")[0].dir, None);
    }

    #[test]
    fn an_empty_slot_between_commas_is_an_exit_with_no_name() {
        // The Labyrinth Passage (1/1972) after `river`: its east exit's name message is blank
        // (C-076).
        let e = parse_exits("north, , west");
        assert_eq!(e.len(), 3);
        assert_eq!(e[0].dir.as_deref(), Some("north"));
        assert_eq!(
            e[1],
            Exit {
                dir: None,
                kind: "custom".into(),
                state: Some(String::new())
            }
        );
        assert_eq!(e[2].dir.as_deref(), Some("west"));
        // A comma at either end is no slot.
        assert_eq!(parse_exits("north, west,").len(), 2);
        assert_eq!(parse_exits(", north").len(), 1);
    }

    #[test]
    fn carrying() {
        let (coins, items) = parse_carrying(
            "18 silver nobles, padded vest (Torso), quarterstaff (Two handed), 3 torch, rope",
        );
        assert_eq!(coins["silver nobles"], 18);
        assert_eq!(
            items[0],
            Carried {
                name: "padded vest".into(),
                slot: Some("Torso".into()),
                count: 1
            }
        );
        assert_eq!(
            items[2],
            Carried {
                name: "torch".into(),
                slot: None,
                count: 3
            }
        );
        assert_eq!(items[3].name, "rope");
        assert_eq!(parse_carrying("Nothing!"), (BTreeMap::new(), vec![]));
    }

    #[test]
    fn also_here() {
        let p = parse_also_here("Thornwyn*, giant rat, Bob(Hidden), kobold (Charmed)");
        assert_eq!(
            p[0],
            Present {
                name: "Thornwyn".into(),
                player: true,
                flags: vec!["star".into()],
                colour: None,
            }
        );
        assert!(!p[1].player);
        assert_eq!(p[2].flags, ["Hidden"]);
        assert_eq!(
            p[3],
            Present {
                name: "kobold".into(),
                player: false,
                flags: vec!["Charmed".into()],
                colour: None,
            }
        );
    }

    /// C-129: Asa's line (silver-mirror, 2026-11-22), the names' colours as the game sends
    /// them: the label's magenta, the lunatic's bright magenta, a reset after it.
    #[test]
    fn also_here_names_carry_their_colour() {
        let who = "crazed lunatic";
        let p = parse_also_here_coloured(who, &[(0, "1;35".into()), (14, "0".into())]);
        assert_eq!(p[0].colour, Some(NameColour::Evil));
        // Cyan is good or neutral, grey lawful; the colour in force at the name counts, so a
        // second name after a reset and a plain comma is evil.
        let who = "unicorn, Sheriff Lionheart, giant rat";
        let colours = [
            (0, "0;36".into()),
            (7, "0".into()),
            (9, "0;37".into()),
            (26, "0".into()),
        ];
        let p = parse_also_here_coloured(who, &colours);
        assert_eq!(
            p.iter().map(|w| w.colour).collect::<Vec<_>>(),
            [
                Some(NameColour::GoodOrNeutral),
                Some(NameColour::Lawful),
                Some(NameColour::Evil)
            ]
        );
        // A palette with no names colour of its own: the label's colour runs on, evil.
        let p = parse_also_here_coloured("giant rat", &[(0, "0;32".into())]);
        assert_eq!(p[0].colour, Some(NameColour::Evil));
        assert_eq!(name_colour("1;36"), NameColour::Evil);
        assert_eq!(name_colour("0;36"), NameColour::GoodOrNeutral);
        assert_eq!(name_colour("0;37;40"), NameColour::Lawful);
        assert_eq!(name_colour("0"), NameColour::Evil);
    }
}
