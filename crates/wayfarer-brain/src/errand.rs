//! Errands (docs/brain.md "Errands"): one-shot jobs the player starts from a tile, run with the
//! bot stopped or paused. **Pick up all** takes the coins and the items on the floor; **Equip
//! all** wears and wields what is carried into empty slots, never replacing what is worn.
//!
//! An errand is a small state machine the session drives like the bot: it is asked for its next
//! command when nothing is in flight and the game is at a prompt, it hears the game's events,
//! and it is told how the command ended. It never fights or flees; the session ends it when
//! something attacks.

use std::collections::{BTreeMap, VecDeque};

use wayfarer_parse::helpers::Carried;
use wayfarer_parse::typed::GameEvent;

use crate::executor::{Action, Expect, Outcome};
use crate::loot::{coin_word, limit_pct, room_for};

/// The coins, largest first, by the word `get` takes: all of them, copper too.
const COINS: [&str; 5] = ["runic", "platinum", "gold", "silver", "copper"];
/// `get N <coin>` takes 1 to 99998 (`kb/economy.md`).
const MOST_AT_ONCE: u64 = 99_998;
/// Copies of one item picked up (or worn) at most.
const MOST_COPIES: u32 = 10;
/// Words of items that are not worn: they are left alone by Equip all, whatever the game says.
const NOT_WORN: [&str; 8] = [
    "torch", "lantern", "scroll", "potion", "elixir", "key", "flask", "bread",
];

/// Which errand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ErrandKind {
    PickUp,
    Equip,
}

impl ErrandKind {
    /// The name on the wire (`character.errand` begins with it).
    pub fn label(self) -> &'static str {
        match self {
            ErrandKind::PickUp => "Pick up",
            ErrandKind::Equip => "Equip",
        }
    }
}

/// What the errand needs of the session to decide.
pub(crate) struct Ctx<'a> {
    /// The coins on the floor by the game's name, and the other things there.
    pub floor: &'a BTreeMap<String, u64>,
    pub floor_items: &'a [Carried],
    /// Carried and worn items, as the last `i` listed them.
    pub carried: &'a [Carried],
    /// Coins carried by word (`gold`).
    pub coins: &'a BTreeMap<String, u64>,
    /// Weight carried and the most, when known.
    pub load: Option<(u64, u64)>,
    /// `cash.max_encumbrance`.
    pub limit: &'a str,
}

