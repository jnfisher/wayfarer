//! The walker: a route to a room over the world graph, walked one confirmed step at a time
//! (docs/brain.md "Walking"). It decides what to send next from each action's outcome; the
//! session sends it through the executor. It never fights: combat stops it.
//!
//! A pipelined walk (Task mode's legs) may have two moves in flight: with the belief sure of
//! the room, the next plain move goes ahead of the one in flight ([`Next::ahead`]). Each room
//! confirms its move in order. When the first goes wrong (a room that disagrees, a refusal, a
//! door) the move ahead has gone already: its confirmation is void, and once its answer is in
//! the walker plans again from wherever the belief puts the character.
//!
//! A locked door the character cannot open (its odds nil, [`Traveller::lock_odds`]) is given
//! up at once, with no `bash`. A route through a Hidden exit not showing in the room's exits
//! line searches for it first (`search <dir>`) until it is found, up to [`MAX_SEARCHES`] times.
//!
//! A route through an Item or Key exit whose item a shop sells goes by the shop when that is the
//! only or the shorter way ([`route_buying`]): there the walker sends `buy <item>` and goes on. A
//! one-charge item (a room ticket) is used up by its exit, so it is no longer counted as carried.
//!
//! [`Traveller::lock_odds`]: wayfarer_world::Traveller::lock_odds

use wayfarer_parse::typed::{GameEvent, Refusal};
use wayfarer_proto::{BossLair, RoomKey, Walking};
use wayfarer_world::{Belief, ExitType, Gate, Key, Purchase, Route, Step, mm, route_buying};

use crate::brain::Atlas;
use crate::executor::{Action, Confirmed, Expect, Outcome, Refused};
use crate::flee::dir_name;
use crate::position::Position;

/// `bash` commands sent at one locked door before the walker gives up on it.
pub const MAX_BASHES: u32 = 3;
/// The default of `movement.search_limit`: `search` commands for one hidden exit that find nothing
/// before the walker blocks it for the
/// session. A search is a dice roll (`max(3, perception - 15)` percent, `kb/navigation.md`
/// "Hidden"), so a clumsy character needs many: at the 3% floor 300 empty searches in a row is
/// a one in ten thousand chance, which means the belief is wrong, not the dice.
pub const MAX_SEARCHES: u32 = 300;
/// After the first empty search, and after every this many more, the walker sends `look`: an
/// exit found lately by someone else shows and finds nothing to search.
pub const RECHECK_EVERY: u32 = 5;
/// The walker says it is still searching after this many empty searches, and each this many.
pub const NARRATE_EVERY: u32 = 10;
/// "No exit" answers in one walk: the first relocates (look, then replan), the second stops.
pub const MAX_RELOCATIONS: u32 = 2;
/// Routes planned again in one walk before it gives up (the prototype's five attempts).
pub const MAX_REPLANS: u32 = 5;

/// A line for the dev panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Say {
    Notice(String),
    Warning(String),
}

/// What the walker wants done after a call.
#[derive(Debug, Default)]
pub(crate) struct Next {
    /// The action to submit to the executor.
    pub action: Option<Action>,
    pub says: Vec<Say>,
    /// The route started, changed or ended.
    pub route_changed: bool,
    /// The walk ended in its destination: this room.
    pub arrived: Option<Key>,
    /// A move to send ahead of `action` or of the move in flight, if the session may
    /// (pipelining): it says so with [`Walker::went_ahead`].
    pub ahead: Option<Action>,
}

impl Next {
    fn notice(&mut self, text: impl Into<String>) {
        self.says.push(Say::Notice(text.into()));
    }

    fn warning(&mut self, text: impl Into<String>) {
        self.says.push(Say::Warning(text.into()));
    }
}

/// What the action in flight is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Doing {
    Move,
    Open,
    Bash,
    /// `use <key>` at a Key exit the character holds the key of.
    UseKey,
    Search,
    /// A phrase that takes a block off a blocked Hidden exit (`Step::unblock`).
    Unblock,
    /// `look` after a search that found nothing, or a blocked exit's phrases: is the exit
    /// showing?
    Recheck,
    Look,
    /// `look` after a room command or a crossing's phrase (C-055): a teleport shows no room
    /// ("You find yourself...elsewhere."), so where it went is looked at, and checked as a
    /// move's room is.
    Arrive,
    /// `buy <item>` in the shop the route passes, for an exit ahead that needs it.
    Buy,
}

