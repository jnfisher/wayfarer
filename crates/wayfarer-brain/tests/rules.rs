//! The monster rules (docs/brain.md "Monster rules"): *never fight*, *flee from* and *avoid* by
//! name, and a stalemate ending a fight. Against the fake gateway of `game/mod.rs`, in Newhaven
//! (the Arena and its loop), and with the gigantic black ooze's rounds as the M3 soak recorded
//! them (`tests/fixtures/ooze/`). On a paused clock.

mod game;

use std::path::Path;
use std::time::UNIX_EPOCH;

use game::arena::*;
use game::common::repo;
use game::*;
use wayfarer_brain::{Brain, FixedClock, InstantClock, STALEMATE_ROUNDS, Store};
use wayfarer_proto::{BotAction, BotMode, BotState, MonsterRuleEntry, MonsterRuleKind, RuleSource};

const RAT: &str = "large giant rat";
const THIEF: &str = "fierce kobold thief";
const NEWHAVEN_LOOP: &str = "1:2140:8:doors:0";
/// North of the Dungeon Entrance: the stop the loop tests untick.
const SMALL_CAVERN: Key = (1, 2156);

fn clock() -> Arc<FixedClock> {
    Arc::new(FixedClock(UNIX_EPOCH + Duration::from_secs(T0)))
}

/// A brain whose bots flee from giant rats (`--flee-from "giant rat"`).
fn fleeing_rats() -> Brain {
    Brain::new(world(), clock(), None).with_flee_from(vec!["giant rat".into()])
}

/// `brain` attached in the Arena with `monsters`, the attach's re-sync done.
async fn arena_with(brain: Brain, monsters: &[&str]) -> (Game, Run) {
    let (mut g, run) = Game::start_brain(SessionState::Realm, arena(monsters, 35), brain).await;
    g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
        .await;
    g.idle(Duration::from_secs(20)).await;
    (g, run)
}

impl Game {
    async fn set_rule(&mut self, pattern: &str, rule: Option<MonsterRuleKind>) {
        self.send(ToClient::SetMonsterRule {
            session: ID.into(),
            pattern: pattern.into(),
            rule,
        })
        .await;
    }

    async fn run_loop(&self, skip: &[Key]) {
        self.send(ToClient::RunLoop {
            session: ID.into(),
            id: NEWHAVEN_LOOP.into(),
            skip: Some(skip.iter().map(|&k| key(k)).collect()),
        })
        .await;
    }
}

fn rules_of(seen: &[ToGateway]) -> Vec<MonsterRuleEntry> {
    bot(seen).monster_rules
}

fn entry(pattern: &str, rule: MonsterRuleKind, source: RuleSource) -> MonsterRuleEntry {
    MonsterRuleEntry {
        pattern: pattern.into(),
        rule,
        source,
    }
}

#[tokio::test(start_paused = true)]
async fn a_monster_to_flee_from_is_left_on_sight_at_full_hp_and_never_attacked() {
    let (mut g, run) = arena_with(fleeing_rats(), &[RAT]).await;
    start(&mut g, &[RAT], 35).await;
    g.expect("u", room(NARROW_ROAD));
    g.narrated("Fleeing: large giant rat is here (flee from): up to Newhaven, Narrow Road")
        .await;
    g.narrated("Safe in Newhaven, Narrow Road").await;
    // Not back to the Arena while the rat was last seen there.
    g.narrated("Waiting here: large giant rat was last seen in Newhaven, Arena")
        .await;
    g.idle(Duration::from_secs(60)).await;
    let b = bot(&g.seen);
    assert_eq!((b.state, b.mode), (BotState::Running, BotMode::Idle));
    // Half an hour on, the sighting is forgotten: back to the Arena, and the rat is still
    // there.
    g.expect("d", arena(&[RAT], 35));
    g.expect("u", room(NARROW_ROAD));
    g.narrated("Recover: rested; walking back to Newhaven, Arena")
        .await;
    g.narrated("Fleeing: large giant rat is here (flee from): up to Newhaven, Narrow Road")
        .await;
    g.bot(BotAction::Stop).await;
    g.narrated("Stopped: asked to stop").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "u", "d", "u"]);
    assert!(
        rules_of(&seen).contains(&entry(
            "giant rat",
            MonsterRuleKind::FleeFrom,
            RuleSource::Player
        )),
        "{:?}",
        rules_of(&seen)
    );
}

