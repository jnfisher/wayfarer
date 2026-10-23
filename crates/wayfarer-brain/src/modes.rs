//! The bot (docs/brain.md "Fight here", "Running a loop", docs/plan/03-engine.md sections 4 and
//! 5): running, paused or stopped, and the mode stack, highest first: Flee, Recover, Fight, Task
//! (with a loop to run), Idle. On every tick
//! the highest mode that wants control gets it and says what to do next; the session sends its
//! action through the executor (one in flight) or its walk through the walker, and hands the
//! outcome back. Every change of what it is doing is narrated. The monster rules (`rules.rs`)
//! are a policy input the modes read from the [`View`], not a mode.

use std::collections::BTreeMap;

use tokio::time::Instant;
use wayfarer_parse::typed::GameEvent;
use wayfarer_proto::{BotMode, BotState};
use wayfarer_store::BotRecord;
use wayfarer_world::{Key, LockState, MonsterRules, SpellDef, Traveller};

use crate::brain::Atlas;
use crate::executor::{Action, Outcome};
use crate::fight::Fight;
use crate::flee::Flee;
use crate::loot::Loot;
use crate::policy;
use crate::recover::Recover;
use crate::rules::Sightings;
use crate::spellcraft::{Hurt, PartyCast, Spellbook};
use crate::task::Task;
use crate::walker::Say;

/// Who is in the room, as the game has shown it: the monsters and players of the last `Also
/// here:`, with the arrivals, departures and deaths since; and those that attacked the
/// character since. A player is an attacker only once a PvP line says so (`pvp`).
#[derive(Debug, Default)]
pub(crate) struct Presence {
    pub here: Vec<String>,
    pub attackers: Vec<String>,
    /// The players of the last `Also here:`.
    players: Vec<String>,
    /// Players a PvP line showed attacking the character.
    pvp: Vec<String>,
    /// The room being shown: its `Also here:` monsters and players until its exits come.
    shown: Option<(Vec<String>, Vec<String>)>,
    /// The coins on the floor as the last room view showed them, by the game's name for them
    /// (`gold crowns`), until they are picked up or the view changes.
    pub floor: std::collections::BTreeMap<String, u64>,
    /// The other things on the floor, as the last room view showed them.
    pub floor_items: Vec<wayfarer_parse::helpers::Carried>,
}

impl Presence {
    pub fn event(&mut self, atlas: &Atlas, ev: &GameEvent) {
        match ev {
            GameEvent::RoomTitle { .. } => {
                self.shown = Some(Default::default());
                self.floor.clear();
                self.floor_items.clear();
            }
            GameEvent::RoomItems { coins, items } => {
                self.floor.clone_from(coins);
                self.floor_items.clone_from(items);
            }
            GameEvent::ItemTaken { item } => {
                let item = item.to_lowercase();
                if let Some(i) = self
                    .floor_items
                    .iter()
                    .position(|c| c.name.to_lowercase() == item)
                {
                    if self.floor_items[i].count > 1 {
                        self.floor_items[i].count -= 1;
                    } else {
                        self.floor_items.remove(i);
                    }
                }
            }
            // Taken, or the game would not give it: not on the floor for the bot's purposes
            // (a pickup that took only part is not asked for the rest again).
            GameEvent::CoinsPicked { coin, .. } => {
                let word = crate::loot::coin_word(coin).to_owned();
                self.floor
                    .retain(|name, _| crate::loot::coin_word(name) != word);
            }
            GameEvent::AlsoHere { monsters, players } => match &mut self.shown {
                Some(shown) => *shown = (monsters.clone(), players.clone()),
                None => {
                    self.here.clone_from(monsters);
                    self.players.clone_from(players);
                }
            },
            GameEvent::RoomExits { .. } | GameEvent::RoomDark | GameEvent::Prompt { .. } => {
                if let Some((here, players)) = self.shown.take() {
                    self.here = here;
                    self.players = players;
                    // A new room, or this one shown again: who attacked is here or gone.
                    let (here, players) = (&self.here, &self.players);
                    self.pvp.retain(|p| players.contains(p));
                    let pvp = &self.pvp;
                    self.attackers
                        .retain(|a| here.contains(a) || pvp.contains(a));
                }
            }
            // A player's blow can read like a monster's: not an attack without a PvP line.
            GameEvent::AttackedBy { name }
                if self.is_player(atlas, name) && !self.pvp.contains(name) => {}
            GameEvent::MonsterArrives { name } => self.here.push(name.clone()),
            GameEvent::MonsterLeaves { name } => {
                remove_first(&mut self.here, |n| n == name);
                remove_first(&mut self.attackers, |n| n == name);
            }
            GameEvent::AttackedBy { name } if !self.attackers.contains(name) => {
                self.attackers.push(name.clone());
            }
            GameEvent::MonsterDeath { monsters } => {
                let dead = |n: &String| policy::is_one_of(&atlas.monsters, n, monsters);
                // Who died is the one attacking, if one of them fits.
                if let Some(i) = self.attackers.iter().position(dead) {
                    let name = self.attackers.remove(i);
                    remove_first(&mut self.here, |n| *n == name);
                } else {
                    remove_first(&mut self.here, dead);
                }
            }
            GameEvent::YouKilled => {
                self.here.clear();
                self.attackers.clear();
            }
            _ => {}
        }
    }
}

