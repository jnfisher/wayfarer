//! One session as the brain follows it: its parser, its character, the catch-up on its output,
//! the re-sync, and with a world pack its position, the executor, the walker and the bot. The
//! re-sync's commands and the walker's and the bot's actions are all it sends to the game.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::Instant;
use wayfarer_parse::typed::GameEvent;
use wayfarer_parse::{Event, Parser, Registry};
use wayfarer_proto::{
    BotAction, CharacterState, Chunk, SessionId, SessionInfo, SessionState, ToGateway,
};
use wayfarer_world::Key;

use crate::brain::Atlas;
use crate::character::{Character, merge_patch};
use crate::executor::Executor;
use crate::modes::{Bot, Do, Presence, View};
use crate::policy::{COMBAT_QUIET, hp_percent};
use crate::position::Position;
use crate::walker::{Next, Say, Walker};

/// The rooms a `map_view` holds unless a `map_request` asks for another radius: those within
/// this many steps.
pub const MAP_RADIUS: u32 = 15;

/// The re-sync's commands, in order: the "talk and look" kind (docs/plan/03-engine.md
/// section 6). `look`, not an empty line: an empty line shows the room only in statline FULL
/// modes (`kb/screen-parsing.md`).
pub const RESYNC_COMMANDS: [&str; 4] = ["st", "exp", "i", "look"];
/// How long nothing must have been sent to the game for a prompt to count as idle.
pub const IDLE: Duration = Duration::from_secs(2);
/// How long a re-sync command waits for its answer before the next one goes.
pub const ANSWER_TIMEOUT: Duration = Duration::from_secs(5);
/// How long a line the brain sent waits for the gateway's `sent`. The gateway sends it before
/// the line's echo, so it is normally back within a round trip; one still missing after this
/// was lost (a lagged connection) and is forgotten, so a later line the player types is not
/// taken for the brain's.
pub const OWN_PATIENCE: Duration = Duration::from_secs(10);
/// Chunks asked for per `history_request`.
pub const HISTORY_CHUNKS: u32 = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Resync {
    Off,
    /// The command `next` goes at the next prompt; with `idle`, at the next idle prompt.
    Want {
        next: usize,
        idle: bool,
    },
    /// Command `step` went at `sent`; its answer has been seen or not. `typed`: the player
    /// sent a line since, so the next command waits for an idle prompt.
    Waiting {
        step: usize,
        deadline: Instant,
        answered: bool,
        typed: bool,
    },
}

/// Whose action is in the executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    Walker,
    Bot,
}

/// What a re-sync step did, for the brain to report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Progress {
    None,
    Done,
}

/// Whether `ev` answers re-sync command `step`.
fn answers(step: usize, ev: &GameEvent) -> bool {
    use GameEvent as G;
    match step {
        0 => matches!(
            ev,
            G::StatusName { .. }
                | G::StatusRace { .. }
                | G::StatusClass { .. }
                | G::StatusHits { .. }
                | G::StatusMana { .. }
                | G::StatusStat { .. }
        ),
        1 => matches!(ev, G::ExpScreen { .. }),
        2 => matches!(
            ev,
            G::Inventory { .. } | G::Wealth { .. } | G::Encumbrance { .. }
        ),
        _ => matches!(ev, G::RoomTitle { .. } | G::RoomExits { .. } | G::RoomDark),
    }
}

/// Whether `text` shows anything but escapes and white space.
fn visible(text: &str) -> bool {
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.next() == Some('[') {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
        } else if !c.is_whitespace() {
            return true;
        }
    }
    false
}