/// What the errand does next.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ErrandNext {
    Send(Action),
    /// Nothing now: a command is in flight.
    Wait,
    /// Finished, with what to say.
    Done(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Job {
    /// `get <n> <coin>`.
    Coin {
        word: &'static str,
        n: u64,
    },
    /// `get <name>`.
    Item {
        name: String,
    },
    /// `wear <name>`, then `wield <name>` if it is not wearable and no weapon is readied.
    Wear {
        name: String,
    },
    Wield {
        name: String,
    },
}

impl Job {
    fn command(&self) -> String {
        match self {
            Job::Coin { word, n } => format!("get {n} {word}"),
            Job::Item { name } => format!("get {name}"),
            Job::Wear { name } => format!("wear {name}"),
            Job::Wield { name } => format!("wield {name}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// The first command (`look` or `i`) is to go, then the jobs are made from what it shows.
    Start,
    /// The first command is in flight.
    Refreshing,
    Working,
}

/// What the game said to the job in flight.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Heard {
    coins: bool,
    taken: bool,
    worn: bool,
    wielded: bool,
    wear_failed: bool,
    not_weapon: bool,
    too_heavy: bool,
}

#[derive(Debug)]
pub(crate) struct Errand {
    kind: ErrandKind,
    stage: Stage,
    todo: VecDeque<Job>,
    /// The job whose command is in flight, and what the game has said to it.
    current: Option<(Job, Heard)>,
    /// A weapon is readied (or was, this errand): Wield is not tried.
    weapon: bool,
    done: Vec<String>,
    skipped: Vec<String>,
    /// The game would not let the character carry more.
    full: bool,
}

impl Errand {
    pub fn new(kind: ErrandKind) -> Self {
        Errand {
            kind,
            stage: Stage::Start,
            todo: VecDeque::new(),
            current: None,
            weapon: false,
            done: Vec::new(),
            skipped: Vec::new(),
            full: false,
        }
    }

    pub fn kind(&self) -> ErrandKind {
        self.kind
    }

    /// A line for the tile: what it is doing.
    pub fn narration(&self) -> String {
        let what = match (&self.current, self.stage) {
            (Some((job, _)), _) => job.command(),
            (None, Stage::Start | Stage::Refreshing) => match self.kind {
                ErrandKind::PickUp => "looking".to_owned(),
                ErrandKind::Equip => "checking what is carried".to_owned(),
            },
            (None, Stage::Working) => "finishing".to_owned(),
        };
        format!("{}: {what}", self.kind.label())
    }

    /// The next command, when nothing is in flight and the game is at a prompt.
    pub fn step(&mut self, ctx: &Ctx) -> ErrandNext {
        if self.current.is_some() {
            return ErrandNext::Wait;
        }
        match self.stage {
            Stage::Start => {
                self.stage = Stage::Refreshing;
                let action = match self.kind {
                    ErrandKind::PickUp => Action::new("look", Expect::RoomSeen),
                    ErrandKind::Equip => Action::new("i", Expect::Prompt),
                };
                // Not a job: the answer makes them.
                self.current = Some((
                    Job::Item {
                        name: String::new(),
                    },
                    Heard::default(),
                ));
                ErrandNext::Send(action)
            }
            Stage::Refreshing => {
                self.stage = Stage::Working;
                self.todo = self.jobs(ctx);
                self.step(ctx)
            }
            Stage::Working => match self.todo.pop_front() {
                Some(job) => {
                    let action = Action::new(job.command(), Expect::Prompt);
                    self.current = Some((job, Heard::default()));
                    ErrandNext::Send(action)
                }
                None => ErrandNext::Done(self.summary()),
            },
        }
    }

    /// The jobs for what the first command showed.
    fn jobs(&mut self, ctx: &Ctx) -> VecDeque<Job> {
        let mut jobs = VecDeque::new();
        match self.kind {
            ErrandKind::PickUp => {
                for word in COINS {
                    let on_floor = ctx
                        .floor
                        .iter()
                        .find(|(name, _)| coin_word(name) == word)
                        .map_or(0, |(_, n)| *n);
                    if on_floor == 0 {
                        continue;
                    }
                    let n = match ctx.load {
                        Some((current, max)) => on_floor.min(room_for(
                            ctx.coins.get(word).copied().unwrap_or(0),
                            current,
                            max,
                            limit_pct(ctx.limit),
                        )),
                        None => on_floor,
                    };
                    if n > 0 {
                        jobs.push_back(Job::Coin {
                            word,
                            n: n.min(MOST_AT_ONCE),
                        });
                    } else {
                        self.skipped.push(format!("{on_floor} {word} (too heavy)"));
                    }
                }
                for item in ctx.floor_items {
                    for _ in 0..item.count.min(MOST_COPIES) {
                        jobs.push_back(Job::Item {
                            name: item.name.clone(),
                        });
                    }
                }
            }
            ErrandKind::Equip => {
                self.weapon = ctx
                    .carried
                    .iter()
                    .any(|c| matches!(c.slot.as_deref(), Some("Weapon Hand" | "Two handed")));
                for item in ctx.carried.iter().filter(|c| c.slot.is_none()) {
                    let lower = item.name.to_lowercase();
                    if NOT_WORN
                        .iter()
                        .any(|w| lower.split_whitespace().any(|x| x == *w))
                    {
                        continue;
                    }
                    jobs.push_back(Job::Wear {
                        name: item.name.clone(),
                    });
                }
            }
        }
        jobs
    }

    /// An event from the game, whether or not a command is in flight.
    pub fn on_event(&mut self, ev: &GameEvent) {
        let Some((_, heard)) = &mut self.current else {
            return;
        };
        match ev {
            GameEvent::CoinsPicked { .. } => heard.coins = true,
            GameEvent::ItemTaken { .. } => heard.taken = true,
            GameEvent::Worn { .. } => heard.worn = true,
            GameEvent::Wielded { .. } => heard.wielded = true,
            GameEvent::WearFailed => heard.wear_failed = true,
            GameEvent::NotWielded => heard.not_weapon = true,
            GameEvent::CannotCarry => heard.too_heavy = true,
            _ => {}
        }
    }

    /// The command ended (its prompt came, or it timed out): what the game said to it.
    pub fn outcome(&mut self, _action: &Action, outcome: Outcome) {
        let Some((job, heard)) = self.current.take() else {
            return;
        };
        if self.stage == Stage::Refreshing {
            // The first command: the next step makes the jobs.
            return;
        }
        let timed_out = matches!(outcome, Outcome::TimedOut);
        match job {
            Job::Coin { word, n } => {
                if heard.coins && !timed_out {
                    self.done.push(format!("{n} {word}"));
                } else {
                    self.skipped.push(format!("{n} {word}"));
                }
                if heard.too_heavy {
                    self.full = true;
                }
            }
            Job::Item { name } => {
                if heard.taken && !timed_out {
                    self.done.push(name);
                } else {
                    self.skipped.push(name);
                }
                if heard.too_heavy {
                    self.full = true;
                }
            }
            Job::Wear { name } => {
                if heard.worn {
                    self.done.push(format!("wearing {name}"));
                } else if heard.wear_failed && !self.weapon && !timed_out {
                    // Not worn: a weapon, if the hand is empty.
                    self.todo.push_front(Job::Wield { name });
                } else {
                    self.skipped.push(name);
                }
            }
            Job::Wield { name } => {
                if heard.wielded {
                    self.weapon = true;
                    self.done.push(format!("holding {name}"));
                } else {
                    self.skipped.push(name);
                }
            }
        }
        if self.full {
            // Nothing more fits: the rest of the pickups are not tried.
            self.todo
                .retain(|j| !matches!(j, Job::Coin { .. } | Job::Item { .. }));
        }
    }

    fn summary(&self) -> String {
        let list = |v: &[String]| {
            if v.is_empty() {
                "nothing".to_owned()
            } else {
                v.join(", ")
            }
        };
        let mut text = format!("{} done: {}", self.kind.label(), list(&self.done));
        if !self.skipped.is_empty() {
            text.push_str(&format!("; left: {}", list(&self.skipped)));
        }
        if self.full {
            text.push_str(" (the load is too much)");
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn carried(name: &str, slot: Option<&str>) -> Carried {
        Carried {
            name: name.into(),
            slot: slot.map(str::to_owned),
            count: 1,
        }
    }

    fn ctx<'a>(
        floor: &'a BTreeMap<String, u64>,
        floor_items: &'a [Carried],
        carried: &'a [Carried],
        coins: &'a BTreeMap<String, u64>,
    ) -> Ctx<'a> {
        Ctx {
            floor,
            floor_items,
            carried,
            coins,
            load: None,
            limit: "heavy",
        }
    }

    fn ended(e: &mut Errand, a: &Action) {
        e.outcome(a, Outcome::Confirmed(crate::executor::Confirmed::Prompt));
    }

    /// Like `run`, but each command ends as the game's prompt confirms it.
    fn play(e: &mut Errand, c: &Ctx, answer: impl Fn(&str) -> Vec<GameEvent>) -> Vec<String> {
        let mut sent = Vec::new();
        loop {
            match e.step(c) {
                ErrandNext::Send(a) => {
                    sent.push(a.command.clone());
                    for ev in answer(&a.command) {
                        e.on_event(&ev);
                    }
                    ended(e, &a);
                }
                ErrandNext::Wait => panic!("waiting with nothing in flight"),
                ErrandNext::Done(text) => {
                    sent.push(format!("= {text}"));
                    return sent;
                }
            }
        }
    }

    #[test]
    fn pick_up_takes_each_coin_then_each_item_and_says_what_it_left() {
        let floor = BTreeMap::from([
            ("gold crowns".to_owned(), 5),
            ("copper farthings".to_owned(), 9),
        ]);
        let items = [carried("torch", None), carried("sword", None)];
        let none = BTreeMap::new();
        let c = ctx(&floor, &items, &[], &none);
        let mut e = Errand::new(ErrandKind::PickUp);
        let sent = play(&mut e, &c, |cmd| match cmd {
            "get 5 gold" | "get 9 copper" => vec![GameEvent::CoinsPicked {
                n: 1,
                coin: "x".into(),
            }],
            "get torch" => vec![GameEvent::ItemTaken {
                item: "torch".into(),
            }],
            // The sword is too heavy.
            "get sword" => vec![GameEvent::CannotCarry],
            _ => vec![],
        });
        assert_eq!(
            sent,
            [
                "look",
                "get 5 gold",
                "get 9 copper",
                "get torch",
                "get sword",
                "= Pick up done: 5 gold, 9 copper, torch; left: sword (the load is too much)"
            ]
        );
    }

    #[test]
    fn a_full_load_stops_the_rest_of_the_pickups() {
        let floor = BTreeMap::new();
        let items = [carried("anvil", None), carried("sword", None)];
        let none = BTreeMap::new();
        let c = ctx(&floor, &items, &[], &none);
        let mut e = Errand::new(ErrandKind::PickUp);
        let sent = play(&mut e, &c, |cmd| {
            if cmd == "get anvil" {
                vec![GameEvent::CannotCarry]
            } else {
                vec![]
            }
        });
        assert_eq!(
            sent.last().unwrap(),
            "= Pick up done: nothing; left: anvil (the load is too much)"
        );
        assert!(!sent.contains(&"get sword".to_owned()), "{sent:?}");
    }

    #[test]
    fn equip_wears_what_wears_wields_what_does_not_and_leaves_a_taken_slot() {
        let floor = BTreeMap::new();
        let have = [
            carried("padded vest", None),
            carried("iron helm", None),
            carried("short sword", None),
            carried("torch", None),
            carried("quarterstaff", Some("Two handed")),
        ];
        let none = BTreeMap::new();
        let c = ctx(&floor, &[], &have, &none);
        let mut e = Errand::new(ErrandKind::Equip);
        let sent = play(&mut e, &c, |cmd| match cmd {
            "wear padded vest" => vec![GameEvent::Worn {
                item: "padded vest".into(),
            }],
            // A helm slot that is taken, and a sword that is not worn.
            "wear iron helm" | "wear short sword" => vec![GameEvent::WearFailed],
            _ => vec![],
        });
        // A staff is readied, so the sword is not wielded; the torch is not tried.
        assert_eq!(
            sent,
            [
                "i",
                "wear padded vest",
                "wear iron helm",
                "wear short sword",
                "= Equip done: wearing padded vest; left: iron helm, short sword"
            ]
        );
    }

    #[test]
    fn equip_wields_a_weapon_when_the_hand_is_empty_and_only_the_first() {
        let floor = BTreeMap::new();
        let have = [carried("short sword", None), carried("dagger", None)];
        let none = BTreeMap::new();
        let c = ctx(&floor, &[], &have, &none);
        let mut e = Errand::new(ErrandKind::Equip);
        let sent = play(&mut e, &c, |cmd| match cmd {
            "wear short sword" | "wear dagger" => vec![GameEvent::WearFailed],
            "wield short sword" => vec![GameEvent::Wielded {
                item: "short sword".into(),
            }],
            _ => vec![],
        });
        assert_eq!(
            sent,
            [
                "i",
                "wear short sword",
                "wield short sword",
                "wear dagger",
                "= Equip done: holding short sword; left: dagger"
            ]
        );
    }

    #[test]
    fn a_timed_out_command_counts_as_not_done() {
        let floor = BTreeMap::new();
        let items = [carried("torch", None)];
        let none = BTreeMap::new();
        let c = ctx(&floor, &items, &[], &none);
        let mut e = Errand::new(ErrandKind::PickUp);
        // The look ends as a prompt; the pickup times out.
        let a = match e.step(&c) {
            ErrandNext::Send(a) => a,
            n => panic!("{n:?}"),
        };
        ended(&mut e, &a);
        let a = match e.step(&c) {
            ErrandNext::Send(a) => a,
            n => panic!("{n:?}"),
        };
        assert_eq!(a.command, "get torch");
        e.on_event(&GameEvent::ItemTaken {
            item: "torch".into(),
        });
        e.outcome(&a, Outcome::TimedOut);
        assert!(matches!(e.step(&c), ErrandNext::Done(t) if t.contains("left: torch")));
    }
}