impl Presence {
    /// Who a monster's death text (`monsters`: every monster that dies with it) is about: the
    /// one attacking the character, if one of them fits, else the first here that does. Many
    /// monsters share a death text, so its list alone does not say.
    pub fn dead(&self, atlas: &Atlas, monsters: &[String]) -> Option<String> {
        let dead = |n: &&String| policy::is_one_of(&atlas.monsters, n, monsters);
        self.attackers
            .iter()
            .find(dead)
            .or_else(|| self.here.iter().find(dead))
            .cloned()
    }

    /// A PvP line showed `who` attacking the character (`attacks_you` from a player, or
    /// `pvp_attacked` naming the character).
    pub fn pvp(&mut self, who: &str) {
        if !self.pvp.iter().any(|p| p == who) {
            self.pvp.push(who.to_owned());
        }
    }

    /// Whether `name` is a player's: one listed as a player in the room (or that name and
    /// more: a blow's pattern can take the verb in, `Bob swings`), or a single capitalised
    /// word no monster is called; never a monster's name.
    pub fn is_player(&self, atlas: &Atlas, name: &str) -> bool {
        let listed = |p: &String| {
            name.strip_prefix(p.as_str())
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
        };
        let looks = self.players.iter().any(listed)
            || (!name.contains(' ') && name.starts_with(|c: char| c.is_ascii_uppercase()));
        looks && atlas.monsters.by_name(name).is_empty()
    }
}

fn remove_first(v: &mut Vec<String>, f: impl FnMut(&String) -> bool) {
    if let Some(i) = v.iter().position(f) {
        v.remove(i);
    }
}

/// What the modes see.
pub(crate) struct View<'a> {
    pub atlas: &'a Atlas,
    pub now: Instant,
    /// HP in per cent of the maximum, when both are known.
    pub hp: Option<i32>,
    /// In combat, or not known to be out of it.
    pub engaged: bool,
    pub resting: bool,
    pub poisoned: bool,
    /// The believed room, and whether the belief is sure of it.
    pub room: Option<Key>,
    pub certain: bool,
    pub traveller: &'a Traveller,
    /// The doors as the game showed them this session.
    pub doors: &'a BTreeMap<(Key, u8), LockState>,
    pub presence: &'a Presence,
    /// What the character is in a party (`party.rs`).
    pub party: &'a crate::party::PartyRole,
    /// The monster rules.
    pub rules: &'a MonsterRules,
    /// Where the monsters to avoid were last seen.
    pub sightings: &'a Sightings,
    /// The session's settings.
    pub settings: &'a wayfarer_settings::Settings,
    /// Mana now and at most, and the character's level; the attack and heal spells it knows.
    pub mana: Option<(i64, i64)>,
    pub hp_missing: Option<i64>,
    /// HP now and at most, in points.
    pub hp_points: Option<(i64, i64)>,
    pub level: i64,
    pub book: &'a Spellbook,
    /// What the character carries, in weight units now and at most (`i`'s `Encumbrance:`),
    /// and the coins it holds.
    pub load: Option<(u64, u64)>,
    pub coins: &'a [wayfarer_proto::CoinCount],
    /// How fast it gets HP and mana back (C-045).
    pub regen: crate::regen::Regen,
}

/// Who a heal reaches, for [`View::heal_pays`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Healed<'a> {
    Me,
    /// A party member, by the name a heal is cast at.
    Mate(&'a str),
    /// The whole party in the room, the caster too.
    All,
}

