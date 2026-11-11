//! The monster rules (docs/brain.md "Monster rules"): *ignore* and *avoid* by name, removing
//! any rule (a default is switched off), and a stalemate ending a fight. Against the fake gateway of `game/mod.rs`, in Newhaven
//! (the Arena and its loop), and with the gigantic black ooze's rounds as the M3 soak recorded
//! them (`tests/fixtures/ooze/`). On a paused clock.

mod game;

use std::path::Path;
use std::time::UNIX_EPOCH;

use game::arena::*;
use game::common::repo;
use game::*;
use wayfarer_brain::{Brain, FixedClock, InstantClock, STALEMATE_ROUNDS, Store};
use wayfarer_proto::{BotAction, MonsterRuleEntry, MonsterRuleKind, RuleSource};

const RAT: &str = "large giant rat";
const THIEF: &str = "fierce kobold thief";
const NEWHAVEN_LOOP: &str = "1:2140:8:doors:0";
/// North of the Dungeon Entrance: the stop the loop tests untick.
const SMALL_CAVERN: Key = (1, 2156);

fn clock() -> Arc<FixedClock> {
    Arc::new(FixedClock(UNIX_EPOCH + Duration::from_secs(T0)))
}

/// A brain whose bots ignore giant rats (`--ignore "giant rat"`).
fn ignoring_rats() -> Brain {
    Brain::new(world(), clock(), None).with_ignore(vec!["giant rat".into()])
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

/// The rules the bot shows, without the defaults these tests do not touch: the ooze's stays,
/// MegaMud's (C-096) are left out.
fn rules_of(seen: &[ToGateway]) -> Vec<MonsterRuleEntry> {
    bot(seen)
        .monster_rules
        .into_iter()
        .filter(|e| {
            e.source != RuleSource::Default
                || e.pattern == "gigantic black ooze"
                || !wayfarer_world::DEFAULT_RULES
                    .iter()
                    .any(|(p, _, _)| *p == e.pattern)
        })
        .collect()
}

fn entry(pattern: &str, rule: MonsterRuleKind, source: RuleSource) -> MonsterRuleEntry {
    // A default carries its reason.
    let reason = (source == RuleSource::Default).then(|| {
        wayfarer_world::DEFAULT_RULES
            .iter()
            .find(|(p, _, _)| *p == pattern)
            .map(|(_, _, why)| (*why).to_owned())
            .unwrap_or_default()
    });
    MonsterRuleEntry {
        pattern: pattern.into(),
        rule,
        source,
        reason,
    }
}

/// C-073: no monster is fled from on sight. An ignored one at full HP is left alone where it
/// stands, and the bot stays in the room with it.
#[tokio::test(start_paused = true)]
async fn an_ignored_monster_is_neither_attacked_nor_fled_from() {
    let (mut g, run) = arena_with(ignoring_rats(), &[RAT]).await;
    start(&mut g, &[RAT], 35).await;
    g.narrated("Waiting here").await;
    g.idle(Duration::from_secs(60)).await;
    g.freeze();
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look"]);
    let said = notices(&seen);
    assert!(!said.iter().any(|s| s.starts_with("Flee")), "{said:?}");
    assert!(
        rules_of(&seen).contains(&entry(
            "giant rat",
            MonsterRuleKind::Ignore,
            RuleSource::Player
        )),
        "{:?}",
        rules_of(&seen)
    );
}

/// At a loop stop where only an ignored monster is left, the stop is cleared once the bot's
/// own combat has been quiet a while, and the loop goes on, with no flee.
#[tokio::test(start_paused = true)]
async fn an_ignored_monster_at_a_stop_leaves_it_cleared_and_the_loop_goes_on() {
    let brain = ignoring_rats();
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
    g.expect("n", says("The door is closed!"));
    g.expect("bash n", says("You bashed the door open."));
    g.expect("n", room(BEYOND_ARENA));
    g.run_loop(&[SMALL_CAVERN]).await;
    g.narrated("Loop Newhaven: to stop 1/3 Newhaven, Arena (3 steps)")
        .await;
    g.narrated("Stop 2/3 cleared: nothing there").await;
    g.freeze();
    g.bot(BotAction::Stop).await;
    g.narrated("Stopped loop Newhaven: playing here").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "w", "w", "d", "n", "bash n", "n"]);
    let said = notices(&seen);
    assert!(!said.iter().any(|s| s.starts_with("Flee")), "{said:?}");
    assert!(!said.iter().any(|s| s.starts_with("Fight:")), "{said:?}");
    assert!(
        said.iter().any(|s| s.starts_with("Stop 1/3 cleared")),
        "{said:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn an_ignored_monster_is_not_attacked_and_one_beside_it_is() {
    let brain = Brain::new(world(), clock(), None);
    let (mut g, run) = arena_with(brain, &[RAT, THIEF]).await;
    g.set_rule(RAT, Some(MonsterRuleKind::Ignore)).await;
    g.narrated("Monster rule: ignore large giant rat").await;
    start(&mut g, &[RAT, THIEF], 35).await;
    g.expect("a thief", line(ENGAGED, 35));
    g.narrated("Fight: fierce kobold thief").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a thief"]);
    assert!(rules_of(&seen).contains(&entry(RAT, MonsterRuleKind::Ignore, RuleSource::Player)));
}

