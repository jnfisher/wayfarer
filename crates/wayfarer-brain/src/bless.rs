//! Blessings (C-047, docs/brain.md "Blessings"): the lasting spells the player ticked for the
//! character (bless, barkskin, holy aura) are kept on. With the mana to spare
//! (`spells.bless_mana_above`), Bless casts each one the bot does not believe is on: between
//! fights ahead of a rest (the party's too) unless the character is under its own rest line,
//! and in a fight after a heal (`spells.bless_in_combat`, C-056). One is on from a cast that did not fail for as long as it lasts at the
//! character's level (`BuffDef::rounds`), or until its wear-off line; off at a death. A cast
//! that fails is tried again after a round; [`FAILS`] in a row hold that spell for [`HOLD`]
//! (it may be on already and not renewable). A cast is done at the first prompt after it, which
//! another player's line may bring before the cast's own: its failure line coming within the
//! round after still counts. Between fights, while one is believed on, the status screen is
//! looked at every `spells.bless_check_seconds`: it lists the line of every spell on the
//! character after its last row, and one believed on that it does not show is off.
//!
//! A buff item (C-109) is kept on the same way, between fights only, by its `use`: the bot
//! looks at its inventory, wields or wears the item if it is not in place (the game uses a
//! weapon only wielded and a worn item only worn), uses it, puts back what it took the place
//! of, and looks again.
//!
//! A blessing on another of our characters (C-131): a Self or Single spell ticked for a party
//! member is cast at it by name (`sped riley`) when the member stands in the room and its own
//! session does not believe the spell on it. Every session watches the lines of every lasting
//! spell for its own character ("You feel fast!", "You slow down.", the status screen) and
//! says what is on it ([`Bless::buffs_on`]), which the party coordinator carries to the caster
//! as its mate's `buffs`.

use std::collections::BTreeSet;
use std::time::Duration;

use tokio::time::Instant;
use wayfarer_parse::typed::GameEvent;
use wayfarer_world::{BuffDef, BuffItem, ItemPlace};

use crate::executor::{Action, Expect, Outcome};
use crate::modes::{Mode, Step, View};

/// A round: one general spell goes per round (`kb/spells.md`).
const ROUND: Duration = Duration::from_secs(5);
/// Failed casts in a row before the spell is held.
const FAILS: u32 = 3;
/// How long a spell that keeps failing is left alone.
const HOLD: Duration = Duration::from_secs(300);
/// The status screen's spell list for a sysop with debug on, in place of the spells' lines.
const SPELLS_LISTED: &str = "You have the following spell cast upon you:";

/// Whom a kept spell is for (C-131).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    /// The caster itself, or its party for a party spell: no target word.
    Me,
    /// Another of our characters, by the first name the game takes (`riley`).
    Member(String),
}

impl Target {
    /// The name as it is said: `Riley`.
    fn shown(name: &str) -> String {
        let mut c = name.chars();
        match c.next() {
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            None => String::new(),
        }
    }
}

/// How a cast in flight came out, from the lines before its prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CastResult {
    Pending,
    /// "You attempt to cast speed, but fail.", no mana, or one already this round.
    Failed,
    /// "You do not see riley here!": the member was not there after all.
    Missing,
    /// "You may not cast that spell on a user!": the spell cannot go on a player.
    NotOnPlayer,
}

#[derive(Debug, Clone)]
struct Casting {
    short: String,
    target: Target,
    at: Instant,
    result: CastResult,
}

#[derive(Debug, Clone)]
struct Kept {
    def: BuffDef,
    target: Target,
    /// On until then, as far as the bot knows.
    on_until: Option<Instant>,
    /// Not cast again before this (a failure, or held).
    retry_at: Option<Instant>,
    fails: u32,
}

/// Where a buff item's swap is (C-109).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// `i`: what is carried, and where.
    Look,
    /// The inventory is in: wield or wear the item, or use it if it is in place.
    Decide,
    /// The wield or wear went.
    Equip,
    /// The use went.
    Use,
    /// What the item took the place of goes back.
    PutBack,
    /// `i` again: the inventory as it is now.
    LookAgain,
}

