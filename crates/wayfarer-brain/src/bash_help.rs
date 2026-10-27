//! Helping the leader bash a door (docs/brain.md "Parties", MegaMud's *Help Leader Bash
//! Doors*): a follower with its leader, out of a fight, bashes the locked door the leader's walk
//! is bashing, one `bash <dir>` at a time, until it gives way. Each bash is the basher's own
//! roll (Strength plus the door's modifier, `kb/navigation.md` "Door / Gate"), so every
//! member at it opens the door sooner. A bash breaks off auto-combat, so never in a fight.

use wayfarer_world::{Key, mm};

use crate::executor::{Action, Confirmed, Expect, Outcome};
use crate::flee::dir_name;
use crate::modes::{Step, View};

#[derive(Debug, Default)]
pub(crate) struct BashHelp {
    /// The door of the `bash` in flight.
    bashing: Option<(Key, u8)>,
    /// A door it is done with while the leader is at it: it gave way, or the game would not
    /// have it bashed.
    done: Option<(Key, u8)>,
}

impl BashHelp {
    /// A `bash` at the leader's door, when it should help.
    pub fn step(&mut self, v: &View) -> Option<Step> {
        let door = v.party.help_bash();
        if self.done != door {
            self.done = None;
        }
        let (room, dir) = door?;
        if v.engaged || !v.settings.bool("party.help_bash") || self.done == door {
            return None;
        }
        let word = mm::DIRS.get(usize::from(dir))?;
        let leader = v.party.following().unwrap_or("the leader");
        self.bashing = Some((room, dir));
        Some(
            Step::send(
                Action::new(format!("bash {word}"), Expect::BashResult),
                format!("Helping {leader} bash the door {}", dir_name(dir)),
            )
            .doing(format!("helping {leader} bash a door")),
        )
    }

    /// The answer to its `bash`: a failure tries again; anything else (it gave way, it was
    /// open already, a refusal) ends its help at that door.
    pub fn outcome(&mut self, outcome: &Outcome) {
        let door = self.bashing.take();
        if !matches!(outcome, Outcome::Confirmed(Confirmed::Bash { ok: false })) {
            self.done = door;
        }
    }
}