impl View<'_> {
    /// Whether the settings want this coin picked up (`cash.silver`, ...).
    pub fn wants_coin(&self, word: &str) -> bool {
        match word {
            "silver" | "gold" | "platinum" | "runic" => self.settings.bool(&format!("cash.{word}")),
            _ => false,
        }
    }

    /// How many coins of this kind the character holds.
    pub fn carried(&self, word: &str) -> u64 {
        self.coins
            .iter()
            .find(|c| crate::loot::coin_word(&c.coin) == word)
            .map_or(0, |c| c.count)
    }

    /// The command word that starts a fight (`combat.attack_command`).
    pub fn attack_command(&self) -> &'static str {
        self.settings.choice("combat.attack_command")
    }

    /// Rooms Flee runs through while followed (`health.flee_rooms`).
    pub fn flee_rooms(&self) -> u32 {
        u32::try_from(self.settings.int("health.flee_rooms")).unwrap_or(crate::policy::FLEE_HOPS)
    }

    /// Cast a heal while recovering below this per cent HP (`spells.heal_resting_below_hp`,
    /// 0 is off).
    pub fn rest_heal_below(&self) -> i32 {
        self.setting("spells.heal_resting_below_hp", 60)
    }

    /// The most attack casts at one monster before the weapon takes over (0: no limit).
    pub fn max_casts(&self) -> u32 {
        u32::try_from(self.settings.int("spells.max_casts_per_monster")).unwrap_or(0)
    }

    /// Mana in per cent of the maximum, when both are known.
    pub fn mana_percent(&self) -> Option<i32> {
        self.mana
            .filter(|&(_, max)| max > 0)
            .map(|(m, max)| i32::try_from(m * 100 / max).unwrap_or(100))
    }

    pub fn casts_attacks(&self) -> bool {
        self.book.has_attack() && self.settings.choice("spells.attack") == "auto"
    }

    pub fn casts_heals(&self) -> bool {
        self.book.has_heal() && self.settings.choice("spells.heal") == "auto"
    }

    /// The attack spell for the next fight, if the mana allows one.
    pub fn attack_spell(&self) -> Option<&SpellDef> {
        let (mana, _) = self.mana?;
        if !self.casts_attacks()
            || self.mana_percent()? <= self.setting("spells.attack_mana_above", 15)
        {
            return None;
        }
        self.book.attack(self.level, mana)
    }

    /// The heal for the HP that are missing, if there is a spell and the mana for one.
    pub fn heal_spell(&self) -> Option<&SpellDef> {
        let (mana, _) = self.mana?;
        if !self.casts_heals() {
            return None;
        }
        self.book.heal(self.level, mana, self.hp_missing?)
    }

    /// The heal to cast for the party now (C-043), members healed below `member_line` and the
    /// character itself below `own_line` (counted for a party heal only; its own heal is its
    /// self-heal's): none when the line is 0 or healing spells are off.
    pub fn party_heal(&self, member_line: i32, own_line: i32) -> Option<PartyCast<'_>> {
        if member_line == 0 {
            return None;
        }
        self.heal_for(|_| member_line, own_line)
    }

    /// The party heal between fights (Recover). In a party rest (C-045) each member is healed
    /// up to its own rest-until line, the caster too, and only when the cast does not make the
    /// rest longer ([`View::heal_pays`]); out of one, below the heal-a-member line (C-043).
    pub fn rest_party_heal(&self) -> Option<PartyCast<'_>> {
        if self.party.party_rest().is_none() {
            return self.party_heal(self.member_heal_resting(), self.rest_heal_below());
        }
        if self.member_heal_resting() == 0 {
            return None;
        }
        let cast = self.heal_for(|m| m.until, self.rest_until())?;
        let who = match &cast {
            PartyCast::On(s, name) => (s, Healed::Mate(name)),
            PartyCast::Party(s) => (s, Healed::All),
        };
        self.heal_pays(who.0, who.1).then_some(cast)
    }

    /// The HP line below which Recover heals the character itself: its rest-until line in a
    /// party rest (C-045), else `spells.heal_resting_below_hp`.
    pub fn own_heal_below(&self) -> i32 {
        let line = self.rest_heal_below();
        if line > 0 && self.party.party_rest().is_some() {
            line.max(self.rest_until())
        } else {
            line
        }
    }

    /// Whether casting `spell` on `who` now shortens the party rest, or at least does not
    /// lengthen it: the rest lasts until every member with the caster has its HP (at the
    /// resting rate) and its mana back at its rest-until lines, and the cast spends the
    /// caster's mana to take HP off someone's wait. Out of a party rest, always. With a rate
    /// not known, the cast must leave the mana above the caster's rest line.
    pub fn heal_pays(&self, spell: &SpellDef, who: Healed) -> bool {
        if self.party.party_rest().is_none() {
            return true;
        }
        let Some((mana, max)) = self.mana else {
            return false;
        };
        let after = mana - spell.mana;
        let fallback =
            || after * 100 >= max * i64::from(self.setting("health.rest_mana_below", 30));
        let heal = spell.average(self.level) as i64;
        let mana_until = max * i64::from(self.setting("health.rest_mana_until", 90)) / 100;
        let (Some(mana_rate), Some((hp, hp_max))) = (self.regen.mana, self.hp_points) else {
            return fallback();
        };
        let mana_wait = |m: i64| crate::regen::wait_secs(mana_until - m, mana_rate);
        let me = crate::regen::Waiting {
            hp_short: (hp_max * i64::from(self.rest_until()) / 100 - hp).max(0),
            hp_rate: self.regen.hp,
            mana_before: mana_wait(mana),
            mana_after: mana_wait(after),
            healed: matches!(who, Healed::Me | Healed::All),
        };
        let party: Vec<crate::regen::Waiting> = std::iter::once(me)
            .chain(self.party.mates().iter().map(|m| crate::regen::Waiting {
                hp_short: m.short,
                hp_rate: m.hp_rate,
                mana_before: m.mana_wait,
                mana_after: m.mana_wait,
                healed: match who {
                    Healed::All => true,
                    Healed::Me => false,
                    Healed::Mate(n) => n == m.name,
                },
            }))
            .collect();
        crate::regen::shortens(&party, heal).unwrap_or_else(fallback)
    }

    /// The heal for the party with each member's line from `member_line`, the caster's
    /// `own_line` (C-043's choice, `Spellbook::for_party`).
    fn heal_for(
        &self,
        member_line: impl Fn(&crate::party::Mate) -> i32,
        own_line: i32,
    ) -> Option<PartyCast<'_>> {
        if self.settings.choice("spells.heal") != "auto"
            || !self.book.heals_others()
            || self.party.mates().is_empty()
        {
            return None;
        }
        let (mana, _) = self.mana?;
        let mut hurt: Vec<Hurt> = self
            .party
            .mates()
            .iter()
            .map(|m| Hurt {
                name: Some(&m.name),
                hp: m.hp,
                missing: m.missing,
                line: member_line(m),
                fleeing: m.flee,
            })
            .collect();
        if let (Some(hp), Some(missing)) = (self.hp, self.hp_missing) {
            hurt.push(Hurt {
                name: None,
                hp,
                missing,
                line: own_line,
                fleeing: false,
            });
        }
        self.book.for_party(
            self.level,
            mana,
            &hurt,
            self.settings.choice("party.heals") == "auto",
        )
    }

    /// `party.heal_member_in_combat_below_hp` and `party.heal_member_resting_below_hp`.
    pub fn member_heal_in_combat(&self) -> i32 {
        self.setting("party.heal_member_in_combat_below_hp", 50)
    }

    pub fn member_heal_resting(&self) -> i32 {
        self.setting("party.heal_member_resting_below_hp", 60)
    }

    /// Mana matters to this character: it casts what it knows.
    pub fn uses_mana(&self) -> bool {
        self.mana.is_some() && (self.casts_attacks() || self.casts_heals())
    }

    /// Mana is low enough to rest for (`health.rest_mana_below`).
    pub fn mana_low(&self) -> bool {
        self.uses_mana()
            && self
                .mana_percent()
                .is_some_and(|m| m < self.setting("health.rest_mana_below", 30))
    }

    /// Mana is back to where resting for it stops (`health.rest_mana_until`).
    pub fn mana_restored(&self) -> bool {
        !self.uses_mana()
            || self
                .mana_percent()
                .is_none_or(|m| m >= self.setting("health.rest_mana_until", 90))
    }

    /// Cast a heal in a fight below this per cent of HP (`spells.heal_in_combat_below_hp`).
    pub fn heal_in_combat_below(&self) -> i32 {
        self.setting("spells.heal_in_combat_below_hp", 50)
    }

    /// Recover takes control below this much HP (`health.rest_below_hp`, per cent).
    pub fn rest_below(&self) -> i32 {
        self.setting("health.rest_below_hp", crate::policy::REST_BELOW)
    }

    /// Recover rests until this much HP (`health.rest_until_hp`, per cent).
    pub fn rest_until(&self) -> i32 {
        self.setting("health.rest_until_hp", crate::policy::REST_UNTIL)
    }

    fn setting(&self, key: &str, default: i32) -> i32 {
        i32::try_from(self.settings.int(key)).unwrap_or(default)
    }

    pub fn room_name(&self, k: Option<Key>) -> String {
        k.and_then(|k| self.atlas.graph.room(k))
            .map_or_else(|| "this room".to_owned(), |r| r.name.clone())
    }

    /// The monster to avoid last seen in room `k`, if one was: the bot keeps out.
    pub fn shunned(&self, k: Key) -> Option<&str> {
        self.sightings.at(k, self.now)
    }
}

