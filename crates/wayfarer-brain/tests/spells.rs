//! Spells and heals (docs/brain.md "Spells"): a caster's re-sync lists the `spells` screen, a
//! fight starts with the best attack spell it can pay for, a heal is cast before a rest, and a
//! refused cast falls back to the weapon. The Arena of `game/arena.rs`, the pack's own spells.
//! On a paused clock.

mod game;

use std::sync::Arc;
use std::time::UNIX_EPOCH;

use game::arena::*;
use game::*;
use wayfarer_brain::{Brain, FixedClock};
use wayfarer_proto::BotAction;
use wayfarer_settings::{Layer, Overrides, Settings};

/// The capture's status screen as a character of `class` with `mana` of 20.
fn caster_status(class: &str, hp: i32, mana: i32) -> String {
    let screen = status(hp).replace("Warrior    ", &format!("{class:<10} "));
    let hits = "Thievery:    \x1b[36m    0\r\n";
    assert!(screen.contains(hits));
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

/// Start the bot as a `class` with `mana` of 20 and the spells `rows`.
async fn start_caster(
    g: &mut Game,
    class: &str,
    rows: &[(i32, i32, &str, &str)],
    monsters: &[&str],
    hp: i32,
    mana: i32,
) {
    g.expect("st", caster_status(class, hp, mana));
    g.expect("look", arena(monsters, hp));
    g.expect("spells", spells_screen(rows, hp));
    g.bot(BotAction::Start).await;
    g.narrated("Starting in Newhaven, Arena: re-syncing").await;
}

/// The Arena with the brain's settings from `text`, its first re-sync done.
async fn in_arena_with(monsters: &[&str], text: &str) -> (Game, Run) {
    let over = Overrides::parse(text).unwrap();
    let settings = Settings::resolve(&[(Layer::Character, &over)]).unwrap();
    let clock = Arc::new(FixedClock(UNIX_EPOCH + Duration::from_secs(T0)));
    let brain = Brain::new(world(), clock, None).with_settings(settings);
    let (mut g, run) = Game::start_brain(SessionState::Realm, arena(monsters, 35), brain).await;
    g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
        .await;
    g.idle(Duration::from_secs(20)).await;
    (g, run)
}

const MAGE_SPELLS: [(i32, i32, &str, &str); 2] =
    [(1, 1, "mmis", "magic missile"), (1, 4, "star", "starlight")];
const CLERIC_SPELLS: [(i32, i32, &str, &str); 1] = [(1, 2, "mihe", "minor healing")];

#[tokio::test(start_paused = true)]
async fn a_mage_opens_with_magic_missile_and_leaves_starlight_alone() {
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start_caster(&mut g, "Mage", &MAGE_SPELLS, &["large giant rat"], 35, 20).await;
    g.expect("mmis rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat with magic missile")
        .await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "spells", "mmis rat"]);
}

#[tokio::test(start_paused = true)]
async fn a_warrior_is_not_asked_for_spells() {
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat"]);
}

#[tokio::test(start_paused = true)]
async fn low_mana_is_fought_with_the_weapon() {
    // 2 of 20 is 10%, under the 15% line for attack spells; resting for mana is off so the
    // rat is fought rather than left for a rest.
    let (mut g, run) = in_arena_with(&["large giant rat"], "[health]\nrest_mana_below = 0\n").await;
    start_caster(&mut g, "Mage", &MAGE_SPELLS, &["large giant rat"], 35, 2).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "spells", "a rat"]);
}

#[tokio::test(start_paused = true)]
async fn a_mage_with_little_mana_rests_for_it_before_looking_for_a_fight() {
    let (mut g, run) = in_arena(&[]).await;
    start_caster(&mut g, "Mage", &MAGE_SPELLS, &[], 35, 2).await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 35));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(35)));
    g.narrated("Recover: walking to Newhaven, Narrow Road to rest (Newhaven, Arena spawns)")
        .await;
    g.narrated("Recover: resting to 85% in Newhaven, Narrow Road")
        .await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "spells", "u", "rest"]);
}

#[tokio::test(start_paused = true)]
async fn a_cast_the_game_refuses_falls_back_to_the_weapon() {
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start_caster(&mut g, "Mage", &MAGE_SPELLS, &["large giant rat"], 35, 20).await;
    g.expect(
        "mmis rat",
        format!(
            "You do not have enough mana to cast that spell.\r\n{}",
            prompt(35)
        ),
    );
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "spells", "mmis rat", "a rat"]);
}