pub(crate) struct Session {
    pub id: SessionId,
    pub state: SessionState,
    registry: Arc<Registry>,
    parser: Parser,
    pub character: Character,
    /// The last snapshot the UIs were sent in this attach, as JSON; `None` before the whole
    /// snapshot that ends the catch-up.
    published: Option<serde_json::Value>,
    /// The last output `seq` fed to the parser.
    fed: u64,
    /// Chunks up to this `seq` predate the brain: fed without `event` messages.
    silent_until: u64,
    /// A `history_request` is out; live output waits here meanwhile.
    fetching: bool,
    buffer: Vec<Chunk>,
    /// The last chunk ended in a prompt.
    at_prompt: bool,
    /// When a line last went to the game (or the brain attached).
    last_sent: Instant,
    /// The brain's own lines and when each went, until the gateway's `sent` for each comes
    /// back or [`OWN_PATIENCE`] runs out.
    own: VecDeque<(String, Instant)>,
    pub resync: Resync,
    /// A re-sync finished since the brain attached, and the session stayed in the realm.
    pub synced: bool,
    /// The world pack's rooms: without them there is no position, walking or map.
    atlas: Option<Arc<Atlas>>,
    position: Position,
    executor: Executor,
    walker: Walker,
    /// How far a `map_view` reaches.
    map_radius: u32,
    /// The center of the last `map_view` sent in this attach.
    map_center: Option<Key>,
    /// The route changed since the last `map_view`.
    map_dirty: bool,
    /// Whose action the executor holds.
    owner: Option<Owner>,
    bot: Bot,
    /// Who is in the room, for the bot.
    presence: Presence,
    /// The walk in progress is the bot's.
    bot_walk: bool,
    /// When the character's own auto-combat last showed (`*Combat Engaged*`, its blows), or
    /// the brain attached: for how long it has been quiet, since auto-combat can end without a
    /// word (`kb/pitfalls.md` 42).
    last_combat: Instant,
}

/// The events of the character's own auto-combat: its engaging and its blows, hits, misses,
/// glances and dodged swings. A monster's blows are not among them: a monster attacks a
/// character that is not fighting back (`kb/combat.md`, "engage at once").
const OWN_COMBAT: [&str; 7] = [
    "combat_engaged",
    "you_hit",
    "you_miss",
    "you_glance",
    "you_dodged",
    "pvp_dodges",
    "smashed_it",
];

impl Session {
    /// A session the brain has just learned of: its history up to now is fed silently.
    pub fn new(
        info: &SessionInfo,
        registry: Arc<Registry>,
        atlas: Option<Arc<Atlas>>,
        now: Instant,
    ) -> Self {
        Self {
            atlas,
            position: Position::default(),
            executor: Executor::default(),
            walker: Walker::default(),
            map_radius: MAP_RADIUS,
            map_center: None,
            map_dirty: false,
            owner: None,
            bot: Bot::default(),
            presence: Presence::default(),
            bot_walk: false,
            last_combat: now,
            id: info.id.clone(),
            state: info.state,
            parser: Parser::new(registry.clone()),
            registry,
            character: Character::new(),
            published: None,
            fed: 0,
            silent_until: info.last_seq,
            fetching: false,
            buffer: Vec::new(),
            at_prompt: false,
            last_sent: now,
            own: VecDeque::new(),
            resync: Resync::Off,
            synced: false,
        }
    }

    /// The brain (re)attached: catch up on the output from where it left off, then send a
    /// whole snapshot. `info` is the gateway's view of the session now.
    pub fn attach(&mut self, info: &SessionInfo, now: Instant, out: &mut Vec<ToGateway>) {
        self.published = None;
        self.map_center = None;
        self.own.clear();
        self.last_sent = now;
        self.buffer.clear();
        self.last_combat = now;
        // An answer to the action in flight may have gone by unseen.
        self.cancel(out);
        let next = self.walker.abandon("the brain re-attached");
        self.apply(next, out);
        if info.last_seq < self.fed {
            // The gateway's log started again: what was fed is not this log's.
            tracing::warn!(session = %self.id, "the output log restarted; parsing afresh");
            self.parser = Parser::new(self.registry.clone());
            self.character.parser_reset();
            self.fed = 0;
            // The new log's output so far went by before this attach, like a new session's.
            self.silent_until = info.last_seq;
            self.synced = false;
        }
        if let Resync::Waiting { step, .. } = self.resync {
            // The command's answer may have gone by; ask again.
            self.resync = Resync::Want {
                next: step,
                idle: true,
            };
        }
        self.set_state(info.state, out);
        self.fetch(out);
    }

    fn fetch(&mut self, out: &mut Vec<ToGateway>) {
        self.fetching = true;
        out.push(ToGateway::HistoryRequest {
            session: self.id.clone(),
            after_seq: (self.fed > 0).then_some(self.fed),
            before_seq: None,
            max_chunks: HISTORY_CHUNKS,
        });
    }

