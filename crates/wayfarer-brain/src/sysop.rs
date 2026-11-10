//! `sysop goto <town>` (C-080): the towns a sysop character may teleport to, and the room each
//! lands in (`kb/navigation.md` "SYSOP GOTO"). By policy only these towns, never a room number.

use wayfarer_world::Key;

/// A town `sysop goto` takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Town {
    /// The word after `sysop goto`, and the protocol's `town`.
    pub word: &'static str,
    /// As the narration says it.
    pub name: &'static str,
    /// The room it lands in.
    pub room: Key,
}

pub(crate) const TOWNS: [Town; 5] = [
    Town {
        word: "newhaven",
        name: "Newhaven",
        room: (1, 2150),
    },
    Town {
        word: "silvermere",
        name: "Silvermere",
        room: (1, 2189),
    },
    Town {
        word: "rhudaur",
        name: "Rhudaur",
        room: (2, 2519),
    },
    Town {
        word: "khazarad",
        name: "Khazarad",
        room: (6, 1249),
    },
    Town {
        word: "lostcity",
        name: "the Lost City",
        room: (16, 426),
    },
];

/// The town the protocol's `town` names.
pub(crate) fn town(word: &str) -> Option<&'static Town> {
    TOWNS.iter().find(|t| t.word == word)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_five_towns_by_their_word() {
        assert_eq!(town("rhudaur").map(|t| t.room), Some((2, 2519)));
        assert_eq!(town("lostcity").map(|t| t.name), Some("the Lost City"));
        assert_eq!(town("rhud"), None);
        assert_eq!(TOWNS.len(), 5);
    }
}