#[derive(Debug, Clone)]
struct Swap {
    /// The blessing's key: the item's name.
    short: String,
    item: BuffItem,
    stage: Stage,
    /// The command that puts back what the item took the place of.
    put_back: Option<String>,
    /// The item was wielded or worn for the use: the inventory is looked at again after.
    moved: bool,
    /// The game refused the step in flight (a wield, wear or use).
    refused: bool,
    /// What the wear in flight took off ("You have removed copper cross.").
    took_off: Option<String>,
    /// When the step in flight went.
    sent_at: Instant,
}

#[derive(Debug, Default)]
pub(crate) struct Bless {
    kept: Vec<Kept>,
    /// The cast in flight: the spell, whom it is for, when it went and what the game said.
    casting: Option<Casting>,
    /// The last cast taken as on (its spell, target and when it went), for a failure line that
    /// comes after the prompt that ended it.
    settled: Option<(String, Target, Instant)>,
    /// Every lasting spell the character could carry, for the lines it sees (C-131).
    watch: Vec<BuffDef>,
    /// The lasting spells (short names) believed on the character from its own lines and
    /// status screen, whoever cast them (C-131).
    seen: BTreeSet<String>,
    /// Spells the game would not cast on a player ("You may not cast that spell on a user!"):
    /// their member targets are dropped for the session.
    refused: BTreeSet<String>,
    /// Something to say once, at the next turn.
    notice: Option<String>,
    /// The character's level when it last cast: how long a spell lasts.
    level: i64,
    /// How often the status screen is looked at (`spells.bless_check_seconds`); zero: never.
    check_every: Duration,
    /// When a status screen last came, or a cast went (as good as a look).
    checked_at: Option<Instant>,
    /// The lines of the status screen coming in, up to its prompt.
    screen: Option<Vec<String>>,
    /// Spells the last screen did not show, for the cast's narration.
    unseen: Vec<String>,
    /// 1.20: the cast in flight only removed another tier of its regen family (the family's
    /// end line came during it: the tiers share their lines, `docs/kb/quicksilver-1.20.md`):
    /// it is cast again, and not taken as on.
    cleared: bool,
    /// A buff item being used (C-109).
    swap: Option<Swap>,
}

/// In a fight, or not known to be out of one.
fn fighting(v: &View) -> bool {
    v.engaged || !v.presence.attackers.is_empty()
}

impl Bless {
    /// The spells to keep on from now on, each with whom it is for; what is believed of those
    /// kept before stays. A spell the game refused on a player keeps no member targets.
    pub fn set(&mut self, defs: Vec<(BuffDef, Target)>) {
        let old = std::mem::take(&mut self.kept);
        self.kept = defs
            .into_iter()
            .filter(|(def, target)| *target == Target::Me || !self.refused.contains(&def.short))
            .map(|(def, target)| {
                old.iter()
                    .find(|k| k.def.short == def.short && k.target == target)
                    .map(|k| Kept {
                        def: def.clone(),
                        ..k.clone()
                    })
                    .unwrap_or(Kept {
                        def,
                        target,
                        on_until: None,
                        retry_at: None,
                        fails: 0,
                    })
            })
            .collect();
    }

    /// The lasting spells whose lines the character's own session reads for what is on it
    /// (C-131): every lasting spell of the pack, whatever the class.
    pub fn watch(&mut self, defs: Vec<BuffDef>) {
        self.watch = defs;
    }

    /// The lasting spells (short names) believed on the character from the lines it saw and
    /// its status screen, whoever cast them (C-131).
    pub fn buffs_on(&self) -> &BTreeSet<String> {
        &self.seen
    }

    /// Whether the spell `short` kept on the member `name` is believed on by this caster's
    /// own casts (C-131); what the member's session sees is the surer word.
    pub fn is_on_member(&self, short: &str, name: &str, now: Instant) -> bool {
        self.kept.iter().any(|k| {
            k.def.short == short
                && k.target == Target::Member(name.to_owned())
                && k.on_until.is_some_and(|t| now < t)
        })
    }

