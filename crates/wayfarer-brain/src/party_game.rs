//! The character's party as the game last showed it (docs/plan/10-party-play.md §4): whom it
//! follows, who `par` listed, and when. The game breaks parties without a word (a follower an
//! exit refuses drops silently, `kb/grouping.md` §2a), so this, not a party's definition, is
//! what the party coordinator (C-039) trusts; the lines between two `par` screens only mark it
//! stale.

use tokio::time::Instant;
use wayfarer_parse::typed::{GameEvent, Rank};

/// One row of a `par` screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ParMember {
    /// The full name (`Lauren Phillips`).
    pub name: String,
    pub class: Option<String>,
    pub mana: Option<i64>,
    pub hp: Option<i64>,
    pub resting: bool,
    pub rank: Option<Rank>,
    /// Invited, not following yet.
    pub invited: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct GameParty {
    /// The leader followed, by the name the game's lines give (the first name).
    pub following: Option<String>,
    /// The last whole `par` screen's rows, the character's own last.
    pub members: Vec<ParMember>,
    /// When that screen came.
    pub as_of: Option<Instant>,
    /// A join, a drop or a removal came since: `members` and `following` may be out of date
    /// until the next `par`.
    pub stale: bool,
    /// The last leader who invited the character, until it follows someone.
    pub invited_by: Option<String>,
    /// When the game last moved the character after its leader.
    pub moved: Option<Instant>,
    /// When a join, a drop or a removal last came (what makes it stale).
    pub changed: Option<Instant>,
    /// When the game last said `You must be invited first!`.
    pub must_invite: Option<Instant>,
    /// A `par` screen being read.
    screen: Option<Screen>,
}

#[derive(Debug, Clone, Default)]
struct Screen {
    following: Option<String>,
    members: Vec<ParMember>,
}

impl GameParty {
    /// Take one event; `me` is the character's full name when known (`st`). A `par` screen is
    /// taken whole when the next other event comes, and only if it ended with the character's
    /// own row (the game always prints it last), so one cut off is not half-applied.
    pub fn event(&mut self, ev: &GameEvent, me: Option<&str>, now: Instant) {
        match ev {
            GameEvent::ParFollowing { leader } => {
                self.close(me, now);
                self.screen = Some(Screen {
                    following: Some(leader.clone()),
                    members: Vec::new(),
                });
            }
            GameEvent::ParHeader | GameEvent::ParNone => {
                // `You are following X.` comes just before the header: the same screen.
                let following = match &self.screen {
                    Some(s) if s.members.is_empty() => s.following.clone(),
                    _ => {
                        self.close(me, now);
                        None
                    }
                };
                self.screen = Some(Screen {
                    following,
                    members: Vec::new(),
                });
            }
            GameEvent::ParRow {
                name,
                class,
                mana,
                hp,
                resting,
                rank,
                invited,
            } if self.screen.is_some() => {
                let s = self.screen.as_mut().expect("a screen");
                s.members.push(ParMember {
                    name: name.clone(),
                    class: class.clone(),
                    mana: *mana,
                    hp: *hp,
                    resting: *resting,
                    rank: *rank,
                    invited: *invited,
                });
            }
            other => {
                self.close(me, now);
                match other {
                    GameEvent::PartyInvited { leader } => self.invited_by = Some(leader.clone()),
                    GameEvent::PartyFollowing { leader } => {
                        self.following = Some(leader.clone());
                        self.invited_by = None;
                        self.changed(now);
                    }
                    GameEvent::PartyLeft { .. } => {
                        self.following = None;
                        self.changed(now);
                    }
                    GameEvent::PartyFollowerJoined { .. }
                    | GameEvent::PartyFollowerRemoved { .. } => self.changed(now),
                    // No invitation stands: one remembered was taken back (`uninvite`
                    // tells only the leader).
                    GameEvent::PartyMustBeInvited => {
                        self.must_invite = Some(now);
                        self.invited_by = None;
                    }
                    GameEvent::PartyMoved { .. } => self.moved = Some(now),
                    _ => {}
                }
            }
        }
    }