/// What a mode wants done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Do {
    /// Through the executor.
    Send(Action),
    /// Through the walker, step by confirmed step.
    Walk(Key),
}

/// A mode's decision: what to do (nothing, to wait), and the narration, said as a warning
/// when `warn`.
#[derive(Debug)]
pub(crate) struct Step {
    pub act: Option<Do>,
    pub narration: String,
    pub warn: bool,
    /// What it is doing, in a few words, when the mode's own default does not say it
    /// (C-046): a heal says whom.
    pub doing: Option<String>,
}

impl Step {
    pub fn wait(narration: impl Into<String>) -> Step {
        Step {
            act: None,
            narration: narration.into(),
            warn: false,
            doing: None,
        }
    }

    pub fn send(action: Action, narration: impl Into<String>) -> Step {
        Step {
            act: Some(Do::Send(action)),
            narration: narration.into(),
            warn: false,
            doing: None,
        }
    }

    pub fn walk(to: Key, narration: impl Into<String>) -> Step {
        Step {
            act: Some(Do::Walk(to)),
            narration: narration.into(),
            warn: false,
            doing: None,
        }
    }

    /// The narration is a warning: the mode is doing something its rules advise against.
    pub fn warning(self) -> Step {
        Step { warn: true, ..self }
    }

    /// Say what it is doing in a few words ("healing lauren").
    pub fn doing(self, doing: impl Into<String>) -> Step {
        Step {
            doing: Some(doing.into()),
            ..self
        }
    }
}

/// One mode of the stack.
pub(crate) trait Mode {
    /// Why it wants control, if it does.
    fn wants(&self, v: &View) -> Option<String>;
    /// It has control, nothing is in flight and the game is at a prompt: what next.
    fn step(&mut self, v: &View) -> Step;
    /// An event from the game, whether or not the mode has control.
    fn on_event(&mut self, ev: &GameEvent, now: Instant);
    /// The outcome of an action this mode asked for.
    fn outcome(&mut self, action: &Action, outcome: Outcome);
}