    fn find(&mut self, short: &str, target: &Target) -> Option<&mut Kept> {
        self.kept
            .iter_mut()
            .find(|k| k.def.short == short && k.target == *target)
    }

    /// The regen tier believed on at `now` (1.20), what it adds to mana regeneration.
    pub fn regen_on(&self, now: Instant) -> Option<i64> {
        self.kept
            .iter()
            .filter(|k| k.on_until.is_some_and(|t| now < t))
            .filter_map(|k| k.def.regen.map(|r| r.regen_pct))
            .max()
    }

    /// Whether the spell `short` is believed on the character at `now`.
    pub fn is_on(&self, short: &str, now: Instant) -> bool {
        self.kept.iter().any(|k| {
            k.def.short == short && k.target == Target::Me && k.on_until.is_some_and(|t| now < t)
        })
    }

    /// The first spell to cast now, if any: off, not held, and paid for without taking the
    /// mana under the blessing's line (`spells.bless_mana_above`).
    fn due<'a>(&'a self, v: &View) -> Option<&'a Kept> {
        let (mana, max) = v.mana?;
        let line = max * i64::from(v.bless_mana_above()) / 100;
        let fighting = fighting(v);
        self.kept.iter().find(|k| {
            (k.def.item.is_none() || !fighting)
                && k.on_until.is_none_or(|t| v.now >= t)
                && k.retry_at.is_none_or(|t| v.now >= t)
                && mana >= k.def.mana
                && mana - k.def.mana >= line
                && match &k.target {
                    Target::Me => true,
                    // A member: here with the character, and its own session not seeing the
                    // spell on it (C-131).
                    Target::Member(name) => v
                        .party
                        .mates()
                        .iter()
                        .any(|m| m.name == *name && !m.buffs.contains(&k.def.short)),
                }
        })
    }

    /// Look at the status screen this often from now on; zero: never.
    pub fn set_check(&mut self, every: Duration) {
        self.check_every = every;
    }

    /// When the status screen is next looked at: one is believed on, and none came for
    /// `check_every`.
    fn check_at(&self, now: Instant) -> Option<Instant> {
        if self.check_every.is_zero()
            || !self
                .kept
                .iter()
                .any(|k| k.on_until.is_some_and(|t| now < t))
        {
            return None;
        }
        Some(self.checked_at.map_or(now, |t| t + self.check_every))
    }

    /// When a spell comes due, may be tried again or is looked for on the status screen, for
    /// the clock to wake the bot.
    pub fn wake(&self, now: Instant) -> Option<Instant> {
        self.kept
            .iter()
            .flat_map(|k| [k.on_until, k.retry_at])
            .chain([self.check_at(now)])
            .flatten()
            .filter(|&t| t > now)
            .min()
    }

    /// The status screen came to its prompt with `seen` after its rows: a spell believed on
    /// that has a line of its own and is not among them is off. A sysop with debug on gets a
    /// list of the spells instead of their lines (`kb/screen-patterns.md` "Status screens"),
    /// which says nothing here.
    fn screen_seen(&mut self, seen: &[String], now: Instant) {
        if self.casting.is_some() || seen.iter().any(|l| l == SPELLS_LISTED) {
            return;
        }
        for k in &mut self.kept {
            let Some(felt) = k.def.felt.as_deref() else {
                continue;
            };
            if k.target == Target::Me
                && k.on_until.is_some_and(|t| now < t)
                && !seen.iter().any(|l| l == felt)
            {
                k.on_until = None;
                self.unseen.push(k.def.short.clone());
            }
        }
        // What is on the character, whoever cast it (C-131): the screen is the whole list.
        for def in &self.watch {
            if let Some(felt) = def.felt.as_deref() {
                if seen.iter().any(|l| l == felt) {
                    self.seen.insert(def.short.clone());
                } else {
                    self.seen.remove(&def.short);
                }
            }
        }
    }

    fn turn_on(&mut self, short: &str, target: &Target, at: Instant) {
        let level = self.level.max(1);
        if let Some(k) = self.find(short, target) {
            // On a member whose own session reads the spell's line, the member's word decides
            // (C-131): the caster's belief only covers the rounds before that word arrives.
            let rounds = if matches!(target, Target::Member(_)) && k.def.felt.is_some() {
                2
            } else {
                k.def.rounds(level)
            };
            k.on_until = Some(at + ROUND * u32::try_from(rounds.max(1)).unwrap_or(1));
            k.fails = 0;
            k.retry_at = None;
        }
        self.checked_at = Some(at);
    }

    /// The cast of `short` for `target` that went at `at` failed: tried again after a round,
    /// or held.
    fn failed(&mut self, short: &str, target: &Target, at: Instant) {
        if let Some(k) = self.find(short, target) {
            k.fails += 1;
            k.on_until = None;
            k.retry_at = Some(if k.fails >= FAILS {
                k.fails = 0;
                at + HOLD
            } else {
                at + ROUND
            });
        }
    }

    /// The member was not there for the cast that went at `at` (C-131): not a failure, looked
    /// at again after a round, by which time the mates list has caught up.
    fn away(&mut self, short: &str, target: &Target, at: Instant) {
        if let Some(k) = self.find(short, target) {
            k.on_until = None;
            k.retry_at = Some(at + ROUND);
        }
    }

    /// The game would not cast `short` on a player (C-131): its member targets are dropped for
    /// the session, said once.
    fn not_on_players(&mut self, short: &str) {
        let name = self
            .kept
            .iter()
            .find(|k| k.def.short == short)
            .map_or_else(|| short.to_owned(), |k| k.def.name.clone());
        self.kept
            .retain(|k| k.def.short != short || k.target == Target::Me);
        self.refused.insert(short.to_owned());
        self.notice = Some(format!("Blessing: {name} cannot be cast on another player"));
    }
}