    pub fn set_state(&mut self, state: SessionState, out: &mut Vec<ToGateway>) {
        let was = self.state;
        self.state = state;
        if state != SessionState::Realm {
            self.synced = false;
            // Nothing in flight will be answered now.
            self.cancel(out);
            let says = self.bot.stop("the session left the realm", true);
            if !says.is_empty() {
                self.say(says, out);
                self.publish_patch(out);
            }
            if self.walker.walking() {
                let next = self.walker.abandon("the session left the realm");
                self.apply(next, out);
                self.flush_views(out);
            }
            if self.resync != Resync::Off {
                // Cut short: start again from `st` in the realm.
                self.request_resync();
            }
        } else if was != SessionState::Realm {
            self.synced = false;
        }
        if state == SessionState::Realm && !self.synced && self.resync == Resync::Off {
            self.request_resync();
        }
    }

    /// Start a re-sync from the first command (at the next idle prompt in the realm).
    pub fn request_resync(&mut self) {
        self.resync = Resync::Want {
            next: 0,
            idle: true,
        };
    }

    /// Live output.
    pub fn output(&mut self, chunk: Chunk, out: &mut Vec<ToGateway>) -> bool {
        if self.fetching {
            self.buffer.push(chunk);
            return false;
        }
        if chunk.seq <= self.fed {
            return false;
        }
        if chunk.seq > self.fed + 1 {
            self.buffer.push(chunk);
            self.fetch(out);
            return false;
        }
        self.feed(chunk, out);
        true
    }

    /// The connection lagged: fetch what it missed.
    pub fn lagged(&mut self, out: &mut Vec<ToGateway>) {
        // The `sent` for a line of ours may have been among what was missed.
        self.own.clear();
        if !self.fetching {
            self.fetch(out);
        }
    }

    /// The answer to a `history_request`. Whether the catch-up finished (the caller then sends
    /// the whole snapshot).
    pub fn history(&mut self, chunks: Vec<Chunk>, out: &mut Vec<ToGateway>) -> bool {
        let full = chunks.len() >= HISTORY_CHUNKS as usize;
        if let Some(first) = chunks.first()
            && self.fed > 0
            && first.seq > self.fed + 1
        {
            tracing::warn!(session = %self.id, "output went by unseen; re-syncing");
            self.synced = false;
            if self.resync == Resync::Off && self.state == SessionState::Realm {
                self.request_resync();
            }
        }
        for c in chunks {
            if c.seq > self.fed {
                self.feed(c, out);
            }
        }
        if full {
            self.fetch(out);
            return false;
        }
        let mut buffer = std::mem::take(&mut self.buffer);
        buffer.sort_by_key(|c| c.seq);
        let mut rest = buffer.into_iter();
        while let Some(c) = rest.next() {
            if c.seq <= self.fed {
                continue;
            }
            if c.seq > self.fed + 1 {
                self.buffer.push(c);
                self.buffer.extend(rest);
                self.fetch(out);
                return false;
            }
            self.feed(c, out);
        }
        let was_fetching = self.fetching;
        self.fetching = false;
        was_fetching && self.published.is_none()
    }

    fn feed(&mut self, chunk: Chunk, out: &mut Vec<ToGateway>) {
        let silent = chunk.seq <= self.silent_until;
        let events = self.parser.feed_at(chunk.seq, &chunk.text);
        let now = Instant::now();
        let mut changed = false;
        let mut combat = false;
        for ev in &events {
            let applied = self.character.apply_event(ev);
            changed |= applied;
            if applied && self.atlas.is_some() {
                // Every HP value, not only those the bot sees at its ticks: a walk or an
                // action in flight does not hold the stop back (`bot_held` below ends the walk).
                let c = self.character.state();
                let says = self.bot.hp(hp_percent(c.hp, c.hp_max));
                self.say(says, out);
            }
            let typed = GameEvent::from_event(ev);
            if let Resync::Waiting {
                step,
                ref mut answered,
                ..
            } = self.resync
                && answers(step, &typed)
            {
                *answered = true;
            }
            if let Some(atlas) = &self.atlas
                && self.position.event(atlas, &typed).is_some()
            {
                self.executor.room_seen();
            }
            self.executor.event(&typed);
            combat |= typed == GameEvent::CombatEngaged;
            if OWN_COMBAT.contains(&ev.name.as_str()) {
                self.last_combat = now;
            }
            if let Some(atlas) = &self.atlas {
                self.presence.event(atlas, &typed);
                let says = self.bot.event(&typed, now);
                self.say(says, out);
            }
            if !silent {
                out.push(event_message(&self.id, ev));
            }
        }
        self.fed = chunk.seq;
        match events.last() {
            Some(ev) => self.at_prompt = ev.name == "prompt",
            None if visible(&chunk.text) => self.at_prompt = false,
            None => {}
        }
        if changed {
            self.character.set_updated_seq(chunk.seq);
        }
        if combat && self.walker.walking() {
            self.stop_walk("combat started", out);
        }
        self.bot_held(out);
        self.drive(now, out);
        self.flush_views(out);
        if let Resync::Waiting {
            step,
            answered: true,
            typed,
            ..
        } = self.resync
            && self.at_prompt
        {
            self.resync = Resync::Want {
                next: step + 1,
                idle: typed,
            };
        }
    }