#[derive(Debug)]
struct Walk {
    to: Key,
    name: String,
    route: Route,
    /// The step of `route` in progress.
    next: usize,
    doing: Doing,
    /// Moves confirmed over the whole walk.
    confirmed: u32,
    bashes: u32,
    /// `use <key>` commands at the step in progress, and the key's name.
    key_tries: u32,
    key: Option<String>,
    /// `search` commands at the step in progress that found nothing.
    searches: u32,
    /// The step in progress's hidden exit was found by a search: move.
    searched: bool,
    /// The phrases of the step in progress's blocked hidden exit typed so far.
    typed: usize,
    relocations: u32,
    replans: u32,
    /// Rooms that were not the one the route expected.
    drifts: u32,
    /// Stop when the action in flight is done, and why.
    stopping: Option<String>,
    /// Moves may be pipelined on this walk.
    pipelined: bool,
    /// The move of step `next + 1` went ahead of the one in flight.
    ahead: bool,
    /// The step before the move ahead went wrong: the move ahead's answer is awaited, then the
    /// walk plans again (`Ok`, why) or ends (`Err`, why).
    void: Option<Result<String, String>>,
    /// The boss lair this walk stops one room short of: `to` is the room before it.
    short_of: Option<Lair>,
    /// An item to buy on the way, for an exit ahead that needs it.
    purchase: Option<Purchase>,
    /// What the shop said to the `buy` in flight: bought, or refused and why.
    heard_buy: Option<Result<(), &'static str>>,
}

/// A boss lair a walk stops short of.
#[derive(Debug, Clone)]
struct Lair {
    key: Key,
    name: String,
    bosses: Vec<String>,
}

impl Lair {
    /// "Large Wooden Room (boss: minotaur chieftain)".
    fn describe(&self) -> String {
        if self.bosses.is_empty() {
            format!("{} (a boss lair)", self.name)
        } else {
            format!("{} (boss: {})", self.name, self.bosses.join(", "))
        }
    }
}

#[derive(Debug)]
pub(crate) struct Walker {
    walk: Option<Walk>,
    /// Searches for one hidden exit before it is given up (`movement.search_limit`).
    search_limit: u32,
    /// A `walk_to` that came while an action of the walk it replaces was in flight, and whether
    /// it stops short of a boss lair.
    then: Option<(Key, bool)>,
}

impl Default for Walker {
    fn default() -> Self {
        Self::with_search_limit(MAX_SEARCHES)
    }
}

impl Walker {
    /// Search for a hidden exit up to `limit` times from now on (a walk in progress goes on).
    pub fn set_search_limit(&mut self, limit: u32) {
        self.search_limit = limit;
    }

    /// A walker that searches for a hidden exit up to `limit` times.
    pub fn with_search_limit(limit: u32) -> Self {
        Self {
            walk: None,
            search_limit: limit,
            then: None,
        }
    }
}

fn room_name(atlas: &Atlas, k: Key) -> String {
    atlas
        .graph
        .room(k)
        .map_or_else(|| format!("{}/{}", k.0, k.1), |r| r.name.clone())
}

/// `use <key>` at one Key exit before the walker gives it up.
const MAX_KEY_TRIES: u32 = 2;

/// The name of the key that turns the Key exit of `step`, when the character holds it.
fn key_for(pos: &Position, atlas: &Atlas, step: &Step) -> Option<String> {
    let e = atlas
        .graph
        .edges(step.from)
        .iter()
        .find(|e| e.dir == step.dir)?;
    match e.gate {
        wayfarer_world::Gate::Key { item } if pos.traveller.keys.contains(&item) => {
            atlas.item_names.get(&item).cloned()
        }
        _ => None,
    }
}

