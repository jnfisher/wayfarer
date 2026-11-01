//! Which rules the server plays (C-064, docs/brain.md "Rule sets",
//! `docs/kb/quicksilver-1.20.md` "Detection"): stock 1.11p or Quicksilver 1.20, unknown until
//! known. At the first quiet prompt in the realm the brain asks `help 1.20`: a 1.20 server
//! answers with its topic, whose first line is the signal, and a 1.11p server with
//! `Sorry, no help is available for that topic!`. No answer within [`PROBE_TIMEOUT`] leaves it
//! unknown, asked again after [`RETRY_AFTER`], [`MAX_PROBES`] times a login. Meanwhile any line
//! only 1.20 can print switches it to 1.20 (the fallback signals). 1.20 is never turned back
//! to 1.11p in a login.
//!
//! The topic's lines are not game events: from its first line to the next prompt they are
//! kept as text (the dev panel shows them) and not applied to anything.

use std::time::Duration;

use tokio::time::Instant;
use wayfarer_parse::Event;
use wayfarer_parse::typed::GameEvent;
use wayfarer_proto::RuleSetView;
use wayfarer_world::RuleSet;
use wayfarer_world::rule_set::{QS120_HELP_TITLE, qs120_name};

/// How long the answer to `help 1.20` may take.
pub(crate) const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
/// How long after an unanswered probe the next may go.
pub(crate) const RETRY_AFTER: Duration = Duration::from_secs(60);
/// Probes a login, at most.
pub(crate) const MAX_PROBES: u32 = 3;
/// A guard refusal counts as a 1.20 line only this soon after a `guard` went: two of them
/// (`You don't see that player here.`, `<Name> is not in your party.`) read like stock lines.
/// The stock package has neither, but a sysop's own text might.
const GUARD_ANSWER: Duration = Duration::from_secs(10);

/// The bars as the character model has them, for the meditate signal.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Bars {
    pub hp: Option<i64>,
    pub hp_max: Option<i64>,
    pub mana: Option<i64>,
    pub mana_max: Option<i64>,
}

