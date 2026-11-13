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
use wayfarer_proto::{BotAction, ToClient};

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
    let settings = test_settings(text);
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

/// A mage with magic missile and the stronger lightning bolt.
const BOLT_SPELLS: [(i32, i32, &str, &str); 2] = [
    (1, 1, "mmis", "magic missile"),
    (8, 5, "lbol", "lightning bolt"),
];

#[tokio::test(start_paused = true)]
async fn auto_casts_the_strongest_attack_it_can_pay_for() {
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start_caster(&mut g, "Mage", &BOLT_SPELLS, &["large giant rat"], 35, 20).await;
    g.expect("lbol rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat with lightning bolt")
        .await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "spells", "lbol rat"]);
    // The spells to choose from, and no choice: auto.
    let bot = bot(&seen);
    let known: Vec<_> = bot
        .attack_spells
        .iter()
        .map(|s| (s.short.as_str(), s.mana))
        .collect();
    assert_eq!(known, [("mmis", 1), ("lbol", 5)]);
    assert_eq!(bot.attack_spell, None);
}

#[tokio::test(start_paused = true)]
async fn the_attack_spell_the_player_chose_is_cast_in_place_of_the_strongest() {
    // C-099: the player picks magic missile; the fight casts it, not the bolt auto would.
    let (mut g, run) = in_arena(&[]).await;
    start_caster(&mut g, "Mage", &BOLT_SPELLS, &[], 35, 20).await;
    // The spellbook is read in the re-sync.
    g.narrated("re-synced").await;
    let choose = |spell: Option<&str>| ToClient::SetAttackSpell {
        session: ID.into(),
        spell: spell.map(str::to_owned),
    };
    // One it does not know is refused, and changes nothing.
    g.send(choose(Some("fbal"))).await;
    g.narrated("set_attack_spell: fbal is not an attack spell this character knows")
        .await;
    g.send(choose(Some("mmis"))).await;
    g.narrated("Attack spell: magic missile (mmis)").await;
    g.expect("mmis rat", line(ENGAGED, 35));
    g.output(&arena(&["large giant rat"], 35)).await;
    g.narrated("Fight: large giant rat with magic missile")
        .await;
    g.send(choose(None)).await;
    g.narrated("Attack spell: auto, the strongest it can pay for")
        .await;
    g.freeze();
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "spells", "mmis rat"]);
    assert_eq!(bot(&seen).attack_spell, None);
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
    // Only with a rest-for-mana line: the default (0) never rests for mana (C-051).
    let (mut g, run) = in_arena_with(&[], "[health]\nrest_mana_below = 30\n").await;
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
async fn a_mage_with_little_mana_does_not_rest_for_it_by_default() {
    let (mut g, run) = in_arena(&[]).await;
    start_caster(&mut g, "Mage", &MAGE_SPELLS, &[], 35, 2).await;
    g.idle(Duration::from_secs(30)).await;
    let (_, played) = g.close(run).await;
    assert!(
        !played.iter().any(|c| c == "rest" || c == "u"),
        "{played:?}"
    );
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

const DRUID_SPELLS: [(i32, i32, &str, &str); 1] = [(8, 5, "strk", "stonestrike")];

#[tokio::test(start_paused = true)]
async fn a_spell_that_lands_on_its_target_counts_toward_the_most_casts() {
    // Stonestrike's line is "cast ... on ...", the text of a monster's hit message too: it is
    // still a cast, so one cast is the limit and the weapon follows (faye, 2026-10-26).
    let (mut g, run) = in_arena_with(
        &["large giant rat"],
        "[spells]\nmax_casts_per_monster = 1\n",
    )
    .await;
    start_caster(&mut g, "Druid", &DRUID_SPELLS, &["large giant rat"], 35, 20).await;
    g.expect("strk rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat with stonestrike").await;
    g.expect("a rat", format!("{}{}", line(ENGAGED, 35), kill(35)));
    g.expect("look", arena(&[], 35));
    g.round("You cast stonestrike on large giant rat for 22 damage!", 35)
        .await;
    g.idle(Duration::from_secs(1)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(
        played,
        ["st", "look", "spells", "strk rat", "a rat", "look"]
    );
    let said = notices(&seen);
    assert!(
        said.iter()
            .any(|n| n == "Fight: 1 casts at large giant rat, now the weapon"),
        "{said:?}"
    );
}

/// A round's line, then the prompt with mana (`[HP=35/MA=7]:`), as a caster's prompt reads.
fn with_mana(text: &str, hp: i32, mana: i32) -> String {
    format!("\x1b[79D\x1b[K\x1b[1;31m{text}\r\n\x1b[79D\x1b[K\x1b[0;37m[HP={hp}/MA={mana}]:")
}

#[tokio::test(start_paused = true)]
async fn the_weapon_takes_over_when_the_mana_comes_down_to_the_line_mid_fight() {
    // The game casts the spell every round on its own: the mana line is checked each round,
    // not only when the fight opens. 40% of 20 is 8.
    let (mut g, run) =
        in_arena_with(&["large giant rat"], "[spells]\nattack_mana_above = 40\n").await;
    start_caster(&mut g, "Mage", &MAGE_SPELLS, &["large giant rat"], 35, 20).await;
    g.expect("mmis rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat with magic missile")
        .await;
    for mana in [16, 12, 9] {
        g.output(&with_mana(CAST, 35, mana)).await;
        g.idle(Duration::from_secs(4)).await;
    }
    g.expect("a rat", format!("{}{}", line(ENGAGED, 35), kill(35)));
    g.expect("look", arena(&[], 35));
    g.output(&with_mana(CAST, 35, 8)).await;
    g.idle(Duration::from_secs(1)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(
        played,
        ["st", "look", "spells", "mmis rat", "a rat", "look"]
    );
    let said = notices(&seen);
    assert!(
        said.iter().any(|n| n == "Fight: mana 40%, now the weapon"),
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

const BLESSER_SPELLS: [(i32, i32, &str, &str); 2] =
    [(1, 2, "mihe", "minor healing"), (2, 4, "bles", "bless")];

/// Blessings (C-047): a Cleric with bless ticked casts it between fights, not again while it
/// is on, and again when the game says it wore off.
#[tokio::test(start_paused = true)]
async fn a_ticked_blessing_is_kept_on() {
    let (mut g, run) = in_arena(&[]).await;
    g.send(ToClient::BlessingsSave {
        session: ID.into(),
        spells: vec!["bles".into()],
    })
    .await;
    start_caster(&mut g, "Cleric", &BLESSER_SPELLS, &[], 35, 20).await;
    let cast = format!(
        "You cast bless on Aldric!\r\nYou feel lucky!\r\n{}",
        prompt(35)
    );
    g.expect("bles", cast.clone());
    g.narrated("Blessing: casting bless").await;
    g.narrated("Waiting here").await;
    // On: nothing more for a minute.
    g.idle(Duration::from_secs(60)).await;
    g.expect("bles", cast);
    g.output(&line("The effects of bless wear off!", 35)).await;
    g.narrated("Blessing: casting bless").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "spells", "bles", "bles"]);
    let on = seen.iter().any(|m| {
        matches!(m, ToGateway::Blessings { characters }
            if characters.iter().any(|c| c.spells.iter().any(|s| s.short == "bles" && s.chosen && s.on)))
    });
    assert!(on, "a blessings message with bless on");
}

/// A blessing that is off goes ahead of a rest once the character is over its rest line: it
/// rests first from 40%, casts at 71% and rests on.
#[tokio::test(start_paused = true)]
async fn a_blessing_goes_ahead_of_a_rest_over_the_rest_line() {
    let (mut g, run) = in_arena(&[]).await;
    g.send(ToClient::BlessingsSave {
        session: ID.into(),
        spells: vec!["bles".into()],
    })
    .await;
    start_caster(&mut g, "Cleric", &[(2, 4, "bles", "bless")], &[], 14, 20).await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 14));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(14)));
    g.narrated("Recover: resting to 85% in Newhaven, Narrow Road")
        .await;
    // The rest answered, HP comes back over the rest line.
    g.idle(Duration::from_secs(5)).await;
    g.expect(
        "bles",
        format!(
            "You cast bless on Aldric!\r\nYou feel lucky!\r\n{}",
            prompt(25)
        ),
    );
    g.expect("rest", format!("You are now resting.\r\n{}", resting(25)));
    g.output(&resting(25)).await;
    g.narrated("Blessing: casting bless").await;
    g.idle(Duration::from_secs(20)).await;
    let (_, played) = g.close(run).await;
    assert_eq!(
        played,
        ["st", "look", "spells", "u", "rest", "bles", "rest"]
    );
}