    /// A line went to the game (the gateway's `sent`).
    pub fn sent(&mut self, text: &str, now: Instant, out: &mut Vec<ToGateway>) {
        while self
            .own
            .front()
            .is_some_and(|(_, at)| now.duration_since(*at) > OWN_PATIENCE)
        {
            self.own.pop_front();
        }
        if self.own.front().is_some_and(|(c, _)| c == text) {
            // Ours; the parser and the position were told when it went.
            self.own.pop_front();
            return;
        }
        self.parser.sent(text);
        if let Some(atlas) = &self.atlas {
            self.position.sent(atlas, text);
        }
        self.last_sent = now;
        match &mut self.resync {
            Resync::Waiting { typed, .. } => *typed = true,
            Resync::Want { idle, .. } => *idle = true,
            Resync::Off => {}
        }
        if self.walker.walking() {
            // The player comes first (docs/plan/02-architecture.md "Input arbitration").
            self.stop_walk("you typed a line", out);
        }
        self.take_over(out);
        self.flush_views(out);
    }

    /// The player acted (a typed line, a walk): a running bot pauses after the action in
    /// flight.
    fn take_over(&mut self, out: &mut Vec<ToGateway>) {
        if !self.bot.running() {
            return;
        }
        let name = self.character.state().name.as_deref().unwrap_or(&self.id);
        let first = name.split_whitespace().next().unwrap_or(name).to_owned();
        out.push(ToGateway::Notice {
            session: Some(self.id.clone()),
            text: format!("You took over {first}"),
        });
        let says = self
            .bot
            .pause("you took over", self.executor.in_flight().is_some());
        self.say(says, out);
        self.bot_held(out);
    }

    /// The bot is pausing, paused or stopped: its walk stops after the move in flight, and an
    /// action of its that has not gone yet is dropped.
    fn bot_held(&mut self, out: &mut Vec<ToGateway>) {
        if self.bot.active() {
            return;
        }
        if self.bot_walk {
            self.stop_walk("the bot is not running", out);
        }
        if self.owner == Some(Owner::Bot) && self.executor.in_flight().is_none() {
            self.executor.cancel();
            self.owner = None;
            self.bot.forget();
        }
    }

    /// Send one of the brain's own lines to the game.
    fn send_own(&mut self, cmd: &str, now: Instant, out: &mut Vec<ToGateway>) {
        self.parser.sent(cmd);
        if let Some(atlas) = &self.atlas {
            self.position.sent(atlas, cmd);
        }
        self.own.push_back((cmd.to_owned(), now));
        self.last_sent = now;
        self.at_prompt = false;
        out.push(ToGateway::InputLine {
            session: self.id.clone(),
            text: cmd.to_owned(),
        });
    }

    /// `walk_to`: plan a route from the believed room and start on it.
    pub fn walk_to(&mut self, to: Key, now: Instant, out: &mut Vec<ToGateway>) {
        let Some(atlas) = self.atlas.clone() else {
            out.push(self.warning("walk_to: the brain has no world pack"));
            return;
        };
        if self.state != SessionState::Realm {
            out.push(self.warning("walk_to: the character is not in the realm"));
            return;
        }
        self.take_over(out);
        self.bot_walk = false;
        // An action queued but not sent yet is the old walk's (or the bot's): drop it.
        if self.executor.in_flight().is_none() {
            self.cancel(out);
        }
        self.position.refresh(&atlas, &self.character.snapshot());
        let busy = self.executor.in_flight().is_some();
        let next = self.walker.start(&mut self.position, &atlas, to, busy);
        self.apply(next, out);
        self.drive(now, out);
        self.flush_views(out);
    }