#[derive(Debug, Default)]
pub(crate) struct Bot {
    state: BotState,
    mode: BotMode,
    narration: String,
    /// What it is doing, in a few words (C-046).
    doing: Option<String>,
    reason: Option<String>,
    flee: Flee,
    fight: Fight,
    loot: Loot,
    recover: Recover,
    /// The gateway is logging the character off under attack: Flee alone runs, then the bot
    /// stops.
    leaving: bool,
    /// The mode whose action is in flight.
    acting: Option<BotMode>,
    /// Pause when the action in flight is done, and why.
    pausing: Option<String>,
    /// The loop it runs (Task mode).
    task: Option<Task>,
    /// Monsters a stalemate gave up, for the session to make *never fight*.
    learned: Vec<String>,
}

impl Bot {
    /// A bot whose flee line is `flee_below` per cent (the live check's `--flee-below`).
    pub fn new(flee_below: i32) -> Bot {
        Bot {
            flee: Flee::new(flee_below),
            ..Bot::default()
        }
    }

    /// Flee below `percent` HP from now on (the settings changed: the class became known).
    pub fn set_flee_below(&mut self, percent: i32) {
        self.flee.set_below(percent);
    }

    pub fn running(&self) -> bool {
        self.state == BotState::Running
    }

    pub fn stopped(&self) -> bool {
        self.state == BotState::Stopped
    }

    /// Running, and not about to pause.
    pub fn active(&self) -> bool {
        self.running() && self.pausing.is_none()
    }

    /// The wire's `bot`.
    pub fn snapshot(&self) -> wayfarer_proto::Bot {
        wayfarer_proto::Bot {
            state: self.state,
            mode: self.mode,
            narration: self.narration.clone(),
            doing: self.doing.clone().filter(|_| self.running()),
            reason: self.reason.clone(),
            task: self.task.as_ref().map(Task::snapshot),
            // The session's: it keeps the rules.
            monster_rules: Vec::new(),
        }
    }

    fn narrate(&mut self, text: String, says: &mut Vec<Say>) {
        self.narrate_as(text, false, says);
    }

    fn narrate_as(&mut self, text: String, warn: bool, says: &mut Vec<Say>) {
        if text != self.narration {
            says.push(if warn {
                Say::Warning(text.clone())
            } else {
                Say::Notice(text.clone())
            });
            self.narration = text;
        }
    }

    /// `start`: play from `home`, the room the character is in. The session re-syncs first.
    pub fn start(&mut self, home: Option<Key>, room_name: &str) -> Vec<Say> {
        let mut says = Vec::new();
        self.state = BotState::Running;
        self.mode = BotMode::Idle;
        self.reason = None;
        self.pausing = None;
        self.leaving = false;
        self.flee.restart();
        self.fight = Fight::default();
        self.recover = Recover::new(home);
        self.task = None;
        self.narrate(format!("Starting in {room_name}: re-syncing"), &mut says);
        says
    }

    /// `run_loop`: run `task`, from wherever the character is (replacing a "fight here"). The
    /// session re-syncs first.
    pub fn run_loop(&mut self, task: Task, loop_name: &str) -> Vec<Say> {
        let mut says = Vec::new();
        self.state = BotState::Running;
        self.mode = BotMode::Idle;
        self.reason = None;
        self.pausing = None;
        self.leaving = false;
        self.flee.restart();
        self.fight = Fight::default();
        self.recover = Recover::new(Some(task.home()));
        self.task = Some(task);
        self.narrate(format!("Running loop {loop_name}: re-syncing"), &mut says);
        says
    }

    /// Made a follower of `leader`'s party: the loop it runs, if any, stops (the bot runs on
    /// as "fight here"); the party goes where the leader takes it.
    pub fn join_party(&mut self, leader: &str) -> Vec<Say> {
        let mut says = Vec::new();
        if self.task.take().is_some() {
            self.narrate(format!("In {leader}'s party: the loop stops"), &mut says);
        }
        says
    }

    /// The monster it fights, while it fights one.
    pub fn fight_target(&self) -> Option<&str> {
        (self.mode == BotMode::Fight)
            .then(|| self.fight.target())
            .flatten()
    }

    /// Its flee line, per cent of the maximum HP.
    pub fn flee_below(&self) -> i32 {
        self.flee.below()
    }

    /// The party's leader is to run now, for a member under its flee line (C-042).
    pub fn party_flee(&mut self, why: &str) {
        self.flee.run_from(format!("Fleeing with the party: {why}"));
    }

    /// The party's leader gave up waiting for a member: the loop stops (the bot fights on
    /// where it is), said as a warning.
    pub fn stop_loop(&mut self, why: &str) -> Vec<Say> {
        let mut says = Vec::new();
        if self.task.take().is_some() {
            self.narrate_as(format!("Loop stopped: {why}"), true, &mut says);
        }
        says
    }

    /// The loop it runs, if it runs one.
    pub fn task_mut(&mut self) -> Option<&mut Task> {
        self.task.as_mut()
    }