/// Why the locked door of `step` is beyond the character, if its odds are nil: "The door east
/// is beyond my strength (needs more than 250)".
fn hopeless(pos: &Position, atlas: &Atlas, step: &Step) -> Option<String> {
    let e = atlas
        .graph
        .edges(step.from)
        .iter()
        .find(|e| e.dir == step.dir && e.door.is_some())?;
    (pos.traveller.lock_odds(e) == Some(0)).then(|| {
        let what = if e.kind == ExitType::Gate {
            "gate"
        } else {
            "door"
        };
        format!(
            "The {what} {} is beyond my strength (needs more than {})",
            dir_name(step.dir),
            -e.params[1]
        )
    })
}

fn plural(n: usize, one: &str) -> String {
    match n {
        1 => format!("one {one}"),
        n => format!("{n} {one}s"),
    }
}

/// "6 steps, one door".
pub(crate) fn describe(route: &Route) -> String {
    let doors = route.iter().filter(|s| s.door.is_some()).count();
    let mut s = plural(route.len(), "step");
    if doors > 0 {
        s = format!("{s}, {}", plural(doors, "door"));
    }
    s
}

/// "6 steps, buying a room ticket at The Homely Hearth, Common Room".
fn describe_buying(atlas: &Atlas, route: &Route, purchase: Option<&Purchase>) -> String {
    match purchase {
        Some(p) => format!(
            "{}, buying a {} at {}",
            describe(route),
            p.name,
            room_name(atlas, p.shop)
        ),
        None => describe(route),
    }
}

/// Why a shop would not sell: `BuyRefused`'s reason in words.
fn refusal_words(why: &str) -> &'static str {
    match why {
        "poor" => "cannot afford it",
        "not_here" => "not sold here now",
        _ => "unknown to the shop",
    }
}

impl Walker {
    pub fn walking(&self) -> bool {
        self.walk.is_some()
    }

    /// A game event: what the shop says to the walk's `buy`.
    pub fn heard(&mut self, ev: &GameEvent) {
        let Some(w) = &mut self.walk else {
            return;
        };
        if w.doing != Doing::Buy {
            return;
        }
        match ev {
            GameEvent::Bought { .. } => w.heard_buy = Some(Ok(())),
            GameEvent::BuyRefused { why } => w.heard_buy = Some(Err(why)),
            _ => {}
        }
    }

    /// The move [`Next::ahead`] proposed went to the executor.
    pub fn went_ahead(&mut self) {
        if let Some(w) = &mut self.walk {
            w.ahead = true;
        }
    }

    /// The move ahead was dropped before it went (combat started).
    pub fn ahead_dropped(&mut self) {
        if let Some(w) = &mut self.walk {
            w.ahead = false;
        }
    }

    /// The wire's `walking`.
    pub fn snapshot(&self) -> Option<Walking> {
        let w = self.walk.as_ref()?;
        Some(Walking {
            to: RoomKey {
                map: w.to.0,
                room: w.to.1,
            },
            name: w.name.clone(),
            steps_left: u32::try_from(w.route.len() - w.next).unwrap_or(u32::MAX),
            step: w.confirmed + 1,
            commands: w.route[w.next..]
                .iter()
                .map(|s| s.command.clone())
                .collect(),
            short_of: w.short_of.as_ref().map(|l| BossLair {
                map: l.key.0,
                room: l.key.1,
                name: l.name.clone(),
                bosses: l.bosses.clone(),
            }),
        })
    }

    /// The locked door the walk is bashing: its room and direction, while it is at it.
    pub fn bashing(&self) -> Option<(Key, u8)> {
        let w = self.walk.as_ref()?;
        let step = w.route.get(w.next)?;
        (w.doing == Doing::Bash).then_some((step.from, step.dir))
    }

    /// Where the walk in progress goes.
    pub fn destination(&self) -> Option<Key> {
        self.walk.as_ref().map(|w| w.to)
    }

    /// The rooms still to pass, from the one the step in progress leaves.
    pub fn route_keys(&self) -> Option<Vec<Key>> {
        let w = self.walk.as_ref()?;
        let rest = &w.route[w.next..];
        let first = rest.first()?.from;
        Some(
            std::iter::once(first)
                .chain(rest.iter().map(|s| s.to))
                .collect(),
        )
    }