/// A blessing the game will not cast is tried again after a round, and left alone after three
/// failures in a row.
#[tokio::test(start_paused = true)]
async fn a_blessing_that_keeps_failing_is_held() {
    let (mut g, run) = in_arena(&[]).await;
    g.send(ToClient::BlessingsSave {
        session: ID.into(),
        spells: vec!["bles".into()],
    })
    .await;
    start_caster(&mut g, "Cleric", &BLESSER_SPELLS, &[], 35, 20).await;
    let fail = format!("You attempt to cast bless, but fail.\r\n{}", prompt(35));
    for _ in 0..3 {
        g.expect("bles", fail.clone());
    }
    g.narrated("Blessing: casting bless").await;
    g.idle(Duration::from_secs(120)).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "spells", "bles", "bles", "bles"]);
}

/// A cast's failure that comes in a later chunk than another player's line and its prompt
/// (Manuel, 2026-10-25: `valr`, then `Lauren is looking around the room.` with its prompt, then
/// `You attempt to cast song of valour, but fail.`) is still a failure: tried again after a
/// round, not believed on for the spell's length.
#[tokio::test(start_paused = true)]
async fn a_failure_after_another_players_prompt_is_tried_again() {
    let (mut g, run) = in_arena(&[]).await;
    g.send(ToClient::BlessingsSave {
        session: ID.into(),
        spells: vec!["bles".into()],
    })
    .await;
    start_caster(&mut g, "Cleric", &BLESSER_SPELLS, &[], 35, 20).await;
    g.expect("bles", line("Lauren is looking around the room.", 35));
    g.narrated("Blessing: casting bless").await;
    g.idle(Duration::from_millis(100)).await;
    g.expect("bles", blessed(35));
    g.output(&line("You attempt to cast bless, but fail.", 35))
        .await;
    g.idle(Duration::from_secs(20)).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "spells", "bles", "bles"]);
}