    /// The walk in progress is a leg of Task's.
    pub fn task_walking(&self) -> bool {
        self.acting == Some(BotMode::Task)
    }

    /// Whether Fight would take the character here: a monster it would attack, or one
    /// attacking.
    pub fn would_fight(&self, atlas: &Atlas, rules: &MonsterRules, presence: &Presence) -> bool {
        self.fight.would_fight(atlas, rules, presence)
    }

    /// A monster to flee from is in the room (or a stalemate wants out), and Flee would take
    /// control now: what the bot has in flight gives way, as for the flee line.
    pub fn flee_urgent(
        &self,
        rules: &MonsterRules,
        presence: &Presence,
        room: Option<Key>,
    ) -> bool {
        self.active()
            && self.acting != Some(BotMode::Flee)
            && self.flee.sighted(rules, presence, room)
    }

    /// The monsters stalemates gave up since last asked: the session makes them *never
    /// fight*.
    pub fn take_learned(&mut self) -> Vec<String> {
        std::mem::take(&mut self.learned)
    }

    /// Fight gave up `name`: say so, and leave as Flee does.
    fn stalemate(&mut self, name: String, says: &mut Vec<Say>) {
        self.narrate(
            format!("Stalemate: my swings do nothing to {name}; leaving"),
            says,
        );
        if self.running() {
            self.flee
                .run_from(format!("Fleeing: stalemate with {name}"));
        }
        self.learned.push(name);
    }

    /// What the store keeps of the bot.
    pub fn record(&self) -> BotRecord {
        let state = match self.state {
            // The run ends with the log-off; a brain restarted meanwhile does not resume it.
            _ if self.leaving => wayfarer_store::BotState::Stopped,
            BotState::Stopped => wayfarer_store::BotState::Stopped,
            BotState::Running => wayfarer_store::BotState::Running,
            BotState::Paused => wayfarer_store::BotState::Paused,
        };
        let task = self.task.as_ref().map(Task::stored).unwrap_or_default();
        BotRecord {
            state,
            home: self.recover.home(),
            loop_id: self.task.as_ref().map(|t| t.id().to_owned()),
            loop_stops: task.stops,
            stop_index: task.index,
            skip: task.skip,
            paused_reason: self
                .reason
                .clone()
                .filter(|_| self.state == BotState::Paused),
            // The session's: it keeps the rules.
            monster_rules: Vec::new(),
        }
    }

    /// A restarted brain puts the bot back as the store had it (`running` or `paused`), with
    /// its home and loop, narrating `narration`. The session re-syncs first.
    pub fn restore(&mut self, rec: &BotRecord, task: Option<Task>, narration: String) -> Vec<Say> {
        let mut says = Vec::new();
        self.state = match rec.state {
            wayfarer_store::BotState::Paused => BotState::Paused,
            _ => BotState::Running,
        };
        self.mode = BotMode::Idle;
        self.reason = rec
            .paused_reason
            .clone()
            .filter(|_| self.state == BotState::Paused);
        self.pausing = None;
        self.leaving = false;
        self.flee.restart();
        self.fight = Fight::default();
        self.recover = Recover::new(task.as_ref().map(Task::home).or(rec.home));
        self.task = task;
        self.narrate(narration, &mut says);
        says
    }

    /// `stop`, or the bot stopping itself (`warning`).
    pub fn stop(&mut self, why: &str, warning: bool) -> Vec<Say> {
        let mut says = Vec::new();
        if self.state == BotState::Stopped {
            return says;
        }
        self.state = BotState::Stopped;
        self.mode = BotMode::Idle;
        self.pausing = None;
        self.task = None;
        self.reason = Some(why.to_owned());
        // "Stopped at 24% HP: ...", "Stopped: asked to stop".
        let sep = if why.starts_with("at ") { " " } else { ": " };
        self.narration = format!("Stopped{sep}{why}");
        says.push(if warning {
            Say::Warning(self.narration.clone())
        } else {
            Say::Notice(self.narration.clone())
        });
        says
    }

    /// `logging_off` with the character attacked: Flee runs alone, whatever the HP and whether
    /// the bot was running, paused or stopped, to a room where nothing attacks; then the bot
    /// stops ("the gateway is logging off").
    pub fn leave(&mut self) -> Vec<Say> {
        let mut says = Vec::new();
        self.state = BotState::Running;
        self.mode = BotMode::Idle;
        self.reason = None;
        self.pausing = None;
        self.leaving = true;
        self.task = None;
        self.flee.leave();
        self.narrate(
            "Logging off under attack: fleeing first".to_owned(),
            &mut says,
        );
        says
    }

    /// Fleeing before the gateway logs the character off.
    pub fn leaving(&self) -> bool {
        self.leaving && self.running()
    }

    /// The HP a flee started at, once it has: for the run's statistics.
    pub fn take_fled(&mut self) -> Option<i32> {
        self.flee.take_started()
    }

    /// Pause after the action in flight (`busy`), or now.
    pub fn pause(&mut self, why: &str, busy: bool) -> Vec<Say> {
        let mut says = Vec::new();
        if self.state != BotState::Running || self.pausing.is_some() {
            return says;
        }
        if busy {
            self.pausing = Some(why.to_owned());
        } else {
            self.paused(why, &mut says);
        }
        says
    }