impl Bless {
    /// Leaves the item `short` alone for [`HOLD`]: it cannot be put in place now.
    fn hold(&mut self, short: &str, now: Instant) {
        if let Some(k) = self.find(short, &Target::Me) {
            k.fails = 0;
            k.on_until = None;
            k.retry_at = Some(now + HOLD);
        }
    }

    /// The next step of the swap (C-109); None when it is over.
    fn swap_step(&mut self, v: &View) -> Option<Step> {
        let swap = self.swap.as_mut()?;
        swap.sent_at = v.now;
        let name = swap.item.name.clone();
        match swap.stage {
            Stage::Look => Some(
                Step::send(
                    Action::new("i", Expect::Prompt),
                    format!("Blessing: looking at the inventory to use the {name}"),
                )
                .doing(format!("using the {name}")),
            ),
            Stage::Decide => {
                let carried = |n: &str| v.items.iter().find(|c| c.name.eq_ignore_ascii_case(n));
                let Some(it) = carried(&name) else {
                    let short = swap.short.clone();
                    self.swap = None;
                    self.failed(&short, &Target::Me, v.now);
                    return Some(Step::wait(format!(
                        "Blessing: the {name} is not carried; trying again later"
                    )));
                };
                let slot =
                    |c: &wayfarer_parse::helpers::Carried| c.slot.clone().unwrap_or_default();
                if swap.item.place.is(&slot(it)) {
                    swap.stage = Stage::Use;
                    return self.swap_step(v);
                }
                // What stands in the way: the two hands and the off hand exclude each other.
                let blocker = match &swap.item.place {
                    ItemPlace::Weapon { two_handed: true } => v
                        .items
                        .iter()
                        .find(|c| slot(c) == wayfarer_world::OFF_HAND)
                        .map(|c| format!("with the {} worn in the off hand", c.name)),
                    ItemPlace::Worn { off_hand: true, .. } => v
                        .items
                        .iter()
                        .find(|c| slot(c) == "Two handed")
                        .map(|c| format!("with the two-handed {} in hand", c.name)),
                    _ => None,
                };
                if let Some(why) = blocker {
                    let short = swap.short.clone();
                    self.swap = None;
                    self.hold(&short, v.now);
                    return Some(Step::wait(format!(
                        "Blessing: can't use the {name} {why}; trying again in 5 minutes"
                    )));
                }
                let (verb, put_back) = match &swap.item.place {
                    ItemPlace::Weapon { .. } => (
                        "wield",
                        v.items
                            .iter()
                            .find(|c| wayfarer_world::is_weapon_slot(&slot(c)))
                            .map(|c| format!("wield {}", c.name)),
                    ),
                    // What the wear takes off is put back (`took_off`).
                    ItemPlace::Worn { .. } => ("wear", None),
                };
                swap.put_back = put_back;
                swap.moved = true;
                swap.refused = false;
                swap.took_off = None;
                swap.stage = Stage::Equip;
                Some(
                    Step::send(
                        Action::new(format!("{verb} {name}"), Expect::Prompt),
                        format!("Blessing: {verb}ing the {name} to use it"),
                    )
                    .doing(format!("using the {name}")),
                )
            }
            Stage::Use => {
                swap.refused = false;
                self.level = v.level;
                Some(
                    Step::send(
                        Action::new(format!("use {name}"), Expect::Prompt),
                        format!("Blessing: using the {name}"),
                    )
                    .doing(format!("using the {name}")),
                )
            }
            Stage::PutBack => {
                let Some(cmd) = swap.put_back.clone() else {
                    swap.stage = Stage::LookAgain;
                    return self.swap_step(v);
                };
                swap.refused = false;
                Some(
                    Step::send(
                        Action::new(cmd.clone(), Expect::Prompt),
                        format!("Blessing: putting back what the {name} replaced ({cmd})"),
                    )
                    .doing(format!("using the {name}")),
                )
            }
            Stage::LookAgain => Some(Step::send(
                Action::new("i", Expect::Prompt),
                format!("Blessing: used the {name}; looking at the inventory"),
            )),
            Stage::Equip => {
                // The wield or wear is in flight: nothing new until it answers.
                Some(Step::wait("Blessing: waiting for the item"))
            }
        }
    }