/// The status screen with `felt` (the lines of the spells on) after its last row.
fn status_with(felt: &[&str], hp: i32) -> String {
    let screen = caster_status("Cleric", hp, 20);
    let body = screen.strip_suffix(&prompt(hp)).unwrap();
    let lines: String = felt.iter().map(|l| format!("{l}\r\n")).collect();
    format!("{body}{lines}{}", prompt(hp))
}

/// A Cleric in the Arena with bless ticked and the status screen looked at every
/// `seconds`, blessed.
async fn blessed_and_checked(seconds: u32) -> (Game, Run) {
    let text = format!("[spells]\nbless_check_seconds = {seconds}\n");
    let (mut g, run) = in_arena_with(&[], &text).await;
    g.send(ToClient::BlessingsSave {
        session: ID.into(),
        spells: vec!["bles".into()],
    })
    .await;
    start_caster(&mut g, "Cleric", &BLESSER_SPELLS, &[], 35, 20).await;
    g.expect("bles", blessed(35));
    g.narrated("Blessing: casting bless").await;
    (g, run)
}

/// A blessing believed on that the status screen does not show is off: cast again, saying
/// why (Manuel, 2026-10-25: a failure the bot missed left valour off for six minutes).
#[tokio::test(start_paused = true)]
async fn a_blessing_the_status_screen_does_not_show_is_cast_again() {
    let (mut g, run) = blessed_and_checked(60).await;
    g.expect("st", status_with(&[], 35));
    g.expect("bles", blessed(35));
    g.narrated("Blessing: bless is not on the status screen; casting it")
        .await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "spells", "bles", "st", "bles"]);
}

/// One the screen shows stays on, and the screen is looked at again a minute later; so does
/// one a sysop's debug list stands in for; with the check at 0 it is never looked at.
#[tokio::test(start_paused = true)]
async fn a_blessing_the_status_screen_shows_is_left_on() {
    for shown in [
        &["You feel lucky!"][..],
        &[
            "You have the following spell cast upon you:",
            "  bless (3,10).",
        ],
    ] {
        let (mut g, run) = blessed_and_checked(60).await;
        g.expect("st", status_with(shown, 35));
        g.expect("st", status_with(shown, 35));
        g.idle(Duration::from_secs(130)).await;
        let (_, played) = g.close(run).await;
        assert_eq!(
            played,
            ["st", "look", "spells", "bles", "st", "st"],
            "{shown:?}"
        );
    }

    let (mut g, run) = blessed_and_checked(0).await;
    g.idle(Duration::from_secs(130)).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "spells", "bles"]);
}

