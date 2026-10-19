//! One session as the brain follows it: its parser, its character, the catch-up on its output
//! and the re-sync, which is the only thing in M1 that sends to the game.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::Instant;
use wayfarer_parse::typed::GameEvent;
use wayfarer_parse::{Event, Parser, Registry};
use wayfarer_proto::{Chunk, SessionId, SessionInfo, SessionState, ToGateway};

use crate::character::{Character, merge_patch};

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
}

impl Session {
    /// A session the brain has just learned of: its history up to now is fed silently.
    pub fn new(info: &SessionInfo, registry: Arc<Registry>, now: Instant) -> Self {
        Self {
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
        self.own.clear();
        self.last_sent = now;
        self.buffer.clear();
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
        self.set_state(info.state);
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

    pub fn set_state(&mut self, state: SessionState) {
        let was = self.state;
        self.state = state;
        if state != SessionState::Realm {
            self.synced = false;
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
        let mut changed = false;
        for ev in &events {
            changed |= self.character.apply_event(ev);
            if let Resync::Waiting {
                step,
                ref mut answered,
                ..
            } = self.resync
                && answers(step, &GameEvent::from_event(ev))
            {
                *answered = true;
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
            self.publish_patch(out);
        }
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
    pub fn sent(&mut self, text: &str, now: Instant) {
        while self
            .own
            .front()
            .is_some_and(|(_, at)| now.duration_since(*at) > OWN_PATIENCE)
        {
            self.own.pop_front();
        }
        if self.own.front().is_some_and(|(c, _)| c == text) {
            // Ours; the parser was told when it went.
            self.own.pop_front();
            return;
        }
        self.parser.sent(text);
        self.last_sent = now;
        match &mut self.resync {
            Resync::Waiting { typed, .. } => *typed = true,
            Resync::Want { idle, .. } => *idle = true,
            Resync::Off => {}
        }
    }

    fn publish_patch(&mut self, out: &mut Vec<ToGateway>) {
        let Some(old) = &self.published else { return };
        let new = serde_json::to_value(self.character.snapshot()).expect("serializes");
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
        let state = self.character.snapshot();
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
        match self.resync {
            Resync::Want { idle: true, .. } if self.at_prompt => Some(self.last_sent + IDLE),
            Resync::Waiting { deadline, .. } => Some(deadline),
            _ => None,
        }
    }

    /// Move the re-sync on: send the next command if its moment has come, give up waiting
    /// for an answer that has not come.
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
        match self.resync {
            Resync::Want { next, .. } if next >= RESYNC_COMMANDS.len() => {
                self.resync = Resync::Off;
                self.synced = true;
                // Every answer has come, so every `sent` before it has: nothing is pending.
                self.own.clear();
                Progress::Done
            }
            Resync::Want { next, idle }
                if self.at_prompt && (!idle || now >= self.last_sent + IDLE) =>
            {
                let cmd = RESYNC_COMMANDS[next];
                self.parser.sent(cmd);
                self.own.push_back((cmd.to_owned(), now));
                self.last_sent = now;
                self.at_prompt = false;
                self.resync = Resync::Waiting {
                    step: next,
                    deadline: now + ANSWER_TIMEOUT,
                    answered: false,
                    typed: false,
                };
                out.push(ToGateway::InputLine {
                    session: self.id.clone(),
                    text: cmd.to_owned(),
                });
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