    /// The step of the swap in flight answered (C-109).
    fn swap_outcome(&mut self, outcome: &Outcome) {
        let Some(swap) = self.swap.as_mut() else {
            return;
        };
        let refused = swap.refused || matches!(outcome, Outcome::TimedOut);
        let (short, sent) = (swap.short.clone(), swap.sent_at);
        match swap.stage {
            Stage::Look => swap.stage = Stage::Decide,
            Stage::Decide => {}
            Stage::Equip => {
                if let Some(off) = swap.took_off.take() {
                    swap.put_back = Some(format!("wear {off}"));
                }
                if refused {
                    // Nothing used: what was taken off, if anything, goes back.
                    swap.stage = Stage::PutBack;
                    self.failed(&short, &Target::Me, sent);
                } else {
                    swap.stage = Stage::Use;
                }
            }
            Stage::Use => {
                if swap.moved {
                    swap.stage = Stage::PutBack;
                } else {
                    self.swap = None;
                }
                if refused {
                    self.failed(&short, &Target::Me, sent);
                } else {
                    self.turn_on(&short, &Target::Me, sent);
                }
            }
            Stage::PutBack => swap.stage = Stage::LookAgain,
            Stage::LookAgain => self.swap = None,
        }
    }
}

impl Mode for Bless {
    fn wants(&self, v: &View) -> Option<String> {
        // In a fight only with `spells.bless_in_combat` (C-056, after a heal: `Fight::may_bless`);
        // a rest, the party's too, gives way to it (`Recover::yields_to_bless`).
        let fighting = fighting(v);
        // A buff item's swap goes on between fights (C-109).
        if let Some(swap) = &self.swap
            && !fighting
        {
            return Some(format!("using the {}", swap.item.name));
        }
        if self.notice.is_some() {
            return Some("saying why a blessing was dropped".to_owned());
        }
        if fighting && !v.settings.bool("spells.bless_in_combat") {
            return None;
        }
        if let Some(k) = self.due(v) {
            return Some(match &k.target {
                Target::Me => format!("{} is off", k.def.name),
                Target::Member(name) => {
                    format!("{} is off {}", k.def.name, Target::shown(name))
                }
            });
        }
        // The status screen only between fights.
        (!fighting && self.check_at(v.now).is_some_and(|t| v.now >= t))
            .then(|| "checking the blessings are on".to_owned())
    }