/// A Cleric with bless ticked, blessed and fighting the rat (C-056): the cast before the fight,
/// then the attack.
async fn blessed_and_fighting(text: &str) -> (Game, Run) {
    let (mut g, run) = in_arena_with(&["large giant rat"], text).await;
    g.send(ToClient::BlessingsSave {
        session: ID.into(),
        spells: vec!["bles".into()],
    })
    .await;
    start_caster(
        &mut g,
        "Cleric",
        &BLESSER_SPELLS,
        &["large giant rat"],
        35,
        20,
    )
    .await;
    g.expect("bles", blessed(35));
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Blessing: casting bless").await;
    g.narrated("Fight: large giant rat").await;
    g.round("You whap large giant rat for 6 damage!", 35).await;
    (g, run)
}

/// The cast of bless, in a fight: it ends auto-combat (`kb/spells.md`).
fn blessed_in_the_fight(hp: i32) -> String {
    format!(
        "You cast bless on Aldric!\r\nYou feel lucky!\r\n*Combat Off*\r\n{}",
        prompt(hp)
    )
}

fn blessed(hp: i32) -> String {
    format!(
        "You cast bless on Aldric!\r\nYou feel lucky!\r\n{}",
        prompt(hp)
    )
}

/// Bless wears off in the fight: cast there and then, and the same rat attacked again at once,
/// with no `look`.
#[tokio::test(start_paused = true)]
async fn a_blessing_that_wears_off_in_a_fight_is_cast_and_the_fight_goes_on() {
    let (mut g, run) = blessed_and_fighting("").await;
    g.expect("bles", blessed_in_the_fight(30));
    g.expect("a rat", line(ENGAGED, 30));
    g.output(&format!(
        "The large giant rat lunges at you!\r\n{}",
        line("The effects of bless wear off!", 30)
    ))
    .await;
    g.narrated("Blessing: casting bless").await;
    g.narrated("Fight: large giant rat").await;
    let (_, played) = g.close(run).await;
    assert_eq!(
        played,
        ["st", "look", "spells", "bles", "a rat", "bles", "a rat"]
    );
}

/// Under the heal line when it wears off: the heal first, the fight on, and the blessing in a
/// later round.
#[tokio::test(start_paused = true)]
async fn in_a_fight_a_heal_goes_before_the_blessing() {
    let (mut g, run) = blessed_and_fighting("").await;
    g.expect(
        "mihe",
        format!(
            "You cast minor healing, for 5\r\n*Combat Off*\r\n{}",
            prompt(20)
        ),
    );
    g.expect("a rat", line(ENGAGED, 20));
    g.expect("bles", blessed_in_the_fight(20));
    g.expect("a rat", line(ENGAGED, 20));
    // 15 of 35 is 42%: under the 50% heal line.
    g.output(&format!(
        "The large giant rat lunges at you!\r\n{}",
        line("The effects of bless wear off!", 15)
    ))
    .await;
    g.narrated("Fight: HP 42%, casting minor healing").await;
    g.narrated("Blessing: casting bless").await;
    g.narrated("Fight: large giant rat").await;
    let (_, played) = g.close(run).await;
    assert_eq!(
        played[3..],
        ["bles", "a rat", "mihe", "a rat", "bles", "a rat"]
    );
}