#[tokio::test(start_paused = true)]
async fn a_monster_to_flee_from_ends_the_stop_and_the_loop_goes_on_from_the_next() {
    let brain = fleeing_rats();
    let (mut g, run) = Game::start_brain(SessionState::Realm, room(VILLAGE_ENTRANCE), brain).await;
    g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
        .await;
    g.pipelining = true;
    g.idle(Duration::from_secs(20)).await;
    g.expect("st", status(35));
    g.expect("look", room(VILLAGE_ENTRANCE));
    g.expect("w", room(NARROW_PATH));
    g.expect("w", room(NARROW_ROAD));
    g.expect("d", arena(&[RAT], 35));
    // At full HP, the rat not attacking: up out of the Arena.
    g.expect("u", room(NARROW_ROAD));
    // On to the next stop, through the Arena (the only way), the rat still there.
    g.expect("d", arena(&[RAT], 35));
    g.expect("n", says("The door is closed!"));
    g.expect("bash n", says("You bashed the door open."));
    g.expect("n", room(BEYOND_ARENA));
    g.run_loop(&[SMALL_CAVERN]).await;
    g.narrated("Loop Newhaven: to stop 1/3 Newhaven, Arena (3 steps)")
        .await;
    g.narrated("Fleeing: large giant rat is here (flee from): up to Newhaven, Narrow Road")
        .await;
    g.narrated("Safe in Newhaven, Narrow Road").await;
    g.narrated("Stop 1/3 Newhaven, Arena: leaving it for now")
        .await;
    g.narrated("Stop 2/3 cleared: nothing there").await;
    g.bot(BotAction::Stop).await;
    g.narrated("Stopped: asked to stop").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(
        played,
        ["st", "look", "w", "w", "d", "u", "d", "n", "bash n", "n"]
    );
    let said = notices(&seen);
    assert!(
        !said.iter().any(|s| s.starts_with("Stop 1/3 cleared")),
        "the Arena is not cleared: {said:?}"
    );
    assert!(!said.iter().any(|s| s.starts_with("Fight:")), "{said:?}");
}

#[tokio::test(start_paused = true)]
async fn a_never_fight_monster_is_not_attacked_and_one_beside_it_is() {
    let brain = Brain::new(world(), clock(), None);
    let (mut g, run) = arena_with(brain, &[RAT, THIEF]).await;
    g.set_rule(RAT, Some(MonsterRuleKind::NeverFight)).await;
    g.narrated("Monster rule: never fight large giant rat")
        .await;
    start(&mut g, &[RAT, THIEF], 35).await;
    g.expect("a thief", line(ENGAGED, 35));
    g.narrated("Fight: fierce kobold thief").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a thief"]);
    assert!(rules_of(&seen).contains(&entry(RAT, MonsterRuleKind::NeverFight, RuleSource::Player)));
}

#[tokio::test(start_paused = true)]
async fn a_never_fight_monster_that_attacks_is_fought() {
    let brain = Brain::new(world(), clock(), None);
    let (mut g, run) = arena_with(brain, &[RAT]).await;
    g.set_rule("giant rat", Some(MonsterRuleKind::NeverFight))
        .await;
    start(&mut g, &[RAT], 35).await;
    g.narrated("Waiting here").await;
    g.expect("a rat", line(ENGAGED, 33));
    g.output(&line("The large giant rat lunges at you!", 33))
        .await;
    g.narrated("Fight: large giant rat").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat"]);
}

/// The character's swing glancing off the rat's armour, a round of the fight.
const GLANCE: &str = "Your swing at large giant rat hits, but glances off its armour.";