#[tokio::test(start_paused = true)]
async fn a_cleric_heals_then_rests_and_heals_again_while_resting() {
    let (mut g, run) = in_arena(&[]).await;
    // 10 of 35 is 28%: below the 60% line. A heal averages about 5, the mana is 20.
    start_caster(&mut g, "Cleric", &CLERIC_SPELLS, &[], 10, 20).await;
    g.expect(
        "mihe",
        format!("You cast minor healing, for 5\r\n{}", prompt(15)),
    );
    // A round has not passed: it goes on to a quiet room and rests there.
    g.expect("u", at_hp(&room(NARROW_ROAD), 15));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(15)));
    g.narrated("Recover: HP 28%, casting minor healing").await;
    g.narrated("Recover: walking to Newhaven, Narrow Road to rest (Newhaven, Arena spawns)")
        .await;
    g.narrated("Recover: resting to 85% in Newhaven, Narrow Road")
        .await;
    // Resting, HP 18 of 35 (51%) is under the line: a round later it heals again, which ends
    // the rest, and rests again.
    g.expect(
        "mihe",
        format!("You cast minor healing, for 5\r\n{}", prompt(23)),
    );
    g.expect("rest", format!("You are now resting.\r\n{}", resting(23)));
    g.output(&resting(18)).await;
    g.idle(Duration::from_secs(6)).await;
    let (_, played) = g.close(run).await;
    assert_eq!(
        played,
        ["st", "look", "spells", "mihe", "u", "rest", "mihe", "rest"]
    );
}

#[tokio::test(start_paused = true)]
async fn heal_while_resting_at_0_leaves_the_rest_alone() {
    let (mut g, run) = in_arena_with(&[], "[spells]\nheal_resting_below_hp = 0\n").await;
    start_caster(&mut g, "Cleric", &CLERIC_SPELLS, &[], 20, 20).await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 20));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(20)));
    g.narrated("Recover: resting to 85% in Newhaven, Narrow Road")
        .await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "spells", "u", "rest"]);
}

#[tokio::test(start_paused = true)]
async fn healing_off_only_rests() {
    let (mut g, run) = in_arena_with(&[], "[spells]\nheal = \"off\"\n").await;
    start_caster(&mut g, "Cleric", &CLERIC_SPELLS, &[], 20, 20).await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 20));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(20)));
    g.narrated("Recover: walking to Newhaven, Narrow Road to rest (Newhaven, Arena spawns)")
        .await;
    g.narrated("Recover: resting to 85% in Newhaven, Narrow Road")
        .await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "spells", "u", "rest"]);
}

#[tokio::test(start_paused = true)]
async fn a_cleric_hurt_in_a_fight_heals_then_fights_on() {
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start_caster(
        &mut g,
        "Cleric",
        &CLERIC_SPELLS,
        &["large giant rat"],
        35,
        20,
    )
    .await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.round("You whap large giant rat for 6 damage!", 35).await;
    // 15 of 35 is 42%: under the 50% line for a heal in a fight, over the 25% flee line.
    // The cast ends auto-combat: the game says so, and the bot attacks again at once, with no
    // `look` in between, before the next round's swings.
    g.expect(
        "mihe",
        format!(
            "You cast minor healing, for 5\r\n*Combat Off*\r\n{}",
            prompt(20)
        ),
    );
    g.expect("a rat", line(ENGAGED, 20));
    g.output(&line("The large giant rat lunges at you!", 15))
        .await;
    g.narrated("Fight: HP 42%, casting minor healing").await;
    g.narrated("Fight: large giant rat").await;
    let (_, played) = g.close(run).await;
    assert_eq!(
        played[..6],
        ["st", "look", "spells", "a rat", "mihe", "a rat"]
    );
}

const CAST: &str = "You cast magic missile at the large giant rat for 7 damage!";

#[tokio::test(start_paused = true)]
async fn after_the_most_casts_the_attack_command_takes_over_in_the_fight() {
    let (mut g, run) = in_arena_with(
        &["large giant rat"],
        "[spells]\nmax_casts_per_monster = 2\n",
    )
    .await;
    start_caster(&mut g, "Mage", &MAGE_SPELLS, &["large giant rat"], 35, 20).await;
    g.expect("mmis rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat with magic missile")
        .await;
    // Two casts, then `a rat` at once, with combat engaged (the one attack the guard lets
    // through); the rat dies in the answer, since the fake sends no blows and the test's clock
    // would run on to a lapse.
    g.expect("a rat", format!("{}{}", line(ENGAGED, 35), kill(35)));
    g.expect("look", arena(&[], 35));
    g.round(CAST, 35).await;
    g.round(CAST, 35).await;
    // (The notices come while the rounds are played, so they are read from what was seen.)
    g.idle(Duration::from_secs(1)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(
        played,
        ["st", "look", "spells", "mmis rat", "a rat", "look"]
    );
    let said = notices(&seen);
    assert!(
        said.iter()
            .any(|n| n == "Fight: 2 casts at large giant rat, now the weapon"),
        "{said:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn with_no_limit_the_spell_goes_on() {
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start_caster(&mut g, "Mage", &MAGE_SPELLS, &["large giant rat"], 35, 20).await;
    g.expect("mmis rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat with magic missile")
        .await;
    for _ in 0..6 {
        g.round(CAST, 35).await;
    }
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "spells", "mmis rat"]);
}