/// C-073: an ignored monster that attacks is not fought back, nor fled from above the flee
/// line: the bot says so once and keeps going.
#[tokio::test(start_paused = true)]
async fn an_ignored_monster_that_attacks_is_not_fought_back() {
    let brain = Brain::new(world(), clock(), None);
    let (mut g, run) = arena_with(brain, &[RAT]).await;
    g.set_rule("giant rat", Some(MonsterRuleKind::Ignore)).await;
    start(&mut g, &[RAT], 35).await;
    g.narrated("Waiting here").await;
    g.output(&line("The large giant rat lunges at you!", 33))
        .await;
    g.narrated("Ignoring large giant rat: not fighting back")
        .await;
    g.idle(Duration::from_secs(30)).await;
    g.output(&line("The large giant rat lunges at you!", 31))
        .await;
    g.idle(Duration::from_secs(30)).await;
    g.freeze();
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look"]);
    let said = notices(&seen);
    assert!(!said.iter().any(|s| s.starts_with("Fight")), "{said:?}");
    assert!(!said.iter().any(|s| s.starts_with("Flee")), "{said:?}");
    let ignoring = said.iter().filter(|s| s.starts_with("Ignoring")).count();
    assert_eq!(ignoring, 1, "{said:?}");
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
    // No rule (C-096): the player is told how to make one.
    assert!(
        said.contains(&"Stalemate with large giant rat: its room is kept out of for 30 minutes; a monster rule can ignore it for good".to_owned()),
        "{said:?}"
    );
    assert!(
        !rules_of(&seen).iter().any(|e| e.pattern == RAT),
        "{:?}",
        rules_of(&seen)
    );
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

/// The recorded ooze fight, its eight rounds played over and over, from a started bot; a
/// party member's blow on the ooze before round `party_hit` (if any). The rounds until the
/// stalemate, and what the session saw.
async fn ooze_stalemate(party_hit: Option<u32>) -> (u32, Vec<ToGateway>) {
    let chunks = ooze_chunks();
    let passage = chunks[0].1.clone();
    let (mut g, run) = Game::start_lost(SessionState::Realm, passage.clone()).await;
    g.idle(Duration::from_secs(20)).await;
    // Fought, as the soak's brain did: the default *ignore* overridden.
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
    // The recorded rounds, 5 s apart, over and over until the bot gives up.
    let mut rounds = 0;
    let mut said = Vec::new();
    for (_, text) in chunks[2..].iter().cycle().take(40) {
        rounds += 1;
        if party_hit == Some(rounds) {
            g.output(&line(
                "Rocco smashes gigantic black ooze for 36 damage!",
                208,
            ))
            .await;
        }
        g.output(text).await;
        let before = g.seen.len();
        g.idle(Duration::from_secs(5)).await;
        said = notices(&g.seen[before..]);
        if said.iter().any(|s| s.starts_with("Stalemate")) {
            break;
        }
    }
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
    (rounds, seen)
}

#[tokio::test(start_paused = true)]
async fn the_recorded_ooze_fight_ends_by_its_tenth_round() {
    // Close to a second: fifty seconds of rounds on the paused clock, each with its prompts.
    let (rounds, seen) = ooze_stalemate(None).await;
    assert_eq!(rounds, STALEMATE_ROUNDS);
    // No rule of its own (C-096): the player's *avoid* stays as it was.
    assert!(rules_of(&seen).contains(&entry(
        "gigantic black ooze",
        MonsterRuleKind::Avoid,
        RuleSource::Player
    )));
    assert!(
        !rules_of(&seen)
            .iter()
            .any(|e| e.source == RuleSource::Session),
        "{:?}",
        rules_of(&seen)
    );
}

fn brain_on(path: &Path) -> Brain {
    let (store, _) = Store::open(path).unwrap();
    let clock = InstantClock::new(UNIX_EPOCH + Duration::from_secs(T0));
    Brain::new(world(), Arc::new(clock), None).with_store(store)
}

#[tokio::test(start_paused = true)]
async fn a_party_members_blow_on_the_target_puts_the_stalemate_off() {
    // Close to a second: seventy seconds of rounds on the paused clock, each with its prompts.
    // C-091: Rocco hurts the ooze before round 5, so the rounds before it do not count: the
    // stalemate comes ten rounds after.
    let (rounds, _) = ooze_stalemate(Some(5)).await;
    assert_eq!(rounds, 4 + STALEMATE_ROUNDS);
}

#[tokio::test(start_paused = true)]
async fn a_players_rule_round_trips_through_the_snapshot_and_survives_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let (mut g, run) = arena_with(brain_on(&path), &[RAT]).await;
    g.set_rule("Giant Rat", Some(MonsterRuleKind::Avoid)).await;
    g.narrated("Monster rule: avoid Giant Rat").await;
    g.set_rule("giant rat", Some(MonsterRuleKind::Ignore)).await;
    g.narrated("Monster rule: ignore giant rat").await;
    g.set_rule("kobold", Some(MonsterRuleKind::Ignore)).await;
    g.set_rule("kobold", None).await;
    g.narrated("Monster rule removed: kobold").await;
    g.set_rule("x", Some(MonsterRuleKind::Avoid)).await;
    g.narrated("set_monster_rule: the pattern needs two characters or more")
        .await;
    let (seen, _) = g.close(run).await;
    let want = vec![
        entry(
            "gigantic black ooze",
            MonsterRuleKind::Ignore,
            RuleSource::Default,
        ),
        entry("Giant Rat", MonsterRuleKind::Ignore, RuleSource::Player),
    ];
    assert_eq!(rules_of(&seen), want);

    // A second brain on the same store: the player's rule is still there.
    let (g, run) = arena_with(brain_on(&path), &[RAT]).await;
    let (seen, _) = g.close(run).await;
    assert_eq!(rules_of(&seen), want);
}