    fn step(&mut self, v: &View) -> Step {
        if let Some(notice) = self.notice.take() {
            return Step::wait(notice);
        }
        if !fighting(v)
            && let Some(step) = self.swap_step(v)
        {
            return step;
        }
        if let Some((short, item)) = self
            .due(v)
            .and_then(|k| Some((k.def.short.clone(), k.def.item.clone()?)))
        {
            self.unseen.clear();
            self.swap = Some(Swap {
                short,
                item,
                stage: Stage::Look,
                put_back: None,
                moved: false,
                refused: false,
                took_off: None,
                sent_at: v.now,
            });
            if let Some(step) = self.swap_step(v) {
                return step;
            }
        }
        let Some(k) = self.due(v) else {
            if self.check_at(v.now).is_some_and(|t| v.now >= t) {
                self.checked_at = Some(v.now);
                return Step::send(
                    Action::new("st", Expect::Prompt),
                    "Blessing: looking at the status screen",
                )
                .doing("checking the blessings are on");
            }
            return Step::wait("Blessing: nothing to cast");
        };
        let (short, name, target) = (k.def.short.clone(), k.def.name.clone(), k.target.clone());
        // A spell on the caster or the party takes no target word; one on a member takes its
        // name (C-131): `sped riley`.
        let (command, narration, doing) = match &target {
            Target::Me if self.unseen.contains(&short) => (
                short.clone(),
                format!("Blessing: {name} is not on the status screen; casting it"),
                format!("casting {name}"),
            ),
            Target::Me => (
                short.clone(),
                format!("Blessing: casting {name}"),
                format!("casting {name}"),
            ),
            Target::Member(who) => (
                format!("{short} {who}"),
                format!("Blessing: casting {name} on {}", Target::shown(who)),
                format!("casting {name} on {}", Target::shown(who)),
            ),
        };
        self.unseen.clear();
        self.casting = Some(Casting {
            short,
            target,
            at: v.now,
            result: CastResult::Pending,
        });
        self.settled = None;
        self.level = v.level;
        Step::send(Action::new(command, Expect::Prompt), narration).doing(doing)
    }