    /// A walk to `to`. `busy`: an action of the walk in progress is in flight; the new walk
    /// starts when it is done. It pipelines the moves when `pipelined`, and stops one room
    /// short of `to` when `short` and `to` is a boss lair (docs/brain.md "Go to").
    pub fn start_with(
        &mut self,
        pos: &mut Position,
        atlas: &Atlas,
        to: Key,
        busy: bool,
        pipelined: bool,
        short: bool,
    ) -> Next {
        let mut next = Next::default();
        if !atlas.graph.contains(to) {
            next.warning(format!("walk: no room {}/{} in the world pack", to.0, to.1));
            return next;
        }
        if let Some(w) = &mut self.walk {
            if busy {
                w.stopping = Some(format!("walking to {} instead", room_name(atlas, to)));
                self.then = Some((to, short));
                return next;
            }
            self.walk = None;
            next.route_changed = true;
        }
        let name = room_name(atlas, to);
        let Some(from) = pos.best() else {
            next.warning(format!("Not walking to {name}: the position is unknown"));
            return next;
        };
        let Some((mut r, mut purchase)) = route_buying(
            &atlas.graph,
            from,
            to,
            &pos.traveller,
            &atlas.exit_items,
            |_| true,
        ) else {
            next.warning(format!(
                "Not walking to {name}: no way there from {}",
                room_name(atlas, from)
            ));
            return next;
        };
        if r.is_empty() {
            next.notice(format!("Arrived: {name}"));
            next.arrived = Some(to);
            return next;
        }
        // Stop-short is a route cut: the destination becomes the room before the lair.
        let lair = short
            .then(|| atlas.finder.boss_lair(to, &atlas.monsters))
            .flatten()
            .map(|bosses| Lair {
                key: to,
                name: name.clone(),
                bosses: bosses.iter().map(|m| m.name.clone()).collect(),
            });
        let (to, name) = match &lair {
            Some(lair) => {
                let before = r.pop().expect("a route of a step or more").from;
                // The exit the item was for may have been the lair's.
                purchase = purchase.filter(|p| p.at < r.len());
                if r.is_empty() {
                    next.notice(format!("Stopped one room short of {}", lair.describe()));
                    next.arrived = Some(before);
                    return next;
                }
                next.notice(format!(
                    "Walking to {}: {}, stopping one room short",
                    lair.name,
                    describe_buying(atlas, &r, purchase.as_ref())
                ));
                (before, room_name(atlas, before))
            }
            None => {
                next.notice(format!(
                    "Walking to {name}: {}",
                    describe_buying(atlas, &r, purchase.as_ref())
                ));
                (to, name)
            }
        };
        let mut w = Walk {
            short_of: lair,
            to,
            name,
            route: r,
            next: 0,
            doing: Doing::Move,
            confirmed: 0,
            bashes: 0,
            key_tries: 0,
            key: None,
            searches: 0,
            searched: false,
            typed: 0,
            relocations: 0,
            replans: 0,
            drifts: 0,
            stopping: None,
            pipelined,
            ahead: false,
            void: None,
            purchase,
            heard_buy: None,
        };
        let doing = w.first(pos, &mut next);
        next.action = Some(w.send(doing));
        next.ahead = w.propose_ahead(pos);
        next.route_changed = true;
        self.walk = Some(w);
        next
    }

    /// Stop after the move in flight (`walk_stop`, combat, a typed line). `busy`: an action of
    /// the walk is queued or in flight.
    pub fn stop(&mut self, why: &str, busy: bool) -> Next {
        let mut next = Next::default();
        self.then = None;
        match &mut self.walk {
            None => {}
            Some(w) if busy => {
                if w.stopping.is_none() {
                    w.stopping = Some(why.to_owned());
                }
            }
            Some(_) => self.end(&mut next, Say::Notice(format!("Stopped walking: {why}"))),
        }
        next
    }

    /// Stop at once: the action in flight will never be answered (the session left the realm).
    pub fn abandon(&mut self, why: &str) -> Next {
        let mut next = Next::default();
        self.then = None;
        if self.walk.is_some() {
            self.end(&mut next, Say::Notice(format!("Stopped walking: {why}")));
        }
        next
    }