#[tokio::test(start_paused = true)]
async fn four_rounds_of_glancing_blows_end_the_fight_and_the_bot_leaves() {
    let brain = Brain::new(world(), clock(), None);
    let (mut g, run) = arena_with(brain, &[RAT]).await;
    start(&mut g, &[RAT], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    // A round with damage first: the count starts after it.
    g.round("You whap large giant rat for 6 damage!", 35).await;
    g.expect("u", room(NARROW_ROAD));
    for round in 1..=STALEMATE_ROUNDS {
        g.output(&format!("{}{}", line(GLANCE, 35), line(GLANCE, 35)))
            .await;
        if round < STALEMATE_ROUNDS {
            g.idle(Duration::from_secs(5)).await;
        }
    }
    assert!(!notices(&g.seen).iter().any(|s| s.starts_with("Stalemate")));
    // The fourth round is counted once its swings are all in.
    g.narrated("Stalemate: my swings do nothing to large giant rat; leaving")
        .await;
    g.narrated("Fleeing: stalemate with large giant rat: up to Newhaven, Narrow Road")
        .await;
    g.narrated("Safe in Newhaven, Narrow Road").await;
    g.bot(BotAction::Stop).await;
    g.narrated("Stopped: asked to stop").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "u"]);
    let said = notices(&seen);
    assert!(
        said.contains(&"Monster rule: never fight large giant rat (a stalemate)".to_owned()),
        "{said:?}"
    );
    assert!(rules_of(&seen).contains(&entry(
        RAT,
        MonsterRuleKind::NeverFight,
        RuleSource::Session
    )));
}

