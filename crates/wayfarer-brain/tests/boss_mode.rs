//! Boss fight mode (C-100, docs/brain.md "Boss fight mode"): the player walks the party in and
//! each bot fights where it stands. It never flees or walks, casts no blessings, casts its
//! attack spell up to its own limit whatever the mana, and hangs up at its own line. In the
//! Newhaven Arena of `game/arena.rs`, on a paused clock.

mod game;

use std::path::Path;
use std::time::UNIX_EPOCH;

use game::arena::*;
use game::*;
use wayfarer_brain::{Brain, FixedClock, InstantClock, STALEMATE_ROUNDS, Store};
use wayfarer_proto::{BotAction, BotState, SessionAction};

const RAT: &str = "large giant rat";

fn lunge(hp: i32) -> String {
    line("The large giant rat lunges at you!", hp)
}

/// A brain with the settings `text`, attached in the Arena with `monsters`, its re-sync done.
async fn arena_with(monsters: &[&str], text: &str) -> (Game, Run) {
    let clock = Arc::new(FixedClock(UNIX_EPOCH + Duration::from_secs(T0)));
    let brain = Brain::new(world(), clock, None).with_settings(test_settings(text));
    let (mut g, run) = Game::start_brain(SessionState::Realm, arena(monsters, 35), brain).await;
    g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
        .await;
    g.idle(Duration::from_secs(20)).await;
    (g, run)
}

impl Game {
    async fn boss(&mut self, on: bool) {
        self.send(ToClient::SetBossMode {
            session: ID.into(),
            on,
        })
        .await;
    }
}

/// Fighting the rat at full HP with the settings `text`, boss fight mode on.
async fn boss_fight(text: &str) -> (Game, Run) {
    let (mut g, run) = arena_with(&[RAT], text).await;
    start(&mut g, &[RAT], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.boss(true).await;
    g.narrated("Boss fight mode: fighting here, no fleeing or walking")
        .await;
    g.round("The large giant rat lunges at you!", 20).await;
    (g, run)
}

fn hung_up(seen: &[ToGateway]) -> bool {
    seen.iter().any(|m| {
        matches!(
            m,
            ToGateway::SessionControl {
                action: SessionAction::HangUp,
                ..
            }
        )
    })
}

#[tokio::test(start_paused = true)]
async fn under_the_flee_line_it_fights_on_and_hangs_up_at_its_own_line() {
    // The boss line (15%) over the usual one (10%): at 14% it hangs up, where the usual line
    // would let it fight on.
    let (mut g, run) = boss_fight("[health]\nboss_hangup_below_hp = 15\n").await;
    // 8 of 35 is 22%, under the flee line: no flee.
    g.output(&lunge(8)).await;
    g.idle(Duration::from_secs(5)).await;
    assert!(!hung_up(&g.seen));
    // 5 of 35 is 14%.
    g.output(&lunge(5)).await;
    g.narrated("Stopped: hung up at 14% HP (under 15%): something was hitting it")
        .await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat"]);
    assert!(hung_up(&seen));
    assert!(
        !notices(&seen).iter().any(|n| n.starts_with("Fleeing")),
        "{:?}",
        notices(&seen)
    );
}

#[tokio::test(start_paused = true)]
async fn turned_off_again_it_flees_as_before() {
    let (mut g, run) = boss_fight("").await;
    g.boss(false).await;
    g.narrated("Boss fight mode off: playing here").await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 8));
    g.output(&lunge(8)).await;
    g.narrated("Fleeing: HP 22%: up to Newhaven, Narrow Road")
        .await;
    // The command comes after the narration that says it.
    g.idle(Duration::from_secs(1)).await;
    g.freeze();
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "u"]);
    assert!(!bot(&seen).boss_mode);
}