#[tokio::test(start_paused = true)]
async fn a_rule_removed_while_the_monster_is_here_takes_effect_at_once() {
    let (mut g, run) = arena_with(ignoring_rats(), &[RAT]).await;
    start(&mut g, &[RAT], 35).await;
    g.narrated("Waiting here").await;
    // The page's `x` on the player's rule: the rat is a target again, at once.
    g.expect("a rat", line(ENGAGED, 35));
    g.set_rule("giant rat", None).await;
    g.narrated("Monster rule removed: giant rat").await;
    g.narrated("Fight: large giant rat").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat"]);
}

/// The page's `x` on a default: switched off for this character by a player `off`, kept in the
/// store; `x` on the `off` puts the default back.
#[tokio::test(start_paused = true)]
async fn a_default_rule_is_switched_off_and_put_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let (mut g, run) = arena_with(brain_on(&path), &[]).await;
    g.set_rule("gigantic black ooze", None).await;
    g.narrated("Monster rule removed: gigantic black ooze")
        .await;
    let (seen, _) = g.close(run).await;
    let off = entry(
        "gigantic black ooze",
        MonsterRuleKind::Off,
        RuleSource::Player,
    );
    assert!(rules_of(&seen).contains(&off), "{:?}", rules_of(&seen));
    // Kept: a second brain on the store has it off too, until the `off` is removed.
    let (mut g, run) = arena_with(brain_on(&path), &[]).await;
    assert!(rules_of(&g.seen).contains(&off));
    g.set_rule("gigantic black ooze", None).await;
    g.narrated("Monster rule removed: gigantic black ooze")
        .await;
    let (seen, _) = g.close(run).await;
    assert_eq!(
        rules_of(&seen),
        [entry(
            "gigantic black ooze",
            MonsterRuleKind::Ignore,
            RuleSource::Default,
        )]
    );
}

/// C-075, the Thrag run: a boss in the room is the target before anything else, though the
/// room lists it last; its default shows in the rules list once seen.
#[tokio::test(start_paused = true)]
async fn a_boss_is_attacked_first() {
    let brain = Brain::new(world(), clock(), None);
    let (mut g, run) = arena_with(brain, &[RAT, "Thrag"]).await;
    start(&mut g, &[RAT, "Thrag"], 35).await;
    g.expect("a Thrag", line(ENGAGED, 35));
    g.narrated("Fight: Thrag").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a Thrag"]);
    let rules = rules_of(&seen);
    let thrag = rules.iter().find(|e| e.pattern == "thrag");
    assert!(
        thrag.is_some_and(|e| e.rule == MonsterRuleKind::First
            && e.source == RuleSource::Default
            && e.reason.as_deref().is_some_and(|r| r.starts_with("a boss"))),
        "{rules:?}"
    );
}

/// A boss that comes in mid-fight is switched to at once, and only once.
#[tokio::test(start_paused = true)]
async fn a_boss_coming_in_mid_fight_is_switched_to_once() {
    let brain = Brain::new(world(), clock(), None);
    let (mut g, run) = arena_with(brain, &[RAT]).await;
    start(&mut g, &[RAT], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.expect("a Thrag", line(ENGAGED, 34));
    g.output(&line("Thrag moves into the room from the north.", 34))
        .await;
    g.narrated("Fight: Thrag is here: switching from large giant rat")
        .await;
    // A round with Thrag still here: no second switch.
    g.round("You slam Thrag for 5 damage!", 30).await;
    g.idle(Duration::from_secs(2)).await;
    g.freeze();
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "a Thrag"]);
}

/// The player switches a boss's default off: it is attacked as any monster is, in the room's
/// order.
#[tokio::test(start_paused = true)]
async fn a_boss_switched_off_is_not_attacked_first() {
    let brain = Brain::new(world(), clock(), None);
    let (mut g, run) = arena_with(brain, &[RAT, "Thrag"]).await;
    g.set_rule("thrag", None).await;
    g.narrated("Monster rule removed: thrag").await;
    start(&mut g, &[RAT, "Thrag"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat"]);
    assert!(
        rules_of(&seen).contains(&entry("thrag", MonsterRuleKind::Off, RuleSource::Player)),
        "{:?}",
        rules_of(&seen)
    );
}