    /// `walk_stop`.
    pub fn walk_stop(&mut self, out: &mut Vec<ToGateway>) {
        self.stop_walk("asked to stop", out);
        self.flush_views(out);
    }

    /// Stop walking after the action in flight; at once if none has gone.
    fn stop_walk(&mut self, why: &str, out: &mut Vec<ToGateway>) {
        if self.executor.in_flight().is_none() && self.owner == Some(Owner::Walker) {
            self.executor.cancel();
            self.owner = None;
        }
        let busy = self.executor.in_flight().is_some();
        let next = self.walker.stop(why, busy);
        self.apply(next, out);
    }

    /// `bot_control`.
    pub fn bot_control(&mut self, action: BotAction, now: Instant, out: &mut Vec<ToGateway>) {
        let Some(atlas) = self.atlas.clone() else {
            out.push(self.warning("bot: the brain has no world pack"));
            return;
        };
        let says = match action {
            BotAction::Start => {
                if self.state != SessionState::Realm {
                    out.push(self.warning("bot: the character is not in the realm"));
                    return;
                }
                if self.bot.running() {
                    return;
                }
                let home = self.position.best();
                let name = home
                    .and_then(|k| atlas.graph.room(k))
                    .map_or_else(|| "an unknown room".to_owned(), |r| r.name.clone());
                // Re-sync first: the HP maximum, the combat state, who is here.
                self.request_resync();
                self.bot.start(home, &name)
            }
            BotAction::Stop => self.bot.stop("asked to stop", false),
            BotAction::Pause => {
                let busy = self.executor.in_flight().is_some();
                self.bot.pause("asked to pause", busy)
            }
            BotAction::Resume => {
                let says = self.bot.resume();
                if !says.is_empty() {
                    self.request_resync();
                }
                says
            }
        };
        self.say(says, out);
        self.bot_held(out);
        self.drive(now, out);
        self.flush_views(out);
    }

    /// Forget the action in flight (it will not be answered): its owner learns nothing more.
    fn cancel(&mut self, out: &mut Vec<ToGateway>) {
        self.executor.cancel();
        if self.owner.take() == Some(Owner::Bot) {
            self.bot.forget();
            let says = self.bot.settle();
            self.say(says, out);
        }
    }

    /// The bot's narration and warnings, as notices and warnings for the UIs.
    fn say(&self, says: Vec<Say>, out: &mut Vec<ToGateway>) {
        for say in says {
            let session = Some(self.id.clone());
            tracing::info!(session = %self.id, "{say:?}");
            out.push(match say {
                Say::Notice(text) => ToGateway::Notice { session, text },
                Say::Warning(text) => ToGateway::Warning { session, text },
            });
        }
    }

    /// Whether the character is in combat: out of it once the game says so, while resting, or
    /// once it has been quiet for [`COMBAT_QUIET`]; engaged when the game says so; unknown
    /// before either (which the executor's guard takes as engaged).
    fn combat(&self, now: Instant) -> Option<bool> {
        let c = self.character.state();
        if c.in_combat == Some(false)
            || c.resting == Some(true)
            || now >= self.last_combat + COMBAT_QUIET
        {
            return Some(false);
        }
        c.in_combat
    }

    /// The bot decides, if it is running and the game is at a prompt with nothing in flight.
    fn bot_tick(&mut self, now: Instant, out: &mut Vec<ToGateway>) {
        if !self.bot.running()
            || self.state != SessionState::Realm
            || !self.live()
            || !self.at_prompt
            || self.executor.busy()
            || self.walker.walking()
            || self.resync != Resync::Off
        {
            return;
        }
        let Some(atlas) = self.atlas.clone() else {
            return;
        };
        self.position.refresh(&atlas, &self.character.snapshot());
        let in_combat = self.combat(now);
        let c = self.character.state();
        let view = View {
            atlas: &atlas,
            now,
            hp: hp_percent(c.hp, c.hp_max),
            engaged: in_combat != Some(false),
            resting: c.resting == Some(true),
            poisoned: c
                .statuses
                .as_ref()
                .is_some_and(|s| s.iter().any(|s| s == "poisoned")),
            room: self.position.best(),
            certain: self.position.belief.certain(),
            traveller: &self.position.traveller,
            presence: &self.presence,
        };
        let (act, says) = self.bot.tick(&view);
        self.say(says, out);
        match act {
            Some(Do::Send(action)) => match self.executor.submit(action, in_combat) {
                Ok(()) => self.owner = Some(Owner::Bot),
                Err(guard) => {
                    out.push(self.warning(&format!("refused: {}", guard.describe())));
                    self.bot.forget();
                    let says = self.bot.stop(guard.describe(), true);
                    self.say(says, out);
                }
            },
            Some(Do::Walk(to)) => {
                let next = self.walker.start(&mut self.position, &atlas, to, false);
                self.bot_walk = self.walker.walking();
                if !self.bot_walk {
                    self.bot.forget();
                }
                self.apply(next, out);
            }
            None => {}
        }
    }