    fn paused(&mut self, why: &str, says: &mut Vec<Say>) {
        self.state = BotState::Paused;
        self.pausing = None;
        self.reason = Some(why.to_owned());
        self.narrate(format!("Paused: {why}"), says);
    }

    /// `resume`: the session re-syncs first.
    pub fn resume(&mut self) -> Vec<Say> {
        let mut says = Vec::new();
        if self.state != BotState::Paused {
            return says;
        }
        self.state = BotState::Running;
        self.reason = None;
        self.narrate("Resuming: re-syncing".to_owned(), &mut says);
        says
    }

    /// An event from the game: the modes follow it whatever the state; a death stops the bot.
    pub fn event(&mut self, ev: &GameEvent, now: Instant) -> Vec<Say> {
        self.flee.on_event(ev, now);
        self.fight.on_event(ev, now);
        self.loot.on_event(ev, now);
        self.recover.on_event(ev, now);
        if let Some(t) = &mut self.task {
            t.on_event(ev, now);
        }
        let mut says = Vec::new();
        if let Some(name) = self.fight.take_stalemate() {
            self.stalemate(name, &mut says);
        }
        if let GameEvent::YouKilled = ev {
            says.extend(self.stop("the character was killed", true));
        }
        says
    }

    /// A monster died (`name`, as the room showed it): Task counts it for the stop it is at.
    pub fn killed(&mut self, name: &str, now: Instant) {
        if let Some(t) = &mut self.task {
            t.killed(name, now);
        }
    }

    /// An HP value, wherever it came from (pitfall 27: every one is checked, mid-walk and
    /// mid-action too). Whether Flee takes control now: then what the bot has in flight gives
    /// way (its walk stops after the move in flight; an action not sent yet is dropped).
    pub fn hp(&mut self, hp: Option<i32>) -> bool {
        self.flee.hp(hp);
        self.active() && self.acting != Some(BotMode::Flee) && self.flee.urgent(hp)
    }

    /// The outcome of the bot's action: to the mode that asked for it. A pause asked for
    /// meanwhile takes effect now.
    pub fn outcome(&mut self, action: &Action, outcome: Outcome) -> Vec<Say> {
        match self.acting.take() {
            Some(BotMode::Flee) => self.flee.outcome(action, outcome),
            Some(BotMode::Fight) => self.fight.outcome(action, outcome),
            Some(BotMode::Recover) => self.recover.outcome(action, outcome),
            Some(BotMode::Loot) => self.loot.outcome(action, outcome),
            _ => {}
        }
        // Flee broke off the fight it could not run from.
        if let Some(why) = self.flee.take_give_up() {
            return self.stop(&why, true);
        }
        self.settle()
    }

    /// The action in flight is done (the bot's or another's): a pending pause takes effect.
    pub fn settle(&mut self) -> Vec<Say> {
        let mut says = Vec::new();
        if let Some(why) = self.pausing.take() {
            self.paused(&why, &mut says);
        }
        says
    }

    /// When a wait ends, for the clock to wake the bot.
    pub fn wake(&self, now: Instant) -> Option<Instant> {
        self.recover
            .wake(now)
            .into_iter()
            .chain(self.flee.wake())
            .chain(self.fight.wake())
            .chain(self.task.as_ref().and_then(|t| t.wake(now)))
            .min()
    }

    /// The bot's walk ended, or its action was dropped, with no outcome to hand on: the mode
    /// decides again at the next tick.
    pub fn forget(&mut self) {
        self.acting = None;
    }

    /// What a mode is doing when its step does not say (C-046).
    fn default_doing(&self, mode: BotMode, step: &Step, v: &View) -> String {
        let walking = matches!(step.act, Some(Do::Walk(_)));
        match mode {
            BotMode::Flee => "fleeing".to_owned(),
            BotMode::Fight => match self.fight.target() {
                Some(t) => format!("attacking {t}"),
                None => "looking for a fight".to_owned(),
            },
            BotMode::Recover if walking => "walking to rest".to_owned(),
            BotMode::Recover if v.party.party_rest().is_some() => {
                "resting with the party".to_owned()
            }
            BotMode::Recover => "resting".to_owned(),
            BotMode::Loot => "picking up".to_owned(),
            BotMode::Task if walking => match v.party.following() {
                Some(leader) if v.party.rejoin().is_some() => format!("walking back to {leader}"),
                _ => "walking".to_owned(),
            },
            BotMode::Task | BotMode::Idle => match (v.party.following(), v.party.hold()) {
                (Some(leader), _) => format!("following {leader}"),
                (None, Some(_)) => "waiting for the party".to_owned(),
                _ => "waiting".to_owned(),
            },
        }
    }