    fn end(&mut self, next: &mut Next, say: Say) {
        self.walk = None;
        next.says.push(say);
        next.route_changed = true;
    }

    /// The walk's action is done. The position has already read the room it brought, if any;
    /// `seen` is the belief as that room left it (with a move ahead, the position may have
    /// read that move's room too since).
    pub fn outcome(
        &mut self,
        pos: &mut Position,
        atlas: &Atlas,
        action: &Action,
        outcome: Outcome,
        seen: Option<&Belief>,
    ) -> Next {
        let mut next = Next::default();
        let search_limit = self.search_limit;
        let Some(w) = &mut self.walk else {
            return next;
        };
        if let Some(void) = w.void.take() {
            // The move that went ahead of a step that went wrong: the belief has read its room
            // (or its refusal), so plan from wherever that is.
            let result = match (void, outcome) {
                (Err(why), _) => Err(why),
                (Ok(_), Outcome::TimedOut) => Err(format!("no answer to `{}`", action.command)),
                (Ok(why), _) => w.replan(pos, atlas, &mut next, &why),
            };
            return self.after(pos, atlas, result, next);
        }
        let doing = w.doing;
        let step = w.route[w.next].clone();
        let mut advanced = false;
        let result = match (doing, outcome) {
            (_, Outcome::TimedOut) => Err(format!("no answer to `{}`", action.command)),
            // The phrase went and its prompt came: look where it took the character.
            (Doing::Move, Outcome::Confirmed(Confirmed::Prompt)) if step.exit_type.is_command() => {
                Ok(Doing::Arrive)
            }
            (Doing::Move | Doing::Arrive, Outcome::Confirmed(Confirmed::Room)) => {
                if seen.unwrap_or(&pos.belief).contains(step.to) {
                    // A one-charge item (a room ticket) stays in the exit's box.
                    if let Some(Gate::Item { item }) =
                        atlas.graph.edge(step.from, step.dir).map(|e| e.gate)
                        && atlas.exit_items.get(&item).is_some_and(|i| i.used_up())
                    {
                        pos.traveller.items.remove(&item);
                        pos.traveller.keys.remove(&item);
                    }
                    w.next += 1;
                    w.confirmed += 1;
                    w.bashes = 0;
                    w.key_tries = 0;
                    w.searches = 0;
                    w.searched = false;
                    w.typed = 0;
                    advanced = true;
                    if w.next == w.route.len() {
                        let (arrival, to) = (w.arrival(), w.to);
                        self.end(&mut next, Say::Notice(arrival));
                        next.arrived = Some(to);
                        return self.then_start(pos, atlas, next);
                    }
                    Ok(Doing::Move)
                } else {
                    w.drifts += 1;
                    let seen = pos.title.clone().unwrap_or_else(|| "unknown".to_owned());
                    if w.drifts > 1 {
                        Err(format!(
                            "lost the way: expected {}, the room was {seen}",
                            room_name(atlas, step.to)
                        ))
                    } else {
                        w.replan(pos, atlas, &mut next, &format!("the room was {seen}"))
                    }
                }
            }
            // A Key exit the character holds the key of is turned with `use <key>`, twice at most.
            (Doing::Move, Outcome::Refused(Refused::DoorClosed { .. }))
                if step.exit_type == ExitType::Key
                    && w.key_tries < MAX_KEY_TRIES
                    && key_for(pos, atlas, &step).is_some() =>
            {
                w.key_tries += 1;
                w.key = key_for(pos, atlas, &step);
                Ok(Doing::UseKey)
            }
            (Doing::UseKey, Outcome::Confirmed(Confirmed::Prompt)) => Ok(Doing::Move),
            (Doing::Buy, Outcome::Confirmed(_)) => {
                let p = w.purchase.take().expect("buying");
                match w.heard_buy.take() {
                    Some(Ok(())) => {
                        pos.traveller.items.insert(p.item);
                        pos.traveller.keys.insert(p.item);
                        Ok(Doing::Move)
                    }
                    Some(Err(why)) => Err(format!(
                        "could not buy a {}: {}",
                        p.name,
                        refusal_words(why)
                    )),
                    None => Err(format!("could not buy a {}: no answer", p.name)),
                }
            }
            (Doing::Move, Outcome::Refused(Refused::DoorClosed { locked: false })) => {
                Ok(Doing::Open)
            }
            (Doing::Move, Outcome::Refused(Refused::DoorClosed { locked: true }))
            | (Doing::Open, Outcome::Refused(Refused::Locked)) => match hopeless(pos, atlas, &step)
            {
                // No `bash` at a door the character can never open.
                Some(why) => {
                    next.notice(why);
                    pos.block(step.from, step.dir);
                    let door = format!("the door {} cannot be opened", dir_name(step.dir));
                    w.replan(pos, atlas, &mut next, &door)
                }
                None => Ok(Doing::Bash),
            },
            (Doing::Move, Outcome::Refused(Refused::Move(Refusal::NoExit))) => {
                // Pitfall 34: the believed position is wrong. Look, and plan again from
                // where the game says the character is.
                w.relocations += 1;
                if w.relocations >= MAX_RELOCATIONS {
                    Err(format!("no exit `{}` twice: lost", step.command))
                } else {
                    next.notice(format!("No exit `{}`: looking to relocate", step.command));
                    Ok(Doing::Look)
                }
            }
            (Doing::Move, Outcome::Refused(Refused::Move(Refusal::Combat))) => {
                Err("in combat".to_owned())
            }
            (Doing::Move, Outcome::Refused(Refused::Move(reason))) => {
                pos.refused(atlas, &step.command);
                w.replan(
                    pos,
                    atlas,
                    &mut next,
                    &format!("`{}` refused: {}", step.command, reason.describe()),
                )
            }
            (Doing::Open, Outcome::Confirmed(Confirmed::DoorOpened))
            | (Doing::Bash, Outcome::Confirmed(Confirmed::Bash { ok: true })) => Ok(Doing::Move),
            (Doing::Bash, Outcome::Confirmed(Confirmed::Bash { ok: false })) => {
                if w.bashes < MAX_BASHES {
                    Ok(Doing::Bash)
                } else {
                    pos.block(step.from, step.dir);
                    w.replan(pos, atlas, &mut next, "the door would not bash open")
                }
            }
            (Doing::Search, Outcome::Confirmed(Confirmed::Search { found: true })) => {
                w.searched = true;
                Ok(Doing::Move)
            }
            // No search without light (`kb/recovery.md` "Light and darkness").
            (Doing::Search, Outcome::Refused(Refused::TooDark)) => Err(format!(
                "too dark to search for the hidden exit {}",
                dir_name(step.dir)
            )),
            (Doing::Search, Outcome::Confirmed(Confirmed::Search { found: false })) => {
                w.searches += 1;
                if w.searches < search_limit {
                    if w.searches % NARRATE_EVERY == 0 {
                        next.notice(format!(
                            "Still searching for the hidden exit {} ({} tries)",
                            dir_name(step.dir),
                            w.searches
                        ));
                    }
                    if w.searches % RECHECK_EVERY == 1 {
                        // An exit found lately (by anyone) needs no search and finds nothing:
                        // `look` to see whether it shows.
                        Ok(Doing::Recheck)
                    } else {
                        Ok(Doing::Search)
                    }
                } else {
                    pos.block(step.from, step.dir);
                    let why = format!("no hidden exit {} found", dir_name(step.dir));
                    w.replan(pos, atlas, &mut next, &why)
                }
            }
            (Doing::Unblock, Outcome::Confirmed(Confirmed::Prompt)) => {
                if w.typed < step.unblock.len() {
                    Ok(Doing::Unblock)
                } else {
                    Ok(Doing::Recheck)
                }
            }
            // A blocked exit its phrases left shut: not tried again this walk.
            (Doing::Recheck, Outcome::Confirmed(Confirmed::Room))
                if !step.unblock.is_empty() && !pos.showing(step.from, step.dir) =>
            {
                pos.block(step.from, step.dir);
                let why = format!("the hidden exit {} did not open", dir_name(step.dir));
                w.replan(pos, atlas, &mut next, &why)
            }
            // Showing: move; else search again (`after`, `Walk::first`).
            (Doing::Recheck, Outcome::Confirmed(Confirmed::Room)) => Ok(Doing::Move),
            (Doing::Look, Outcome::Confirmed(Confirmed::Room)) => {
                w.replan(pos, atlas, &mut next, "relocated")
            }
            (_, other) => Err(format!("`{}`: unexpected {other:?}", action.command)),
        };
        let w = self.walk.as_mut().expect("still walking");
        if w.ahead {
            w.ahead = false;
            if advanced {
                // The move ahead is the move of this step, in flight already; perhaps the next
                // goes ahead of it.
                w.doing = Doing::Move;
                next.ahead = w.propose_ahead(pos);
            } else {
                // It went after a step that went wrong: wait for its answer.
                w.void = Some(result.map(|_| "a move went ahead of a wrong step".to_owned()));
            }
            return next;
        }
        self.after(pos, atlas, result, next)
    }

