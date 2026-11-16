//! Looking into the next room (`l s`, `look north`): the game shows that room as it shows the
//! room the character stands in, title, `Also here:` and exits. Taken for the character's own,
//! the bot believed it had moved and attacked a monster it could not reach (C-103: Riley on
//! 2026-11-16, `l s` mid-fight, then `bash wizard`, said aloud). The room a look into a
//! direction shows goes to the screen only: the position, the fight, the presence and the rest
//! of the brain never see it.

use wayfarer_parse::typed::GameEvent;
use wayfarer_world::mm;

/// Where a look into a direction is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Peek {
    /// No look into a direction waiting for its room.
    #[default]
    None,
    /// A look into a direction went; its room has not started yet.
    Asked,
    /// The looked-at room is showing: its title came, its exits have not.
    Showing,
}

/// Whether `line` looks into a direction: `l`, `lo`, `loo` or `look`, then one direction
/// (`n`, `north`, `u`, `down`, ...). `look` alone, or at a monster or item, shows nothing of
/// another room.
pub fn looks_into(line: &str) -> bool {
    let mut words = line.split_whitespace();
    let (Some(verb), Some(dir), None) = (words.next(), words.next(), words.next()) else {
        return false;
    };
    let verb = verb.to_ascii_lowercase();
    ["l", "lo", "loo", "look"].contains(&verb.as_str()) && mm::direction(dir).is_some()
}

impl Peek {
    /// A line went to the game.
    pub fn sent(&mut self, line: &str) {
        if looks_into(line) {
            *self = Peek::Asked;
        }
    }

    /// An event from the game: whether it is part of the looked-at room, which only the screen
    /// gets.
    pub fn event(&mut self, ev: &GameEvent) -> bool {
        use GameEvent as G;
        match (*self, ev) {
            (Peek::None, _) => false,
            (Peek::Asked, G::RoomTitle { .. }) => {
                *self = Peek::Showing;
                true
            }
            // Too dark to see into: that is the whole answer.
            (Peek::Asked, G::RoomDark) => {
                *self = Peek::None;
                true
            }
            // The look answered with no room (no exit that way): nothing more comes.
            (Peek::Asked | Peek::Showing, G::Prompt { .. }) => {
                *self = Peek::None;
                false
            }
            (Peek::Asked, _) => false,
            (Peek::Showing, G::RoomExits { .. } | G::RoomDark) => {
                *self = Peek::None;
                true
            }
            (Peek::Showing, _) => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_look_into_a_direction_is_told_from_other_looks() {
        for line in ["l s", "look north", "L SE", "lo u", "loo down", " l  w "] {
            assert!(looks_into(line), "{line:?}");
        }
        for line in [
            "l",
            "look",
            "l wiz",
            "look rat",
            "l s s",
            "s",
            "lk s",
            "looking s",
        ] {
            assert!(!looks_into(line), "{line:?}");
        }
    }

    #[test]
    fn the_looked_at_room_is_kept_from_the_brain_until_its_exits() {
        let mut p = Peek::default();
        p.sent("l s");
        assert_eq!(p, Peek::Asked);
        assert!(p.event(&GameEvent::RoomTitle {
            title: "Alchemy Laboratory".into()
        }));
        assert_eq!(p, Peek::Showing);
        assert!(p.event(&GameEvent::RoomExits { exits: Vec::new() }));
        assert_eq!(p, Peek::None);
        // The character's own room after it: the brain's.
        assert!(!p.event(&GameEvent::RoomTitle {
            title: "Narrow Stone Tunnel".into()
        }));
    }

    #[test]
    fn a_look_into_the_dark_is_one_line() {
        let mut p = Peek::default();
        p.sent("look up");
        assert!(p.event(&GameEvent::RoomDark));
        assert_eq!(p, Peek::None);
        assert!(!p.event(&GameEvent::RoomDark));
    }
}