impl Bars {
    /// Mana full and HP not: 1.11p refuses to meditate then, 1.20 starts.
    fn mana_full_hp_not(&self) -> bool {
        match (self.hp, self.hp_max, self.mana, self.mana_max) {
            (Some(h), Some(hm), Some(m), Some(mm)) => mm > 0 && m >= mm && h < hm,
            _ => false,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct Detector {
    rules: RuleSet,
    /// Why the rule set is what it is (`help 1.20`, or the signal seen).
    reason: Option<String>,
    /// What this server was found to play at an earlier login: shown, never acted on.
    remembered: Option<RuleSet>,
    /// Probes sent this login.
    probes: u32,
    /// A probe is out, unanswered until then.
    asked: Option<Instant>,
    /// No probe before then.
    not_before: Option<Instant>,
    /// Inside the `help 1.20` topic, until the next prompt.
    in_topic: bool,
    /// The topic's lines.
    help: Vec<String>,
    /// When a `guard` line last went.
    guard_sent: Option<Instant>,
    /// Decided since the last [`Detector::take_decision`].
    decided: bool,
}

impl Detector {
    pub fn new(remembered: Option<RuleSet>) -> Self {
        Self {
            remembered: remembered.filter(|r| *r != RuleSet::Unknown),
            ..Self::default()
        }
    }

    /// The rules to play by now.
    pub fn rules(&self) -> RuleSet {
        self.rules
    }

    /// A new login (the session came into the realm from outside it): unknown again, asked
    /// again. A server can be rebuilt between logins. What was found is remembered.
    pub fn new_login(&mut self) {
        if self.rules != RuleSet::Unknown {
            self.remembered = Some(self.rules);
        }
        *self = Self::new(self.remembered);
    }

    /// Whether to send `help 1.20` now, given that the moment is quiet.
    pub fn wants_probe(&self, now: Instant) -> bool {
        self.rules == RuleSet::Unknown
            && self.asked.is_none()
            && !self.in_topic
            && self.probes < MAX_PROBES
            && self.not_before.is_none_or(|t| now >= t)
    }

    /// `help 1.20` went.
    pub fn probe_sent(&mut self, now: Instant) {
        self.probes += 1;
        self.asked = Some(now + PROBE_TIMEOUT);
    }

    /// When [`Detector::tick`] or [`Detector::wants_probe`] next has something to do.
    pub fn deadline(&self) -> Option<Instant> {
        match self.asked {
            Some(t) => Some(t),
            None if self.rules == RuleSet::Unknown && self.probes < MAX_PROBES => self.not_before,
            None => None,
        }
    }

    /// A probe whose answer has not come in time: unknown still, asked again later.
    pub fn tick(&mut self, now: Instant) {
        if self.asked.is_some_and(|t| now >= t) {
            tracing::debug!("no answer to help 1.20");
            self.asked = None;
            self.not_before = Some(now + RETRY_AFTER);
        }
    }

    /// A line went to the game, the player's or the brain's.
    pub fn sent(&mut self, text: &str, now: Instant) {
        let first = text.split_whitespace().next().unwrap_or_default();
        if first.eq_ignore_ascii_case("guard") {
            self.guard_sent = Some(now);
        }
    }

    /// One event the parser read. True when it is a line of the `help 1.20` topic: not a game
    /// event, kept as text only. `bars` are the character's as the model has them.
    pub fn event(&mut self, ev: &Event, typed: &GameEvent, bars: Bars, now: Instant) -> bool {
        if self.in_topic {
            if matches!(typed, GameEvent::Prompt { .. }) {
                self.in_topic = false;
                return false;
            }
            self.help.push(ev.line.clone());
            return true;
        }
        if ev.line.trim_end() == QS120_HELP_TITLE {
            self.asked = None;
            self.in_topic = true;
            self.help = vec![ev.line.trim_end().to_owned()];
            self.decide(RuleSet::Qs120, "the server answered help 1.20");
            return true;
        }
        if ev.name == "help_none" && self.asked.take().is_some() {
            self.decide(RuleSet::Stock, "no help 1.20 topic");
            return false;
        }
        if let Some(why) = self.signal(ev, typed, bars, now) {
            self.decide(RuleSet::Qs120, &why);
        }
        false
    }

    /// A line only a 1.20 server prints, and what it was.
    fn signal(&self, ev: &Event, typed: &GameEvent, bars: Bars, now: Instant) -> Option<String> {
        use GameEvent as G;
        let line = || format!("a 1.20 line: {}", ev.line.trim());
        match typed {
            G::Opening { .. }
            | G::Weave { .. }
            | G::Intercept { .. }
            | G::Guarding { .. }
            | G::StrRequirement { .. } => Some(line()),
            G::GuardRefused { .. }
                if self
                    .guard_sent
                    .is_some_and(|t| now.duration_since(t) <= GUARD_ANSWER) =>
            {
                Some(line())
            }
            G::YouHit {
                crit: true,
                backstab: true,
                ..
            } => Some(format!("a critical backstab: {}", ev.line.trim())),
            G::Other(e)
                if e.name == "other_hits"
                    && e.text("crit").is_some_and(|s| !s.is_empty())
                    && e.text("surprise").is_some_and(|s| !s.is_empty()) =>
            {
                Some(format!("a critical backstab: {}", ev.line.trim()))
            }
            G::SpellRow { name, .. } if qs120_name(name) => {
                Some(format!("a 1.20 spell in the spell list: {name}"))
            }
            G::Learned { spell } if qs120_name(spell) => {
                Some(format!("a 1.20 spell learned: {spell}"))
            }
            G::Bought { item, .. } if qs120_name(item) => {
                Some(format!("a 1.20 item bought: {item}"))
            }
            G::Inventory { items, .. } | G::RoomItems { items, .. } => items
                .iter()
                .find(|i| qs120_name(&i.name))
                .map(|i| format!("a 1.20 item: {}", i.name)),
            G::Resting { meditating: true } if bars.mana_full_hp_not() => {
                Some("meditation started at full mana".to_owned())
            }
            _ => None,
        }
    }

    fn decide(&mut self, rules: RuleSet, why: &str) {
        if self.rules == RuleSet::Qs120 || self.rules == rules {
            return;
        }
        tracing::info!(rules = %rules, why, "rule set");
        self.rules = rules;
        self.reason = Some(why.to_owned());
        self.asked = None;
        self.decided = true;
    }

    /// The rule set and its reason, once each time it is decided: for the store.
    pub fn take_decision(&mut self) -> Option<(RuleSet, String)> {
        if !std::mem::take(&mut self.decided) {
            return None;
        }
        Some((self.rules, self.reason.clone().unwrap_or_default()))
    }

    pub fn wire(&self) -> RuleSetView {
        RuleSetView {
            rules: self.rules.as_str().to_owned(),
            reason: self.reason.clone(),
            remembered: self.remembered.map(|r| r.as_str().to_owned()),
            probes: self.probes,
            help: self.help.clone(),
        }
    }
}

/// The `guard` line to send, or `None`: never unless the server is known to play 1.20 (on
/// 1.11p the word is not a command and is said aloud to the room). `ward` `None` stops
/// guarding. Every `guard` the brain sends goes through here (`GUARD`, C-068).
pub(crate) fn guard_line(rules: RuleSet, ward: Option<&str>) -> Option<String> {
    if !rules.qs120() {
        return None;
    }
    Some(match ward {
        Some(w) => format!("guard {w}"),
        None => "guard".to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, OnceLock};

    use wayfarer_parse::{Parser, Registry};

    use super::*;

    const HELP: &str = include_str!("../../../tests/fixtures/qs120/help-1.20.txt");
    const PROMPT: &str = "[HP=35/MA=20]:";

    fn reg() -> Arc<Registry> {
        static REG: OnceLock<Arc<Registry>> = OnceLock::new();
        REG.get_or_init(|| Arc::new(Registry::static_only()))
            .clone()
    }

    /// Feed `text` (lines, then a prompt) to the detector as the session does; the events it
    /// did not swallow, by name.
    fn feed(d: &mut Detector, text: &str, bars: Bars, now: Instant) -> Vec<String> {
        let mut p = Parser::new(reg());
        let mut evs = p.feed(text);
        evs.extend(p.finish());
        let mut kept = Vec::new();
        for ev in &evs {
            let typed = GameEvent::from_event(ev);
            if !d.event(ev, &typed, bars, now) {
                kept.push(ev.name.clone());
            }
        }
        kept
    }

    fn full() -> Bars {
        Bars {
            hp: Some(35),
            hp_max: Some(35),
            mana: Some(20),
            mana_max: Some(20),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn the_topic_makes_it_1_20_and_is_not_game_events() {
        let now = Instant::now();
        let mut d = Detector::new(None);
        assert!(d.wants_probe(now));
        d.probe_sent(now);
        assert!(!d.wants_probe(now));
        let text = format!("{}\r\n{PROMPT}", HELP.replace('\n', "\r\n").trim_end());
        let kept = feed(&mut d, &text, full(), now);
        assert_eq!(
            kept,
            ["prompt"],
            "only the prompt after the topic is a game event"
        );
        assert_eq!(d.rules(), RuleSet::Qs120);
        assert_eq!(
            d.take_decision(),
            Some((RuleSet::Qs120, "the server answered help 1.20".into()))
        );
        assert_eq!(d.take_decision(), None);
        let w = d.wire();
        assert_eq!(w.rules, "1.20");
        // The topic's lines, its one blank line aside (the parser gives none an event).
        let lines: Vec<&str> = HELP.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(w.help, lines);
        assert_eq!(w.help[0], QS120_HELP_TITLE);
        assert_eq!(w.probes, 1);
        assert!(!d.wants_probe(now));
        assert_eq!(d.deadline(), None);
        // Game lines after the prompt are game events again.
        let kept = feed(
            &mut d,
            &format!("You are now resting.\r\n{PROMPT}"),
            full(),
            now,
        );
        assert_eq!(kept, ["rest_start", "prompt"]);
    }

    #[tokio::test(start_paused = true)]
    async fn the_stock_answer_makes_it_1_11p() {
        let now = Instant::now();
        let mut d = Detector::new(None);
        d.probe_sent(now);
        let kept = feed(
            &mut d,
            &format!("Sorry, no help is available for that topic!\r\n{PROMPT}"),
            full(),
            now,
        );
        assert_eq!(kept, ["help_none", "prompt"]);
        assert_eq!(d.rules(), RuleSet::Stock);
        assert_eq!(
            d.take_decision(),
            Some((RuleSet::Stock, "no help 1.20 topic".into()))
        );
        assert!(!d.wants_probe(now));
    }

    #[tokio::test(start_paused = true)]
    async fn the_stock_answer_counts_only_for_the_probe() {
        let now = Instant::now();
        let mut d = Detector::new(None);
        feed(
            &mut d,
            &format!("Sorry, no help is available for that topic!\r\n{PROMPT}"),
            full(),
            now,
        );
        assert_eq!(d.rules(), RuleSet::Unknown);
    }

    #[tokio::test(start_paused = true)]
    async fn no_answer_three_times_leaves_it_unknown() {
        let mut now = Instant::now();
        let mut d = Detector::new(None);
        for n in 1..=MAX_PROBES {
            assert!(d.wants_probe(now), "try {n}");
            d.probe_sent(now);
            assert_eq!(d.deadline(), Some(now + PROBE_TIMEOUT));
            // Some other answer: nothing decided.
            feed(&mut d, &format!("Huh?\r\n{PROMPT}"), full(), now);
            d.tick(now + PROBE_TIMEOUT - Duration::from_millis(1));
            assert!(!d.wants_probe(now + PROBE_TIMEOUT));
            now += PROBE_TIMEOUT;
            d.tick(now);
            assert!(!d.wants_probe(now), "waits before trying again");
            if n < MAX_PROBES {
                assert_eq!(d.deadline(), Some(now + RETRY_AFTER));
            }
            now += RETRY_AFTER;
        }
        assert!(!d.wants_probe(now));
        assert_eq!(d.deadline(), None);
        assert_eq!(d.rules(), RuleSet::Unknown);
        assert_eq!(d.take_decision(), None);
        assert_eq!(d.wire().probes, MAX_PROBES);
    }

    fn signals(line: &str, bars: Bars) -> RuleSet {
        let now = Instant::now();
        let mut d = Detector::new(None);
        feed(&mut d, &format!("{line}\r\n{PROMPT}"), bars, now);
        d.rules()
    }

    #[tokio::test(start_paused = true)]
    async fn each_fallback_signal_makes_it_1_20() {
        for line in [
            // 1: a 1.20 line
            "You find an opening and slash giant rat for 31 damage!",
            "Thornwyn weaves meteor swarm into giant rat for 75 damage!",
            "You weave meteor swarm into giant rat, but it is resisted!",
            "Brannoc intercepts the orc's attack on Mirelle!",
            "Brannoc is now guarding you.",
            "It requires 75 strength to wield well (you have 92).",
            // 2: a critical stab
            "You critically surprise slash giant rat for 140 damage!",
            "Thornwyn critically surprise slashes giant rat for 140 damage!",
            // 3: a 1.20 spell or scroll
            "You read scroll of lesser attunement and learn the spell lesser attunement.",
        ] {
            assert_eq!(signals(line, full()), RuleSet::Qs120, "{line}");
        }
        // 3: the spell list
        let now = Instant::now();
        let mut d = Detector::new(None);
        let rows = "Level Mana Short Spell Name\r\n    5    10    paur  protective aura\r\n";
        feed(&mut d, &format!("{rows}{PROMPT}"), full(), now);
        assert_eq!(d.rules(), RuleSet::Qs120, "{:?}", d.wire());
        // 4: meditate starting at full mana with HP missing
        let hurt = Bars {
            hp: Some(20),
            ..full()
        };
        assert_eq!(signals("You are now meditating.", hurt), RuleSet::Qs120);
        assert_eq!(signals("You are now meditating.", full()), RuleSet::Unknown);
        let low_mana = Bars {
            mana: Some(5),
            ..hurt
        };
        assert_eq!(
            signals("You are now meditating.", low_mana),
            RuleSet::Unknown
        );
    }

    #[tokio::test(start_paused = true)]
    async fn stock_lines_are_no_signal() {
        for line in [
            "You surprise slash giant rat for 61 damage!",
            "You critically slash giant rat for 40 damage!",
            "You read scroll of mana flux and learn the spell mana flux.",
        ] {
            assert_eq!(signals(line, full()), RuleSet::Unknown, "{line}");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_guard_refusal_counts_only_after_a_guard() {
        let refusal = "You don't see that player here.";
        assert_eq!(signals(refusal, full()), RuleSet::Unknown);
        let now = Instant::now();
        let mut d = Detector::new(None);
        d.sent("guard mir", now);
        feed(&mut d, &format!("{refusal}\r\n{PROMPT}"), full(), now);
        assert_eq!(d.rules(), RuleSet::Qs120);
        let mut d = Detector::new(None);
        d.sent("guard mir", now);
        let later = now + GUARD_ANSWER + Duration::from_secs(1);
        feed(&mut d, &format!("{refusal}\r\n{PROMPT}"), full(), later);
        assert_eq!(d.rules(), RuleSet::Unknown);
    }

    #[tokio::test(start_paused = true)]
    async fn a_signal_turns_1_11p_into_1_20_and_never_back() {
        let now = Instant::now();
        let mut d = Detector::new(None);
        d.probe_sent(now);
        feed(
            &mut d,
            &format!("Sorry, no help is available for that topic!\r\n{PROMPT}"),
            full(),
            now,
        );
        assert_eq!(d.rules(), RuleSet::Stock);
        feed(
            &mut d,
            &format!("You find an opening and slash giant rat for 31 damage!\r\n{PROMPT}"),
            full(),
            now,
        );
        assert_eq!(d.rules(), RuleSet::Qs120);
        assert!(d.wire().reason.unwrap().starts_with("a 1.20 line"));
        // A stock answer later changes nothing.
        d.probe_sent(now);
        feed(
            &mut d,
            &format!("Sorry, no help is available for that topic!\r\n{PROMPT}"),
            full(),
            now,
        );
        assert_eq!(d.rules(), RuleSet::Qs120);
    }

    #[tokio::test(start_paused = true)]
    async fn a_new_login_asks_again_and_remembers() {
        let now = Instant::now();
        let mut d = Detector::new(Some(RuleSet::Qs120));
        // Remembered from an earlier login: shown, not acted on.
        assert_eq!(d.rules(), RuleSet::Unknown);
        assert_eq!(d.wire().remembered.as_deref(), Some("1.20"));
        d.probe_sent(now);
        feed(
            &mut d,
            &format!("Sorry, no help is available for that topic!\r\n{PROMPT}"),
            full(),
            now,
        );
        assert_eq!(d.rules(), RuleSet::Stock);
        d.new_login();
        assert_eq!(d.rules(), RuleSet::Unknown);
        assert_eq!(d.wire().remembered.as_deref(), Some("1.11p"));
        assert!(d.wants_probe(now));
        assert_eq!(d.wire().probes, 0);
    }

    #[test]
    fn guard_goes_only_on_1_20() {
        assert_eq!(guard_line(RuleSet::Unknown, Some("Mirelle")), None);
        assert_eq!(guard_line(RuleSet::Stock, Some("Mirelle")), None);
        assert_eq!(guard_line(RuleSet::Stock, None), None);
        assert_eq!(
            guard_line(RuleSet::Qs120, Some("Mirelle")).as_deref(),
            Some("guard Mirelle")
        );
        assert_eq!(guard_line(RuleSet::Qs120, None).as_deref(), Some("guard"));
    }
}