    /// Go on from an outcome's `result`: stop the walk, arrive, or send the next action.
    fn after(
        &mut self,
        pos: &mut Position,
        atlas: &Atlas,
        result: Result<Doing, String>,
        mut next: Next,
    ) -> Next {
        let w = self.walk.as_mut().expect("still walking");
        match result {
            Err(why) => {
                self.end(&mut next, Say::Warning(format!("Stopped walking: {why}")));
                self.then_start(pos, atlas, next)
            }
            Ok(_) if w.stopping.is_some() => {
                let why = w.stopping.take().expect("stopping");
                self.end(&mut next, Say::Notice(format!("Stopped walking: {why}")));
                self.then_start(pos, atlas, next)
            }
            Ok(_) if w.next == w.route.len() => {
                // Replanned from the destination itself.
                let (arrival, to) = (w.arrival(), w.to);
                self.end(&mut next, Say::Notice(arrival));
                next.arrived = Some(to);
                next
            }
            Ok(doing) => {
                let doing = match doing {
                    Doing::Move => w.first(pos, &mut next),
                    other => other,
                };
                next.action = Some(w.send(doing));
                next.ahead = w.propose_ahead(pos);
                next
            }
        }
    }

    /// Start the walk a `walk_to` asked for while the last one was finishing. Only the new
    /// walk's arrival counts: the old one was replaced.
    fn then_start(&mut self, pos: &mut Position, atlas: &Atlas, mut next: Next) -> Next {
        if let Some((to, short)) = self.then.take() {
            let started = self.start_with(pos, atlas, to, false, false, short);
            next.action = started.action;
            next.says.extend(started.says);
            next.route_changed = true;
            next.arrived = started.arrived;
        }
        next
    }
}