    fn on_event(&mut self, ev: &GameEvent, now: Instant) {
        if let Some(swap) = &mut self.swap {
            match ev {
                GameEvent::WearFailed
                | GameEvent::NotWielded
                | GameEvent::NoMana
                | GameEvent::LearnRefused => swap.refused = true,
                GameEvent::Other(e)
                    if matches!(e.name.as_str(), "use_forbidden" | "offhand_2h") =>
                {
                    swap.refused = true;
                }
                // What a wear took off, to put back.
                GameEvent::Other(e) if e.name == "removed" => {
                    if let Some(item) = e.text("item") {
                        swap.took_off = Some(item.to_owned());
                    }
                }
                GameEvent::YouKilled => self.swap = None,
                _ => {}
            }
        }
        match ev {
            GameEvent::CastFailed { spell } if self.casting.is_none() => {
                // The cast taken as on, failed after all: by name, so another spell's failure
                // (Fight's) is not taken for it.
                if let Some((short, target, at)) = self.settled.take() {
                    let named = self
                        .kept
                        .iter()
                        .any(|k| k.def.short == short && k.def.name.eq_ignore_ascii_case(spell));
                    if named && now < at + ROUND {
                        self.failed(&short, &target, at);
                    } else {
                        self.settled = Some((short, target, at));
                    }
                }
            }
            GameEvent::CastFailed { .. } | GameEvent::NoMana | GameEvent::AlreadyCast => {
                if let Some(c) = &mut self.casting {
                    c.result = CastResult::Failed;
                }
            }
            // The member the cast was for is not here (C-131).
            GameEvent::YouDoNotSee { .. } => {
                if let Some(c) = &mut self.casting
                    && matches!(c.target, Target::Member(_))
                {
                    c.result = CastResult::Missing;
                }
            }
            // "You may not cast that spell on a user!" (C-131).
            GameEvent::Other(e) if e.name == "bad_target" => {
                if let Some(c) = &mut self.casting
                    && matches!(c.target, Target::Member(_))
                {
                    c.result = CastResult::NotOnPlayer;
                }
            }
            GameEvent::YouKilled => {
                for k in &mut self.kept {
                    k.on_until = None;
                }
                self.seen.clear();
            }
            // 1.20's regen line (C-067): its lines are typed, by family. Its end: the tier of
            // that family is off; during a cast of it, the cast only removed the old tier.
            GameEvent::RegenLine {
                family,
                kind,
                who: None,
            } if kind == "ends" => {
                let casting = self.casting.as_ref().map(|c| c.short.clone());
                for k in &mut self.kept {
                    if k.def.regen.is_some_and(|r| r.family.as_str() == family) {
                        k.on_until = None;
                        k.retry_at = None;
                        k.fails = 0;
                        if casting.as_deref() == Some(k.def.short.as_str()) {
                            self.cleared = true;
                        }
                    }
                }
            }
            // How it shows on the status screen: one of the screen's spell lines.
            GameEvent::RegenLine {
                family,
                kind,
                who: None,
            } if kind == "status" => {
                if let (Some(screen), Some(k)) = (
                    &mut self.screen,
                    self.kept
                        .iter()
                        .find(|k| k.def.regen.is_some_and(|r| r.family.as_str() == family)),
                ) && let Some(felt) = &k.def.felt
                {
                    screen.push(felt.clone());
                }
            }
            // The status screen's first row: its spell lines follow its last, up to the prompt.
            GameEvent::StatusName { .. } => {
                self.screen = Some(Vec::new());
                self.checked_at = Some(now);
            }
            GameEvent::Prompt { .. } => {
                if let Some(seen) = self.screen.take() {
                    self.screen_seen(&seen, now);
                }
            }
            _ => {
                let Some(line) = crate::modes::line_of(ev) else {
                    return;
                };
                let line = line.trim();
                if let Some(screen) = &mut self.screen {
                    screen.push(line.to_owned());
                }
                for k in &mut self.kept {
                    if k.target == Target::Me && k.def.wear_off.as_deref() == Some(line) {
                        k.on_until = None;
                        k.retry_at = None;
                        k.fails = 0;
                    }
                }
                // The line its target sees: on, whoever cast it.
                if let Some(short) = self
                    .kept
                    .iter()
                    .find(|k| {
                        k.target == Target::Me
                            && k.def.felt.as_deref() == Some(line)
                            && k.on_until.is_none_or(|t| now >= t)
                    })
                    .map(|k| k.def.short.clone())
                {
                    self.turn_on(&short, &Target::Me, now);
                }
                // What is on the character, whoever cast it (C-131), by every lasting spell's
                // lines; wind of haste and speed share "You feel fast!", so both count.
                for def in &self.watch {
                    if def.wear_off.as_deref() == Some(line) {
                        self.seen.remove(&def.short);
                    } else if def.felt.as_deref() == Some(line) {
                        self.seen.insert(def.short.clone());
                    }
                }
            }
        }
    }

    fn outcome(&mut self, _action: &Action, outcome: Outcome) {
        if self.swap.is_some() && self.casting.is_none() {
            self.swap_outcome(&outcome);
            return;
        }
        let Some(Casting {
            short,
            target,
            at,
            result,
        }) = self.casting.take()
        else {
            return;
        };
        let failed = result == CastResult::Failed;
        if std::mem::take(&mut self.cleared) && !failed {
            // The old tier is gone; the new one goes on with a second cast, now.
            if let Some(k) = self.find(&short, &target) {
                k.on_until = None;
                k.retry_at = None;
            }
        } else if result == CastResult::Missing {
            self.away(&short, &target, at);
        } else if result == CastResult::NotOnPlayer {
            self.not_on_players(&short);
        } else if failed
            || matches!(
                outcome,
                Outcome::TimedOut | Outcome::Refused(crate::executor::Refused::Afraid)
            )
        {
            self.failed(&short, &target, at);
        } else {
            self.turn_on(&short, &target, at);
            self.settled = Some((short, target, at));
        }
    }
}