    fn changed(&mut self, now: Instant) {
        self.stale = true;
        self.changed = Some(now);
    }

    /// Whether the last whole `par` screen listed `name` (a full name) as following, not
    /// only invited.
    pub fn lists(&self, name: &str) -> bool {
        self.members.iter().any(|m| m.name == name && !m.invited)
    }

    /// The screen being read is over: take it if it is whole.
    fn close(&mut self, me: Option<&str>, now: Instant) {
        let Some(s) = self.screen.take() else {
            return;
        };
        let whole = match (s.members.last(), me) {
            (Some(last), Some(me)) => last.name == me,
            (Some(_), None) => true,
            (None, _) => false,
        };
        if whole {
            self.following = s.following;
            self.members = s.members;
            self.as_of = Some(now);
            self.stale = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str) -> GameEvent {
        GameEvent::ParRow {
            name: name.into(),
            class: Some("Cleric".into()),
            mana: Some(100),
            hp: Some(90),
            resting: false,
            rank: Some(Rank::Mid),
            invited: false,
        }
    }

    fn prompt() -> GameEvent {
        GameEvent::RoomDark
    }

    const ME: Option<&str> = Some("Lauren Phillips");

    #[tokio::test]
    async fn a_par_screen_fills_the_party() {
        let now = Instant::now();
        let mut p = GameParty::default();
        for ev in [
            GameEvent::ParFollowing {
                leader: "Penny".into(),
            },
            GameEvent::ParHeader,
            row("Penny Pax"),
            row("Lauren Phillips"),
            prompt(),
        ] {
            p.event(&ev, ME, now);
        }
        assert_eq!(p.following.as_deref(), Some("Penny"));
        let names: Vec<_> = p.members.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Penny Pax", "Lauren Phillips"]);
        assert_eq!((p.as_of, p.stale), (Some(now), false));
    }

    #[tokio::test]
    async fn a_screen_cut_off_leaves_the_last_whole_one() {
        let now = Instant::now();
        let mut p = GameParty::default();
        for ev in [GameEvent::ParNone, row("Lauren Phillips"), prompt()] {
            p.event(&ev, ME, now);
        }
        assert_eq!(p.members.len(), 1);
        // The own row never came: the screen is dropped.
        for ev in [GameEvent::ParHeader, row("Penny Pax"), prompt()] {
            p.event(&ev, ME, now);
        }
        assert_eq!(p.members[0].name, "Lauren Phillips");
        assert_eq!(p.following, None);
    }

    #[tokio::test]
    async fn a_refusal_forgets_an_old_invitation() {
        // Invited, then uninvited: the game says nothing to the invitee, so only the
        // refusal of its `follow` tells it the invitation is gone.
        let now = Instant::now();
        let mut p = GameParty::default();
        p.event(
            &GameEvent::PartyInvited {
                leader: "Penny".into(),
            },
            ME,
            now,
        );
        p.event(&GameEvent::PartyMustBeInvited, ME, now);
        assert_eq!(p.invited_by, None);
        assert_eq!(p.must_invite, Some(now));
    }

    #[tokio::test]
    async fn joining_and_leaving_between_screens() {
        let now = Instant::now();
        let mut p = GameParty::default();
        p.event(
            &GameEvent::PartyInvited {
                leader: "Penny".into(),
            },
            ME,
            now,
        );
        assert_eq!(p.invited_by.as_deref(), Some("Penny"));
        p.event(
            &GameEvent::PartyFollowing {
                leader: "Penny".into(),
            },
            ME,
            now,
        );
        assert_eq!(
            (p.following.as_deref(), p.invited_by.as_deref(), p.stale),
            (Some("Penny"), None, true)
        );
        p.event(
            &GameEvent::PartyMoved {
                dir: "north".into(),
            },
            ME,
            now,
        );
        assert_eq!(p.moved, Some(now));
        p.event(
            &GameEvent::PartyLeft {
                leader: "Penny".into(),
            },
            ME,
            now,
        );
        assert_eq!((p.following.as_deref(), p.stale), (None, true));
    }
}