#[tokio::test(start_paused = true)]
async fn on_starts_a_stopped_bot_where_it_stands() {
    let (mut g, run) = arena_with(&[], "").await;
    g.expect("st", status(35));
    g.expect("look", arena(&[], 35));
    g.boss(true).await;
    g.narrated("Starting in Newhaven, Arena: re-syncing").await;
    g.narrated("Waiting here").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look"]);
    let b = bot(&seen);
    assert_eq!(b.state, BotState::Running);
    assert!(b.boss_mode);
}

#[tokio::test(start_paused = true)]
async fn on_ends_a_go_to_after_the_move_in_flight() {
    let (mut g, run) = arena_with(&[], "").await;
    start(&mut g, &[], 35).await;
    g.narrated("Waiting here").await;
    // Up to the Narrow Road, then east to the Narrow Path: the first move only.
    g.expect("u", room(NARROW_ROAD));
    g.walk_to(NARROW_PATH).await;
    g.boss(true).await;
    let said = g
        .narrated(
            "Boss fight mode; stopped Newhaven, Narrow Path: fighting here, no fleeing or walking",
        )
        .await;
    g.idle(Duration::from_secs(30)).await;
    g.freeze();
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "u"], "{said:?}");
    assert!(bot(&seen).journey.is_none());
}

#[tokio::test(start_paused = true)]
async fn a_stalemate_does_not_send_it_away() {
    let (mut g, run) = boss_fight("").await;
    let glance = "Your swing at large giant rat hits, but glances off its armour.";
    g.round("You whap large giant rat for 6 damage!", 35).await;
    for round in 1..=STALEMATE_ROUNDS {
        g.output(&format!("{}{}", line(glance, 35), line(glance, 35)))
            .await;
        if round < STALEMATE_ROUNDS {
            g.idle(Duration::from_secs(5)).await;
        }
    }
    g.narrated(
        "Stalemate: my swings do nothing to large giant rat; ignoring it (boss fight: not leaving)",
    )
    .await;
    g.idle(Duration::from_secs(10)).await;
    g.freeze();
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat"]);
    assert!(!notices(&seen).iter().any(|n| n.starts_with("Fleeing")));
}

#[tokio::test(start_paused = true)]
async fn it_rests_where_it_stands_though_the_room_spawns_and_quiet_rooms_are_asked_for() {
    // The Arena spawns; with `health.rest_quiet_room` on the bot would walk up to the Narrow
    // Road to rest. 15 of 35 is 42%, under the rest line.
    let (mut g, run) = arena_with(&[], "[health]\nrest_quiet_room = true\n").await;
    g.expect("st", status(15));
    g.expect("look", arena(&[], 15));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(15)));
    g.boss(true).await;
    g.narrated("Recover: resting to 85% in Newhaven, Arena")
        .await;
    // The command comes after the narration that says it.
    g.idle(Duration::from_secs(1)).await;
    g.freeze();
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "rest"]);
}

/// The capture's status screen as a character of `class` with `mana` of 20.
fn caster_status(class: &str, hp: i32, mana: i32) -> String {
    let screen = status(hp).replace("Warrior    ", &format!("{class:<10} "));
    let hits = "Thievery:    \x1b[36m    0\r\n";
    screen.replacen(
        hits,
        &format!(
            "{hits}\x1b[32mMana: \x1b[36m * {mana}/20    \x1b[32mSpellcasting: \x1b[36m  40     \
             \x1b[32mTraps:       \x1b[36m    0\r\n"
        ),
        1,
    )
}

/// The `spells` screen with these rows (level, mana, short, name), then the prompt.
fn spells_screen(rows: &[(i32, i32, &str, &str)], hp: i32) -> String {
    let mut s = String::from("You have the following spells:\r\nLevel Mana Short Spell Name\r\n");
    for (level, mana, short, name) in rows {
        s.push_str(&format!("  {level}   {mana}    {short}  {name}\r\n"));
    }
    s + &prompt(hp)
}