#[tokio::test(start_paused = true)]
async fn a_round_with_damage_resets_the_count() {
    let brain = Brain::new(world(), clock(), None);
    let (mut g, run) = arena_with(brain, &[RAT]).await;
    start(&mut g, &[RAT], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    for _ in 0..3 {
        g.round(GLANCE, 35).await;
    }
    g.round("You whap large giant rat for 6 damage!", 35).await;
    for _ in 0..3 {
        g.round(GLANCE, 35).await;
    }
    // The third round since the damage is counted: still no stalemate.
    g.idle(Duration::from_secs(3)).await;
    g.bot(BotAction::Stop).await;
    g.narrated("Stopped: asked to stop").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat"]);
    assert!(
        !notices(&seen).iter().any(|s| s.starts_with("Stalemate")),
        "{:?}",
        notices(&seen)
    );
}

/// The ooze fight's chunks as recorded (`tests/fixtures/ooze/output.jsonl`): `(seq, text)`.
fn ooze_chunks() -> Vec<(u64, String)> {
    let raw = std::fs::read_to_string(repo("tests/fixtures/ooze/output.jsonl")).unwrap();
    raw.lines()
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).unwrap();
            (
                v["seq"].as_u64().unwrap(),
                v["text"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[tokio::test(start_paused = true)]
async fn the_ooze_is_never_fought_by_default() {
    let chunks = ooze_chunks();
    let passage = chunks[0].1.clone();
    let (mut g, run) = Game::start_lost(SessionState::Realm, passage.clone()).await;
    g.idle(Duration::from_secs(20)).await;
    g.expect("st", status(35));
    g.expect("look", passage);
    g.bot(BotAction::Start).await;
    g.narrated("re-synced").await;
    g.idle(Duration::from_secs(10)).await;
    g.bot(BotAction::Stop).await;
    g.narrated("Stopped: asked to stop").await;
    let (seen, played) = g.close(run).await;
    assert!(!played.iter().any(|c| c.starts_with("a ")), "{played:?}");
    let said = notices(&seen);
    assert!(!said.iter().any(|s| s.starts_with("Fight:")), "{said:?}");
}

#[tokio::test(start_paused = true)]
async fn the_recorded_ooze_fight_ends_by_its_fourth_round() {
    let chunks = ooze_chunks();
    let passage = chunks[0].1.clone();
    let (mut g, run) = Game::start_lost(SessionState::Realm, passage.clone()).await;
    g.idle(Duration::from_secs(20)).await;
    // Fought, as the soak's brain did: the default *flee from* overridden.
    g.set_rule("gigantic black ooze", Some(MonsterRuleKind::Avoid))
        .await;
    g.expect("st", status(35));
    g.expect("look", passage);
    g.expect("a ooze", chunks[1].1.clone());
    // Leaving: which Labyrinth Passage this is the belief cannot tell (the island has two
    // hundred), so there is no exit to flee by; the bot breaks off and stays.
    g.expect("break", line("*Combat Off*", 208));
    g.bot(BotAction::Start).await;
    g.narrated("Fight: gigantic black ooze").await;
    // The recorded rounds, 5 s apart; the bot gives up after the fourth.
    let mut rounds = 0;
    let mut said = Vec::new();
    for (_, text) in &chunks[2..] {
        g.output(text).await;
        rounds += 1;
        let before = g.seen.len();
        g.idle(Duration::from_secs(5)).await;
        said = notices(&g.seen[before..]);
        if said.iter().any(|s| s.starts_with("Stalemate")) {
            break;
        }
    }
    assert_eq!(rounds, STALEMATE_ROUNDS, "{said:?}");
    assert!(
        said.contains(
            &"Stalemate: my swings do nothing to gigantic black ooze; leaving".to_owned()
        ),
        "{said:?}"
    );
    g.bot(BotAction::Stop).await;
    g.narrated("Stopped: asked to stop").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played.iter().filter(|c| c.starts_with("a ")).count(), 1);
    assert!(rules_of(&seen).contains(&entry(
        "gigantic black ooze",
        MonsterRuleKind::NeverFight,
        RuleSource::Session
    )));
}

fn brain_on(path: &Path) -> Brain {
    let (store, _) = Store::open(path).unwrap();
    let clock = InstantClock::new(UNIX_EPOCH + Duration::from_secs(T0));
    Brain::new(world(), Arc::new(clock), None).with_store(store)
}

#[tokio::test(start_paused = true)]
async fn a_players_rule_round_trips_through_the_snapshot_and_survives_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let (mut g, run) = arena_with(brain_on(&path), &[RAT]).await;
    g.set_rule("Giant Rat", Some(MonsterRuleKind::Avoid)).await;
    g.narrated("Monster rule: avoid Giant Rat").await;
    g.set_rule("giant rat", Some(MonsterRuleKind::FleeFrom))
        .await;
    g.narrated("Monster rule: flee from giant rat").await;
    g.set_rule("kobold", Some(MonsterRuleKind::NeverFight))
        .await;
    g.set_rule("kobold", None).await;
    g.narrated("Monster rule removed: kobold").await;
    g.set_rule("x", Some(MonsterRuleKind::Avoid)).await;
    g.narrated("set_monster_rule: the pattern needs two characters or more")
        .await;
    let (seen, _) = g.close(run).await;
    let want = vec![
        entry(
            "gigantic black ooze",
            MonsterRuleKind::FleeFrom,
            RuleSource::Default,
        ),
        entry("Giant Rat", MonsterRuleKind::FleeFrom, RuleSource::Player),
    ];
    assert_eq!(rules_of(&seen), want);

    // A second brain on the same store: the player's rule is still there.
    let (g, run) = arena_with(brain_on(&path), &[RAT]).await;
    let (seen, _) = g.close(run).await;
    assert_eq!(rules_of(&seen), want);
}

#[tokio::test(start_paused = true)]
async fn a_rule_set_while_the_monster_is_here_takes_effect_at_once() {
    let brain = Brain::new(world(), clock(), None);
    let (mut g, run) = arena_with(brain, &[RAT]).await;
    start(&mut g, &[RAT], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.output(&kill(35)).await;
    g.expect("look", arena(&[RAT], 35));
    g.narrated("Fight: looking for the next").await;
    g.expect("u", room(NARROW_ROAD));
    g.set_rule("rat", Some(MonsterRuleKind::FleeFrom)).await;
    g.narrated("Fleeing: large giant rat is here (flee from): up to Newhaven, Narrow Road")
        .await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "look", "u"]);
}