    /// Decide: nothing is in flight, nothing is walking, and the game is at a prompt.
    pub fn tick(&mut self, v: &View) -> (Option<Do>, Vec<Say>) {
        let mut says = Vec::new();
        if self.state != BotState::Running || self.pausing.is_some() {
            return (None, says);
        }
        // A round of swings that is over counts now: it may be the stalemate's last.
        self.fight.settle(v.now);
        if let Some(name) = self.fight.take_stalemate() {
            self.stalemate(name, &mut says);
        }
        self.fight
            .set_cornered(self.flee.trapped_in(v.room) || self.flee.cornered_in(v.room));
        if let Some(t) = &mut self.task {
            t.observe(v);
            // Recover rests from, and walks back to, the stop Task is at or walking to.
            self.recover.set_home(t.home());
        }
        if !self.leaving
            && let Some(home) = self.recover.find_home(v)
        {
            let name = v.room_name(Some(home));
            self.narrate(format!("Playing from {name}"), &mut says);
        }
        // A mode that finishes in its step (safe, rested, back home) hands over at once: the
        // stack is asked again, once.
        for _ in 0..2 {
            let (mode, reason) = if let Some(r) = self.flee.wants(v) {
                (BotMode::Flee, r)
            } else if self.leaving {
                says.extend(self.stop("the gateway is logging off", false));
                return (None, says);
            } else if let Some(r) = self.loot.wants(v) {
                (BotMode::Loot, r)
            } else if let Some(r) = self.recover.wants(v) {
                (BotMode::Recover, r)
            } else if let Some(r) = self.fight.wants(v) {
                (BotMode::Fight, r)
            } else if v.party.rejoin().is_some_and(|k| v.room != Some(k)) {
                // Out of its party: back to the leader, the one walk a follower makes.
                (BotMode::Task, "walking back to the party".to_owned())
            } else if let Some(r) = self.task.as_ref().and_then(|t| t.wants(v)) {
                (BotMode::Task, r)
            } else {
                (BotMode::Idle, "nothing to do".to_owned())
            };
            if mode != self.mode {
                tracing::info!(?mode, %reason, "the bot's mode");
                self.mode = mode;
            }
            let step = match mode {
                BotMode::Flee => self.flee.step(v),
                BotMode::Recover => self.recover.step(v),
                BotMode::Loot => self.loot.step(v),
                BotMode::Fight => self.fight.step(v),
                BotMode::Task if let Some(to) = v.party.rejoin().filter(|k| v.room != Some(*k)) => {
                    let leader = v.party.following().unwrap_or("the leader");
                    Step::walk(to, format!("Walking back to {leader}"))
                }
                BotMode::Task => {
                    let task = self.task.as_mut().expect("Task has a loop");
                    let step = task.step(v);
                    let (notes, rest) = (task.take_notes(), task.take_rest());
                    for note in notes {
                        self.narrate(note, &mut says);
                    }
                    if rest {
                        self.recover.after_flee();
                    }
                    step
                }
                BotMode::Idle if self.recover.stranded(v) => {
                    Step::wait("Rested; could not walk home, waiting here")
                }
                BotMode::Idle if let Some((home, name)) = self.recover.shunned(v) => {
                    Step::wait(format!(
                        "Waiting here: {name} was last seen in {}",
                        v.room_name(Some(home))
                    ))
                }
                BotMode::Idle => Step::wait("Waiting here"),
            };
            // A follower goes where its leader takes it: none of its own modes walks it, Flee
            // aside (docs/plan/10-party-play.md §5).
            let rejoining = v.party.rejoin().is_some() && mode == BotMode::Task;
            let step = match (&step.act, v.party.following(), v.party.hold()) {
                (Some(Do::Walk(_)), Some(leader), _) if mode != BotMode::Flee && !rejoining => {
                    Step::wait(format!("Following {leader}: staying with the party"))
                        .doing(format!("following {leader}"))
                }
                // A leader holds its loop while a member walks back (C-041).
                (Some(Do::Walk(_)), _, Some(why)) if mode == BotMode::Task => {
                    Step::wait(why.to_owned()).doing("waiting for the party")
                }
                _ => step,
            };
            let finished = step.act.is_none()
                && match mode {
                    BotMode::Flee => !self.flee.busy(),
                    BotMode::Recover => !self.recover.busy(),
                    _ => false,
                };
            if step.act.is_some() {
                self.acting = Some(mode);
            }
            self.doing = Some(
                step.doing
                    .clone()
                    .unwrap_or_else(|| self.default_doing(mode, &step, v)),
            );
            if finished && mode == BotMode::Flee && !self.leaving {
                self.recover.after_flee();
            }
            self.narrate_as(step.narration, step.warn, &mut says);
            // Away from (or cornered by) a monster it does not fight: the loop goes on from the
            // next stop, and Recover walks there, not back.
            if mode == BotMode::Flee
                && self.flee.take_forced_done()
                && let Some(t) = &mut self.task
            {
                t.move_on(v);
                self.recover.set_home(t.home());
                for note in t.take_notes() {
                    self.narrate(note, &mut says);
                }
            }
            if step.act.is_none()
                && let Some(why) = self
                    .flee
                    .take_give_up()
                    .or_else(|| self.task.as_mut().and_then(Task::take_give_up))
            {
                says.extend(self.stop(&why, true));
                return (None, says);
            }
            if !finished {
                return (step.act, says);
            }
        }
        (None, says)
    }
}