impl Walk {
    /// What the walk says when it ends in its destination.
    fn arrival(&self) -> String {
        match &self.short_of {
            Some(lair) => format!("Stopped one room short of {}", lair.describe()),
            None => format!("Arrived: {}", self.name),
        }
    }

    /// What to do first at the step in progress: search for its hidden exit when it is not
    /// showing (not in the room's exits line, not found lately), else move.
    fn first(&self, pos: &Position, next: &mut Next) -> Doing {
        if self.purchase.as_ref().is_some_and(|p| p.at == self.next) {
            return Doing::Buy;
        }
        let s = &self.route[self.next];
        if s.exit_type != ExitType::Hidden || self.searched || pos.showing(s.from, s.dir) {
            return Doing::Move;
        }
        // Blocked, not hidden: no search finds it, its room's phrases open it.
        if !s.unblock.is_empty() {
            if self.typed == 0 {
                next.notice(format!(
                    "Opening the hidden exit {}: {}",
                    dir_name(s.dir),
                    s.unblock.join(", ")
                ));
            }
            return Doing::Unblock;
        }
        if self.searches == 0 {
            next.notice(format!("Searching for the hidden exit {}", dir_name(s.dir)));
        }
        Doing::Search
    }

    /// The move of the step after the one in progress, to go ahead of it: on a pipelined walk,
    /// with the belief sure of the room, both steps plain moves (a direction, no door), the step
    /// in progress a move, and the walk not stopping.
    fn propose_ahead(&self, pos: &Position) -> Option<Action> {
        let plain = |i: usize| {
            self.route.get(i).filter(|s| {
                s.door.is_none()
                    && s.exit_type != ExitType::Hidden
                    && mm::direction(&s.command).is_some()
            })
        };
        if !self.pipelined
            || self.ahead
            || self.void.is_some()
            || self.stopping.is_some()
            || self.doing != Doing::Move
            || !pos.belief.certain()
            || self
                .purchase
                .as_ref()
                .is_some_and(|p| p.at <= self.next + 1)
        {
            return None;
        }
        plain(self.next)?;
        let step = plain(self.next + 1)?;
        Some(Action::new(step.command.clone(), Expect::RoomSeen))
    }