#[tokio::test(start_paused = true)]
async fn no_blessing_is_cast() {
    let (mut g, run) = arena_with(&[], "").await;
    g.send(ToClient::BlessingsSave {
        session: ID.into(),
        spells: vec!["bles".into()],
    })
    .await;
    g.expect("st", caster_status("Cleric", 35, 20));
    g.expect("look", arena(&[], 35));
    g.expect(
        "spells",
        spells_screen(
            &[(1, 2, "mihe", "minor healing"), (2, 4, "bles", "bless")],
            35,
        ),
    );
    g.boss(true).await;
    g.narrated("Waiting here").await;
    g.idle(Duration::from_secs(60)).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "spells"]);
}

#[tokio::test(start_paused = true)]
async fn the_attack_spell_is_cast_under_the_mana_floor() {
    // 2 of 20 is 10%, under the 15% floor for attack spells: the weapon, out of boss mode.
    let (mut g, run) = arena_with(&[RAT], "[health]\nrest_mana_below = 0\n").await;
    g.expect("st", caster_status("Mage", 35, 2));
    g.expect("look", arena(&[RAT], 35));
    g.expect(
        "spells",
        spells_screen(&[(1, 1, "mmis", "magic missile")], 35),
    );
    g.expect("mmis rat", line(ENGAGED, 35));
    g.boss(true).await;
    g.narrated("Fight: large giant rat with magic missile")
        .await;
    // The command comes after the narration that says it.
    g.idle(Duration::from_secs(1)).await;
    g.freeze();
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "spells", "mmis rat"]);
}

fn brain_on(path: &Path) -> Brain {
    let (store, _) = Store::open(path).unwrap();
    let clock = InstantClock::new(UNIX_EPOCH + Duration::from_secs(T0));
    Brain::new(world(), Arc::new(clock), None).with_store(store)
}

#[tokio::test(start_paused = true)]
async fn it_stays_on_across_a_restart_of_the_brain() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let (mut g, run) =
        Game::start_brain(SessionState::Realm, arena(&[], 35), brain_on(&path)).await;
    g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
        .await;
    g.idle(Duration::from_secs(20)).await;
    g.expect("st", status(35));
    g.expect("look", arena(&[], 35));
    g.boss(true).await;
    g.narrated("Waiting here").await;
    g.close(run).await;

    let (mut g, run) =
        Game::start_brain(SessionState::Realm, arena(&[], 35), brain_on(&path)).await;
    g.idle(Duration::from_secs(20)).await;
    g.bot(BotAction::Stop).await;
    let (seen, _) = g.close(run).await;
    assert!(bot(&seen).boss_mode, "still on");
}

const CAST: &str = "You cast magic missile at the large giant rat for 7 damage!";

#[tokio::test(start_paused = true)]
async fn its_own_casts_limit_holds_in_place_of_the_usual_one() {
    // One cast at a monster, usually; three in boss fight mode.
    let (mut g, run) = arena_with(
        &[RAT],
        "[spells]\nmax_casts_per_monster = 1\nboss_max_casts_per_monster = 3\n",
    )
    .await;
    g.expect("st", caster_status("Mage", 35, 20));
    g.expect("look", arena(&[RAT], 35));
    g.expect(
        "spells",
        spells_screen(&[(1, 1, "mmis", "magic missile")], 35),
    );
    g.expect("mmis rat", line(ENGAGED, 35));
    g.boss(true).await;
    g.narrated("Fight: large giant rat with magic missile")
        .await;
    g.expect("a rat", format!("{}{}", line(ENGAGED, 35), kill(35)));
    g.expect("look", arena(&[], 35));
    for _ in 0..3 {
        g.round(CAST, 35).await;
    }
    g.idle(Duration::from_secs(1)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(
        played,
        ["st", "look", "spells", "mmis rat", "a rat", "look"]
    );
    let said = notices(&seen);
    assert!(
        said.iter()
            .any(|n| n == "Fight: 3 casts at large giant rat, now the weapon"),
        "{said:?}"
    );
}