    /// `map_request`.
    pub fn map_request(&mut self, radius: Option<u32>, out: &mut Vec<ToGateway>) {
        if let Some(r) = radius {
            self.map_radius = r;
        }
        if self.atlas.is_none() {
            out.push(self.warning("map: the brain has no world pack"));
        } else if self.position.best().is_none() {
            out.push(self.warning("map: the position is unknown"));
        } else {
            self.send_map(out);
        }
    }

    fn warning(&self, text: &str) -> ToGateway {
        ToGateway::Warning {
            session: Some(self.id.clone()),
            text: text.to_owned(),
        }
    }

    /// Do what the walker asked: narrate, and submit its next action.
    fn apply(&mut self, next: Next, out: &mut Vec<ToGateway>) {
        self.say(next.says, out);
        self.map_dirty |= next.route_changed;
        if let Some(action) = next.action {
            let in_combat = self.combat(Instant::now());
            match self.executor.submit(action, in_combat) {
                Ok(()) => self.owner = Some(Owner::Walker),
                Err(guard) => {
                    out.push(self.warning(&format!("refused: {}", guard.describe())));
                    let next = self.walker.abandon(guard.describe());
                    self.apply(next, out);
                }
            }
        }
    }

    /// Move the executor on: hand a finished action's outcome to the walker, and send the next
    /// action if this is a prompt boundary in the realm with nothing else in flight.
    fn drive(&mut self, now: Instant, out: &mut Vec<ToGateway>) {
        if let Some((action, outcome)) = self.executor.finished(self.at_prompt, now)
            && let Some(atlas) = self.atlas.clone()
        {
            tracing::debug!(session = %self.id, cmd = %action.command, ?outcome, "action done");
            match self.owner.take() {
                Some(Owner::Bot) => {
                    let says = self.bot.outcome(&action, outcome);
                    self.say(says, out);
                }
                _ => {
                    self.position.refresh(&atlas, &self.character.snapshot());
                    let next = self
                        .walker
                        .outcome(&mut self.position, &atlas, &action, outcome);
                    self.apply(next, out);
                    // A pause asked for while the walker's action was in flight.
                    let says = self.bot.settle();
                    self.say(says, out);
                }
            }
        }
        if self.bot_walk && !self.walker.walking() {
            self.bot_walk = false;
            self.bot.forget();
        }
        self.bot_tick(now, out);
        if self.state == SessionState::Realm
            && self.live()
            && self.at_prompt
            && !matches!(self.resync, Resync::Waiting { .. })
            && let Some(cmd) = self.executor.take_ready(now)
        {
            self.send_own(&cmd, now, out);
        }
    }

    /// The character with the session's position and walk.
    pub fn snapshot(&self) -> CharacterState {
        let mut s = self.character.snapshot();
        if self.atlas.is_some() {
            s.position = Some(self.position.snapshot());
        }
        s.walking = self.walker.snapshot();
        if self.atlas.is_some() {
            s.bot = Some(self.bot.snapshot());
        }
        s
    }

    /// Send what changed since the UIs last heard: a patch, and a `map_view` if the believed
    /// room or the route changed.
    fn flush_views(&mut self, out: &mut Vec<ToGateway>) {
        self.publish_patch(out);
        if self.published.is_some() && (self.map_dirty || self.map_center != self.position.best()) {
            self.send_map(out);
        }
    }

    /// A `map_view` now, if there is a world pack and a position.
    pub fn send_map(&mut self, out: &mut Vec<ToGateway>) {
        let Some(atlas) = &self.atlas else { return };
        self.map_dirty = false;
        self.map_center = self.position.best();
        let route = self.walker.route_keys();
        if let Some(view) = self
            .position
            .map_view(atlas, &self.id, self.map_radius, route)
        {
            out.push(view);
        }
    }

