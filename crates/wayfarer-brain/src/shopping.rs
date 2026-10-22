//! The spell-shopping errand (docs/brain.md "Buying spells"): from the spell plan, withdraw what
//! is needed from the Bank of Godfrey when the cash on hand falls short, walk to each shop that
//! sells a wanted scroll, buy it and read it. When the money does not cover everything it buys
//! what it can, the cheapest first.
//!
//! Like the other errands it is driven by the session: it asks for its next command (or walk)
//! when nothing is in flight, hears the game's events and is told how each command ended.

use std::collections::{BTreeMap, VecDeque};

use wayfarer_parse::typed::GameEvent;
use wayfarer_world::Key;

use crate::errand::{Ctx, ErrandNext};
use crate::executor::{Action, Expect, Outcome};

/// A scroll to buy: what the plan found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Want {
    pub spell: String,
    pub scroll: String,
    pub shop: String,
    pub room: Key,
    /// What the plan expects it to cost, in copper.
    pub price: u64,
    /// Steps to the shop when planned.
    pub steps: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Job {
    /// Walk to a room, named for the narration.
    Walk { to: Key, name: String },
    /// `bankbook`: the balance.
    Balance,
    /// `withdraw N`, the most the balance and the need allow.
    Withdraw,
    /// `buy <scroll>`.
    Buy(usize),
    /// `read <scroll>`.
    Read(usize),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Heard {
    bought: Option<u64>,
    refused: Option<&'static str>,
    learned: bool,
    known: bool,
    read_refused: bool,
    deposit: Option<u64>,
    withdrew: Option<u64>,
}

#[derive(Debug)]
pub(crate) struct Shopping {
    wants: Vec<Want>,
    bank: Option<Key>,
    /// Copper on hand now: what the plan started with, less what was paid, plus what was drawn.
    cash: u64,
    started: bool,
    todo: VecDeque<Job>,
    current: Option<(Job, Heard)>,
    /// The walk at the front of the list has been asked for.
    walking: bool,
    balance: Option<u64>,
    spent: u64,
    learned: Vec<String>,
    left: Vec<String>,
}

/// Copper as coins, largest first (`4 gold 3 silver`).
pub(crate) fn coin_text(copper: u64) -> String {
    if copper == 0 {
        return "nothing".to_owned();
    }
    let mut left = copper;
    let mut parts = Vec::new();
    for (name, unit) in [
        ("runic", 1_000_000),
        ("platinum", 10_000),
        ("gold", 100),
        ("silver", 10),
        ("copper", 1),
    ] {
        if left >= unit {
            parts.push(format!("{} {name}", left / unit));
            left %= unit;
        }
    }
    parts.join(" ")
}

impl Shopping {
    /// `wants`: the usable scrolls the plan found, cheapest first; `bank`: where to draw money;
    /// `cash`: the copper carried.
    pub fn new(wants: Vec<Want>, bank: Option<Key>, cash: u64) -> Self {
        Shopping {
            wants,
            bank,
            cash,
            started: false,
            todo: VecDeque::new(),
            current: None,
            walking: false,
            balance: None,
            spent: 0,
            learned: Vec::new(),
            left: Vec::new(),
        }
    }

    pub fn narration(&self) -> String {
        let what = match (&self.current, self.todo.front()) {
            (Some((job, _)), _) => self.describe(job),
            (None, Some(job)) => self.describe(job),
            (None, None) => "finishing".to_owned(),
        };
        format!("Spells: {what}")
    }

    fn describe(&self, job: &Job) -> String {
        match job {
            Job::Walk { name, .. } => format!("walking to {name}"),
            Job::Balance => "checking the bank balance".to_owned(),
            Job::Withdraw => "withdrawing".to_owned(),
            Job::Buy(i) => format!("buying {}", self.wants[*i].scroll),
            Job::Read(i) => format!("reading {}", self.wants[*i].scroll),
        }
    }

    /// The copper everything wanted costs.
    fn need(&self) -> u64 {
        self.wants.iter().map(|w| w.price).sum()
    }

    /// The jobs for the shops: the wants the cash covers, cheapest first, each shop's together,
    /// the shops nearest first.
    fn shop_jobs(&mut self) {
        let mut budget = self.cash;
        let mut picked: Vec<usize> = Vec::new();
        for (i, w) in self.wants.iter().enumerate() {
            if w.price <= budget {
                budget -= w.price;
                picked.push(i);
            } else {
                self.left
                    .push(format!("{} ({})", w.spell, coin_text(w.price)));
            }
        }
        // By shop room, the nearest first.
        let mut shops: BTreeMap<(u32, Key), Vec<usize>> = BTreeMap::new();
        for i in picked {
            let w = &self.wants[i];
            shops
                .entry((w.steps.unwrap_or(u32::MAX), w.room))
                .or_default()
                .push(i);
        }
        for ((_, room), spells) in shops {
            let name = self.wants[spells[0]].shop.clone();
            self.todo.push_back(Job::Walk { to: room, name });
            for i in spells {
                self.todo.push_back(Job::Buy(i));
            }
        }
    }

    /// The next walk or command, when nothing is in flight and the game is at a prompt.
    pub fn step(&mut self, ctx: &Ctx) -> ErrandNext {
        if ctx.walking {
            return ErrandNext::Wait;
        }
        if self.current.is_some() {
            return ErrandNext::Wait;
        }
        if !self.started {
            self.started = true;
            match self.bank {
                Some(bank) if self.need() > self.cash => {
                    self.todo.push_back(Job::Walk {
                        to: bank,
                        name: "the Bank of Godfrey".to_owned(),
                    });
                    self.todo.push_back(Job::Balance);
                    self.todo.push_back(Job::Withdraw);
                }
                _ => self.shop_jobs(),
            }
        }
        loop {
            let Some(job) = self.todo.front().cloned() else {
                return ErrandNext::Done(self.summary());
            };
            match job {
                Job::Walk { ref to, ref name } => {
                    let (to, name) = (*to, name.clone());
                    if !self.walking {
                        self.walking = true;
                        return ErrandNext::Walk(to);
                    }
                    // The walk ended: there, or not.
                    self.walking = false;
                    self.todo.pop_front();
                    if ctx.here != Some(to) {
                        self.left.push(format!("could not get to {name}"));
                        self.skip_walk_jobs(&job);
                    }
                }
                Job::Withdraw => {
                    self.todo.pop_front();
                    let balance = self.balance.unwrap_or(0);
                    let amount = balance.min(self.need().saturating_sub(self.cash));
                    if amount == 0 {
                        self.left.push("nothing to withdraw".to_owned());
                        self.shop_jobs();
                        continue;
                    }
                    self.current = Some((Job::Withdraw, Heard::default()));
                    return ErrandNext::Send(Action::new(
                        format!("withdraw {amount}"),
                        Expect::Prompt,
                    ));
                }
                other => {
                    self.todo.pop_front();
                    let command = match &other {
                        Job::Balance => "bankbook".to_owned(),
                        Job::Buy(i) => format!("buy {}", self.wants[*i].scroll),
                        Job::Read(i) => format!("read {}", self.wants[*i].scroll),
                        _ => unreachable!("handled above"),
                    };
                    self.current = Some((other, Heard::default()));
                    return ErrandNext::Send(Action::new(command, Expect::Prompt));
                }
            }
        }
    }

    /// A walk failed: what was to be done there is left, up to the next walk (the bank's balance
    /// and withdrawal included, and then the shops are planned with the cash on hand).
    fn skip_walk_jobs(&mut self, failed: &Job) {
        let at_bank = matches!(failed, Job::Walk { to, .. } if Some(*to) == self.bank);
        while let Some(front) = self.todo.front() {
            if matches!(front, Job::Walk { .. }) {
                break;
            }
            self.todo.pop_front();
        }
        if at_bank {
            self.shop_jobs();
        }
    }

    pub fn on_event(&mut self, ev: &GameEvent) {
        let Some((_, heard)) = &mut self.current else {
            return;
        };
        match ev {
            GameEvent::Bought { copper, .. } => heard.bought = Some(*copper),
            GameEvent::BuyRefused { why } => heard.refused = Some(why),
            GameEvent::Learned { .. } => heard.learned = true,
            GameEvent::AlreadyKnown => heard.known = true,
            GameEvent::LearnRefused => heard.read_refused = true,
            GameEvent::Deposit { copper } => heard.deposit = Some(*copper),
            GameEvent::Withdrew { copper } => heard.withdrew = Some(*copper),
            _ => {}
        }
    }

    pub fn outcome(&mut self, _action: &Action, outcome: Outcome) {
        let Some((job, heard)) = self.current.take() else {
            return;
        };
        let timed_out = matches!(outcome, Outcome::TimedOut);
        match job {
            Job::Balance => self.balance = Some(heard.deposit.unwrap_or(0)),
            Job::Withdraw => {
                if let Some(n) = heard.withdrew {
                    self.cash += n;
                } else {
                    self.left
                        .push("the withdrawal did not go through".to_owned());
                }
                // What the cash now covers.
                self.shop_jobs();
            }
            Job::Buy(i) => match heard.bought {
                Some(paid) if !timed_out => {
                    self.cash = self.cash.saturating_sub(paid);
                    self.spent += paid;
                    self.todo.push_front(Job::Read(i));
                }
                _ => {
                    let why = match heard.refused {
                        Some("poor") => "cannot afford it",
                        Some("not_here") => "not sold here now",
                        Some(_) => "unknown to the shop",
                        None => "no answer",
                    };
                    self.left.push(format!("{} ({why})", self.wants[i].spell));
                }
            },
            Job::Read(i) => {
                let w = &self.wants[i];
                if heard.learned {
                    self.learned.push(w.spell.clone());
                } else if heard.known {
                    self.left.push(format!("{} (already known)", w.spell));
                } else {
                    self.left.push(format!(
                        "{} (bought, could not be read{})",
                        w.spell,
                        if heard.read_refused {
                            ""
                        } else {
                            ": no answer"
                        }
                    ));
                }
            }
            Job::Walk { .. } => {}
        }
    }

    fn summary(&self) -> String {
        let learned = if self.learned.is_empty() {
            "nothing learned".to_owned()
        } else {
            format!("learned {}", self.learned.join(", "))
        };
        let mut text = format!("Spells done: {learned} (spent {})", coin_text(self.spent));
        if !self.left.is_empty() {
            text.push_str(&format!("; left: {}", self.left.join(", ")));
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::Confirmed;

    fn want(spell: &str, shop: &str, room: i64, price: u64, steps: u32) -> Want {
        Want {
            spell: spell.into(),
            scroll: format!("scroll of {spell}"),
            shop: shop.into(),
            room: (1, room),
            price,
            steps: Some(steps),
        }
    }

    fn ctx_at<'a>(here: Option<Key>, walking: bool, empty: &'a BTreeMap<String, u64>) -> Ctx<'a> {
        Ctx {
            floor: empty,
            floor_items: &[],
            carried: &[],
            coins: empty,
            load: None,
            limit: "heavy",
            here,
            walking,
        }
    }

    /// Play the errand against a game: a walk takes the character to its target, and each
    /// command is answered with `answer`'s events. The log of what it asked for.
    fn play(
        s: &mut Shopping,
        reachable: &[Key],
        answer: impl Fn(&str) -> Vec<GameEvent>,
    ) -> Vec<String> {
        let empty = BTreeMap::new();
        let mut here = Some((1, 1));
        let mut log = Vec::new();
        for _ in 0..200 {
            match s.step(&ctx_at(here, false, &empty)) {
                ErrandNext::Walk(to) => {
                    log.push(format!("walk {}", to.1));
                    // Arrived (or not) before the next ask.
                    here = if reachable.contains(&to) {
                        Some(to)
                    } else {
                        here
                    };
                }
                ErrandNext::Send(a) => {
                    log.push(a.command.clone());
                    for ev in answer(&a.command) {
                        s.on_event(&ev);
                    }
                    s.outcome(&a, Outcome::Confirmed(Confirmed::Prompt));
                }
                ErrandNext::Wait => panic!("waiting"),
                ErrandNext::Done(text) => {
                    log.push(format!("= {text}"));
                    return log;
                }
            }
        }
        panic!("never finished: {log:?}");
    }

    fn game(cmd: &str) -> Vec<GameEvent> {
        match cmd {
            "bankbook" => vec![GameEvent::Deposit { copper: 1000 }],
            c if c.starts_with("withdraw ") => vec![GameEvent::Withdrew {
                copper: c["withdraw ".len()..].parse().unwrap(),
            }],
            "buy scroll of mend" => vec![GameEvent::Bought {
                item: "scroll of mend".into(),
                copper: 832,
            }],
            "buy scroll of illuminate" => vec![GameEvent::Bought {
                item: "scroll of illuminate".into(),
                copper: 416,
            }],
            c if c.starts_with("read scroll of ") => vec![GameEvent::Learned {
                spell: c["read scroll of ".len()..].to_owned(),
            }],
            _ => vec![],
        }
    }

    #[test]
    fn with_enough_cash_it_walks_to_the_shop_buys_and_reads() {
        let mut s = Shopping::new(
            vec![want("illuminate", "Spell Shop", 20, 400, 5)],
            Some((1, 297)),
            500,
        );
        let log = play(&mut s, &[(1, 20)], game);
        assert_eq!(
            log,
            [
                "walk 20",
                "buy scroll of illuminate",
                "read scroll of illuminate",
                "= Spells done: learned illuminate (spent 4 gold 1 silver 6 copper)"
            ]
        );
    }

    #[test]
    fn short_of_cash_it_draws_from_the_bank_first_and_buys_what_the_money_covers() {
        // 400 + 800 + 5000 wanted, 100 in hand, 1000 at the bank: 1000 is drawn (all it has,
        // less than the need), so 1100 buys illuminate (400, cheapest first) and not mend (800
        // more) or firebolt.
        let mut s = Shopping::new(
            vec![
                want("illuminate", "Spell Shop", 20, 400, 5),
                want("mend", "Spell Shop", 20, 800, 5),
                want("firebolt", "Mage Shop", 30, 5000, 9),
            ],
            Some((1, 297)),
            100,
        );
        let log = play(&mut s, &[(1, 297), (1, 20), (1, 30)], game);
        assert_eq!(
            log,
            [
                "walk 297",
                "bankbook",
                "withdraw 1000",
                "walk 20",
                "buy scroll of illuminate",
                "read scroll of illuminate",
                "= Spells done: learned illuminate (spent 4 gold 1 silver 6 copper); left: mend (8 gold), firebolt (50 gold)"
            ]
        );
    }

    #[test]
    fn a_bank_it_cannot_reach_leaves_it_to_what_is_on_hand() {
        let mut s = Shopping::new(
            vec![want("illuminate", "Spell Shop", 20, 400, 5)],
            Some((1, 297)),
            500,
        );
        // Cash covers it: no bank at all.
        let log = play(&mut s, &[(1, 20)], game);
        assert!(!log.contains(&"bankbook".to_owned()), "{log:?}");
        let mut s = Shopping::new(
            vec![want("illuminate", "Spell Shop", 20, 400, 5)],
            Some((1, 297)),
            100,
        );
        // The bank is unreachable: the 100 on hand buys nothing, and it says so.
        let log = play(&mut s, &[(1, 20)], game);
        assert_eq!(
            log,
            [
                "walk 297",
                "= Spells done: nothing learned (spent nothing); left: could not get to the Bank of Godfrey, illuminate (4 gold)"
            ]
        );
    }

    #[test]
    fn a_shop_that_cannot_be_reached_is_left_and_the_next_one_goes_on() {
        let mut s = Shopping::new(
            vec![
                want("illuminate", "Far Shop", 20, 400, 5),
                want("mend", "Near Shop", 30, 800, 3),
            ],
            None,
            2000,
        );
        let log = play(&mut s, &[(1, 30)], game);
        assert_eq!(
            log,
            [
                "walk 30",
                "buy scroll of mend",
                "read scroll of mend",
                "walk 20",
                "= Spells done: learned mend (spent 8 gold 3 silver 2 copper); left: could not get to Far Shop"
            ]
        );
    }

    #[test]
    fn a_refused_buy_is_left_with_why_and_no_read_follows() {
        let mut s = Shopping::new(vec![want("mend", "Shop", 20, 800, 5)], None, 800);
        let log = play(&mut s, &[(1, 20)], |cmd| match cmd {
            "buy scroll of mend" => vec![GameEvent::BuyRefused { why: "poor" }],
            _ => vec![],
        });
        assert_eq!(
            log,
            [
                "walk 20",
                "buy scroll of mend",
                "= Spells done: nothing learned (spent nothing); left: mend (cannot afford it)"
            ]
        );
    }

    #[test]
    fn coins_read_as_the_largest_first() {
        assert_eq!(coin_text(0), "nothing");
        assert_eq!(coin_text(432), "4 gold 3 silver 2 copper");
    }
}