    /// The action for `doing` at the step in progress.
    fn send(&mut self, doing: Doing) -> Action {
        self.doing = doing;
        if doing == Doing::Unblock {
            let phrase = self.route[self.next].unblock[self.typed].clone();
            self.typed += 1;
            return Action::new(phrase, Expect::Prompt);
        }
        let step = &self.route[self.next];
        // A room command or a crossing has no direction; only doors are opened by one.
        let dir = mm::DIRS
            .get(usize::from(step.dir))
            .copied()
            .unwrap_or_default();
        match doing {
            // A room command or a crossing: its prompt, then a look (`Doing::Arrive`).
            Doing::Move if step.exit_type.is_command() => {
                Action::new(step.command.clone(), Expect::Prompt)
            }
            Doing::Move => Action::new(step.command.clone(), Expect::RoomSeen),
            Doing::Arrive => Action::new("look", Expect::RoomSeen),
            Doing::Open => Action::new(format!("open {dir}"), Expect::DoorOpened),
            Doing::UseKey => Action::new(
                format!("use {}", self.key.clone().unwrap_or_default()),
                Expect::Prompt,
            ),
            Doing::Bash => {
                self.bashes += 1;
                Action::new(format!("bash {dir}"), Expect::BashResult)
            }
            Doing::Search => Action::new(format!("search {dir}"), Expect::SearchResult),
            Doing::Unblock => unreachable!("sent above"),
            Doing::Buy => {
                self.heard_buy = None;
                let name = self.purchase.as_ref().map_or("", |p| p.name.as_str());
                Action::new(format!("buy {name}"), Expect::Prompt)
            }
            // `look`, not an empty line: an empty line redisplays the room only in statline
            // FULL modes (`kb/screen-parsing.md`).
            Doing::Look | Doing::Recheck => Action::new("look", Expect::RoomSeen),
        }
    }

    /// Plan again from the believed room. `Ok(Move)` with the new route in place (empty when
    /// already there), or why not.
    fn replan(
        &mut self,
        pos: &Position,
        atlas: &Atlas,
        next: &mut Next,
        why: &str,
    ) -> Result<Doing, String> {
        self.replans += 1;
        if self.replans > MAX_REPLANS {
            return Err(format!("{why}; gave up after {MAX_REPLANS} replans"));
        }
        let Some(from) = pos.best() else {
            // In a room the map lacks (a player's house): say which.
            return Err(match pos.title.as_deref() {
                Some(t) if atlas.graph.rooms_named(t).is_empty() => {
                    format!("{why}; in {t}, a room not on the map")
                }
                _ => format!("{why}; the position is unknown"),
            });
        };
        // A walk stopping short of a boss lair never plans through it.
        let lair = self.short_of.as_ref().map(|l| l.key);
        let r = route_buying(
            &atlas.graph,
            from,
            self.to,
            &pos.traveller,
            &atlas.exit_items,
            |k| Some(k) != lair,
        );
        let Some((r, purchase)) = r else {
            return Err(format!(
                "{why}; no way to {} from {}",
                self.name,
                room_name(atlas, from)
            ));
        };
        self.bashes = 0;
        self.searches = 0;
        self.searched = false;
        self.typed = 0;
        next.route_changed = true;
        if !r.is_empty() {
            next.notice(format!(
                "Replanned ({why}): {}",
                describe_buying(atlas, &r, purchase.as_ref())
            ));
        }
        self.route = r;
        self.next = 0;
        self.purchase = purchase;
        Ok(Doing::Move)
    }
}