/// Off, or under the blessing's mana line: nothing is cast in the fight.
#[tokio::test(start_paused = true)]
async fn no_blessing_in_a_fight_when_turned_off_or_short_of_mana() {
    for (text, before) in [
        // Off: cast before the fight, not in it.
        ("[spells]\nbless_in_combat = false\n", true),
        // 20 mana, bless costs 4: 16 left is 80%, under a 90% line; never cast.
        ("[spells]\nbless_mana_above = 90\n", false),
    ] {
        let (mut g, run) = in_arena_with(&["large giant rat"], text).await;
        g.send(ToClient::BlessingsSave {
            session: ID.into(),
            spells: vec!["bles".into()],
        })
        .await;
        start_caster(
            &mut g,
            "Cleric",
            &BLESSER_SPELLS,
            &["large giant rat"],
            35,
            20,
        )
        .await;
        if before {
            g.expect("bles", blessed(35));
        }
        g.expect("a rat", line(ENGAGED, 35));
        g.narrated("Fight: large giant rat").await;
        g.output(&format!(
            "The large giant rat lunges at you!\r\n{}",
            line("The effects of bless wear off!", 30)
        ))
        .await;
        g.round("You whap large giant rat for 6 damage!", 30).await;
        g.round("The large giant rat lunges at you!", 30).await;
        let (_, played) = g.close(run).await;
        let bles = played.iter().filter(|c| *c == "bles").count();
        assert_eq!(bles, usize::from(before), "{text}: {played:?}");
    }
}

/// The Blessings panel's view of the character: its lasting spells, by short name.
async fn blessing_spells(g: &mut Game) -> Vec<String> {
    g.send(ToClient::BlessingsRequest).await;
    g.until(|m| match m {
        ToGateway::Blessings { characters } => Some(
            characters
                .iter()
                .flat_map(|c| c.spells.iter().map(|s| s.short.clone()))
                .collect(),
        ),
        _ => None,
    })
    .await
}

#[tokio::test(start_paused = true)]
async fn a_1_20_spell_in_the_list_makes_the_server_1_20_and_the_book_is_1_20s() {
    // C-066: arcane attunement is only on a Quicksilver 1.20 server; the `spells` screen that
    // lists it decides the rule set, and the spellbook is learned again with 1.20's spells.
    // (Not kept up here: that is C-067's, below.)
    let (mut g, run) = in_arena_with(&[], "[spells]\nkeep_regen_buff = false\n").await;
    start_caster(
        &mut g,
        "Mage",
        &[
            (1, 1, "mmis", "magic missile"),
            (20, 8, "aatt", "arcane attunement"),
        ],
        &[],
        35,
        20,
    )
    .await;
    // The re-sync's `exp` and `i` go unanswered here, five seconds each, before `spells`.
    g.idle(Duration::from_secs(30)).await;
    assert_eq!(blessing_spells(&mut g).await, ["aatt"]);
    let (seen, _) = g.close(run).await;
    let rules = as_ui_sees(&seen).rule_set.expect("the rule set");
    assert_eq!(rules.rules, "1.20");
    assert_eq!(
        rules.reason.as_deref(),
        Some("a 1.20 spell in the spell list: arcane attunement")
    );
}

const REGEN_MAGE: [(i32, i32, &str, &str); 3] = [
    (1, 1, "mmis", "magic missile"),
    (5, 4, "latt", "lesser attunement"),
    (20, 8, "aatt", "arcane attunement"),
];

/// On 1.20 (C-067) a caster keeps its best regen tier up by itself: arcane attunement, not
/// the lesser tier. The first cast only removes the tier that was up (the family's end line
/// comes): it is cast again. When it wears off, again.
#[tokio::test(start_paused = true)]
async fn on_1_20_the_best_regen_tier_is_kept_up_in_two_casts() {
    let (mut g, run) = in_arena(&[]).await;
    start_caster(&mut g, "Mage", &REGEN_MAGE, &[], 35, 20).await;
    g.expect(
        "aatt",
        format!("Your attunement to the mana fades.\r\n{}", prompt(35)),
    );
    g.expect(
        "aatt",
        format!(
            "You attune your mind to the flow of mana.\r\n{}",
            prompt(35)
        ),
    );
    g.idle(Duration::from_secs(60)).await;
    // On: nothing more until it ends.
    g.expect(
        "aatt",
        format!(
            "You attune your mind to the flow of mana.\r\n{}",
            prompt(35)
        ),
    );
    g.output(&line("Your attunement to the mana fades.", 35))
        .await;
    g.idle(Duration::from_secs(10)).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "spells", "aatt", "aatt", "aatt"]);
}

#[tokio::test(start_paused = true)]
async fn with_the_setting_off_no_regen_tier_is_kept() {
    let (mut g, run) = in_arena_with(&[], "[spells]\nkeep_regen_buff = false\n").await;
    start_caster(&mut g, "Mage", &REGEN_MAGE, &[], 35, 20).await;
    g.idle(Duration::from_secs(60)).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "spells"]);
}
