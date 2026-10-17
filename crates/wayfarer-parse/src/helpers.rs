//! Splitting the list fields of room and inventory events (`patterns.py` field helpers).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;

/// One exit of an `Obvious exits:` list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Exit {
    /// `north` ... `down`; `None` for a found hidden exit shown by its own text.
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

/// `parse_exits`: `closed door north, up, open trap door below` to exits. Trap doors name up
/// and down as above and below.
pub fn parse_exits(exits: &str) -> Vec<Exit> {
    let mut out = Vec::new();
    if exits.trim() == "NONE!!!" {
        return out;
    }
    for part in exits.split(',') {
        let part = part.trim().trim_end_matches('.');
        if part.is_empty() {
            continue;
        }
        let Some(c) = exit_rx().captures(part) else {
            out.push(Exit {
                dir: None,
                kind: "custom".into(),
                state: Some(part.to_owned()),
            });
            continue;
        };
        let d = c["dir"].to_lowercase();
        let d = match d.as_str() {
            "above" => "up".to_owned(),
            "below" => "down".to_owned(),
            _ => d,
        };
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

/// One name of an `Also here:` list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Present {
    pub name: String,
    /// A capitalised single word: a player, or a named monster (check the monster names).
    pub player: bool,
    /// `Hidden`, `Charmed`, `star` (the outlaw/evil mark).
    pub flags: Vec<String>,
}

/// `parse_also_here`: the `Also here:` list to names (LJNDISP.C:2576,2723).
pub fn parse_also_here(who: &str) -> Vec<Present> {
    who.split(", ")
        .map(|part| {
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
            Present {
                name: part.to_owned(),
                player,
                flags,
            }
        })
        .collect()
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
                flags: vec!["star".into()]
            }
        );
        assert!(!p[1].player);
        assert_eq!(p[2].flags, ["Hidden"]);
        assert_eq!(
            p[3],
            Present {
                name: "kobold".into(),
                player: false,
                flags: vec!["Charmed".into()]
            }
        );
    }
}