    fn publish_patch(&mut self, out: &mut Vec<ToGateway>) {
        let Some(old) = &self.published else { return };
        let new = serde_json::to_value(self.snapshot()).expect("serializes");
        if let Some(patch) = merge_patch(old, &new) {
            out.push(ToGateway::CharacterPatch {
                session: self.id.clone(),
                patch,
            });
            self.published = Some(new);
        }
    }

    /// The whole snapshot, which later patches are relative to.
    pub fn publish(&mut self, out: &mut Vec<ToGateway>) {
        let state = self.snapshot();
        self.published = Some(serde_json::to_value(&state).expect("serializes"));
        out.push(ToGateway::Character {
            session: self.id.clone(),
            state: Box::new(state),
        });
    }

    /// The brain's lines still waiting for the gateway's `sent`.
    pub fn own_pending(&self) -> usize {
        self.own.len()
    }

    /// Whether the session is caught up and published (it may then be sent to).
    fn live(&self) -> bool {
        !self.fetching && self.published.is_some()
    }

    /// When [`Session::step`] next has something to do, if a timer must wake it.
    pub fn deadline(&self) -> Option<Instant> {
        if self.state != SessionState::Realm || !self.live() {
            return None;
        }
        let resync = match self.resync {
            // The executor's action goes first; the re-sync waits for it.
            Resync::Want { idle: true, .. } if self.at_prompt && !self.executor.busy() => {
                Some(self.last_sent + IDLE)
            }
            Resync::Waiting { deadline, .. } => Some(deadline),
            _ => None,
        };
        // The bot decides again when the game has been quiet long enough for a fight to have
        // lapsed, or when a wait of Recover's ends.
        let now = Instant::now();
        let bot = [Some(self.last_combat + COMBAT_QUIET), self.bot.wake()]
            .into_iter()
            .flatten()
            .filter(|&t| t > now && self.bot.running() && !self.executor.busy());
        resync
            .into_iter()
            .chain(self.executor.deadline())
            .chain(bot)
            .min()
    }

    /// Move the executor and the re-sync on: an action whose time ran out, the next re-sync
    /// command if its moment has come, an answer that has not come.
    pub fn step(&mut self, now: Instant, out: &mut Vec<ToGateway>) -> Progress {
        if self.state != SessionState::Realm || !self.live() {
            return Progress::None;
        }
        if let Resync::Waiting {
            step,
            deadline,
            typed,
            ..
        } = self.resync
            && now >= deadline
        {
            tracing::debug!(session = %self.id, cmd = RESYNC_COMMANDS[step], "no answer; going on");
            self.resync = Resync::Want {
                next: step + 1,
                idle: typed,
            };
        }
        self.drive(now, out);
        self.flush_views(out);
        if self.executor.busy() {
            // One line in flight: the re-sync waits for the executor.
            return Progress::None;
        }
        match self.resync {
            Resync::Want { next, .. } if next >= RESYNC_COMMANDS.len() => {
                self.resync = Resync::Off;
                self.synced = true;
                // Every answer has come, so every `sent` before it has: nothing is pending.
                self.own.clear();
                // The bot waits for the re-sync: it decides now.
                self.drive(now, out);
                Progress::Done
            }
            Resync::Want { next, idle }
                if self.at_prompt && (!idle || now >= self.last_sent + IDLE) =>
            {
                self.send_own(RESYNC_COMMANDS[next], now, out);
                self.resync = Resync::Waiting {
                    step: next,
                    deadline: now + ANSWER_TIMEOUT,
                    answered: false,
                    typed: false,
                };
                Progress::None
            }
            _ => Progress::None,
        }
    }
}

pub(crate) fn event_message(session: &str, ev: &Event) -> ToGateway {
    let fields = match serde_json::to_value(&ev.fields).expect("serializes") {
        serde_json::Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    ToGateway::Event {
        session: session.to_owned(),
        seq: ev.seq,
        line_no: ev.line_no,
        name: ev.name.clone(),
        cat: ev.cat.clone(),
        fields,
        line: ev.line.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_skips_escapes_and_space() {
        assert!(!visible("\x1b[0;37m\r\n \x1b[K"));
        assert!(visible("\x1b[1;32mYou"));
    }
}
