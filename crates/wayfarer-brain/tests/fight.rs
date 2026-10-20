//! Fight here (docs/brain.md "Fight here"): the bot started in the Newhaven Arena against the
//! fake gateway of `game/mod.rs`, whose game answers each command from the test's script, with
//! texts cut from the `fight-arena` and `room-walk` captures. On a paused clock: a test's
//! minutes of game time take no time.

mod game;

use std::collections::BTreeSet;
use std::sync::OnceLock;

use game::*;
use wayfarer_brain::ATTACK_TRIES;
use wayfarer_brain::executor::{ATTACK_REFUSALS, Action, Executor, Expect, Outcome, Refused};
use wayfarer_brain::policy::{self, RestAt};
use wayfarer_parse::Parser;
use wayfarer_parse::typed::GameEvent;
use wayfarer_proto::{BotAction, BotMode, BotState};

fn fight_capture() -> &'static str {
    static RAW: OnceLock<String> = OnceLock::new();
    RAW.get_or_init(|| {
        let raw =
            std::fs::read(common::repo("tests/fixtures/captures/fight-arena/raw.bin")).unwrap();
        wayfarer_world::cp437::decode(&raw)
    })
}

fn prompt(hp: i32) -> String {
    format!("\x1b[79D\x1b[K\x1b[0;37m[HP={hp}\x1b[0;37m]:")
}

/// The prompt while resting, as the `fight-arena` capture shows it.
fn resting(hp: i32) -> String {
    format!("\x1b[79D\x1b[K\x1b[0;37m[HP={hp}\x1b[0;37m (Resting) ]:")
}

/// A line of the game's, then the prompt.
fn line(text: &str, hp: i32) -> String {
    format!("\x1b[79D\x1b[K\x1b[1;31m{text}\r\n{}", prompt(hp))
}

/// `text` (a room from `game::room`) with the prompt at `hp`.
fn at_hp(text: &str, hp: i32) -> String {
    let cut = text.rfind(PROMPT).unwrap();
    format!("{}{}", &text[..cut], prompt(hp))
}

/// The status screen of the `fight-arena` capture (Aldric, `Hits: 35/35`), then the prompt.
fn status(hp: i32) -> String {
    let raw = fight_capture();
    let start = raw
        .find("\x1b[0;37;40m\x1b[79D\x1b[K\x1b[0m\x1b[32mName:")
        .unwrap();
    let rest = &raw[start..];
    let end = rest
        .find("\x1b[0;37;40m\x1b[79D\x1b[K\x1b[0;37m[HP=")
        .unwrap();
    format!("{}{}", &rest[..end], prompt(hp))
}

/// The Arena with `monsters` in it (the capture's `Also here:` line replaced).
fn arena(monsters: &[&str], hp: i32) -> String {
    let r = room(ARENA);
    let start = r.find("\x1b[0;35mAlso here:").unwrap();
    let end = start + r[start..].find(".\r\n").unwrap() + 3;
    let also = if monsters.is_empty() {
        String::new()
    } else {
        format!(
            "\x1b[0;35mAlso here: \x1b[1;35m{}\x1b[0m\x1b[0;35m.\r\n",
            monsters.join(", ")
        )
    };
    at_hp(&format!("{}{also}{}", &r[..start], &r[end..]), hp)
}

const ENGAGED: &str = "*Combat Engaged*";

/// The kill at the end of a fight with a giant rat, as the capture has it.
fn kill(hp: i32) -> String {
    [
        "The giant rat falls to the ground with a tortured squeak.",
        "You gain 9 experience.",
        "*Combat Off*",
    ]
    .map(|t| line(t, hp))
    .concat()
}

impl Game {
    async fn bot(&self, action: BotAction) {
        self.send(ToClient::BotControl {
            session: ID.into(),
            action,
        })
        .await;
    }

    /// A round of the fight: the monster's blow, and the clock moves on a few seconds.
    async fn round(&mut self, text: &str, hp: i32) {
        self.output(&line(text, hp)).await;
        self.idle(Duration::from_secs(4)).await;
    }
}

/// A brain attached in the Arena, its first re-sync done (with bare prompts).
async fn in_arena(monsters: &[&str]) -> (Game, Run) {
    let (mut g, run) = Game::start_with(SessionState::Realm, arena(monsters, 35)).await;
    g.idle(Duration::from_secs(20)).await;
    (g, run)
}

/// Start the bot: its re-sync's `st` and `look` are answered with `hp` and `monsters`.
async fn start(g: &mut Game, monsters: &[&str], hp: i32) {
    g.expect("st", status(hp));
    g.expect("look", arena(monsters, hp));
    g.bot(BotAction::Start).await;
    g.narrated("Starting in Newhaven, Arena: re-syncing").await;
}

/// The bot's state as the UIs see it.
fn bot(seen: &[ToGateway]) -> wayfarer_proto::Bot {
    as_ui_sees(seen).bot.expect("the bot on the character")
}

fn notices(seen: &[ToGateway]) -> Vec<String> {
    seen.iter()
        .filter_map(|m| match m {
            ToGateway::Notice { text, .. } | ToGateway::Warning { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test(start_paused = true)]
async fn start_attacks_once_waits_while_engaged_and_looks_after_the_kill() {
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    // The fight: nothing is sent while engaged (the script has nothing; anything would fail).
    for hp in [33, 33, 30, 30] {
        g.round("The large giant rat lunges at you!", hp).await;
        g.round("You whap large giant rat for 6 damage!", hp).await;
    }
    g.expect("look", arena(&[], 30));
    g.output(&kill(30)).await;
    g.narrated("Waiting here").await;
    g.idle(Duration::from_secs(60)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "look"]);
    let b = bot(&seen);
    assert_eq!((b.state, b.mode), (BotState::Running, BotMode::Idle));
    assert_eq!(b.narration, "Waiting here");
}

/// Two monsters, then a third that arrives: each attacked once, the next found by looking.
async fn two_and_an_arrival() -> (Vec<String>, Vec<String>) {
    let (mut g, run) = in_arena(&[]).await;
    let both = ["large giant rat", "fat giant rat"];
    start(&mut g, &both, 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.round("The large giant rat lunges at you!", 32).await;
    g.expect("look", arena(&["fat giant rat"], 32));
    g.expect("a rat", line(ENGAGED, 32));
    g.output(&kill(32)).await;
    g.narrated("Fight: fat giant rat").await;
    g.round("The fat giant rat lunges at you!", 30).await;
    g.expect("look", arena(&[], 30));
    g.output(&kill(30)).await;
    g.narrated("Waiting here").await;
    g.idle(Duration::from_secs(30)).await;
    // An arrival: attacked at once.
    g.expect("a rat", line(ENGAGED, 30));
    g.output(&line(
        "A thin giant rat creeps into the room from nowhere.",
        30,
    ))
    .await;
    g.narrated("Fight: thin giant rat").await;
    g.round("The thin giant rat lunges at you!", 30).await;
    let (seen, played) = g.close(run).await;
    (played, notices(&seen))
}

#[tokio::test(start_paused = true)]
async fn the_next_monster_is_found_by_looking_and_an_arrival_is_attacked() {
    let (played, _) = two_and_an_arrival().await;
    assert_eq!(
        played,
        ["st", "look", "a rat", "look", "a rat", "look", "a rat"]
    );
}

#[tokio::test(start_paused = true)]
async fn the_same_game_gives_the_same_commands() {
    let first = two_and_an_arrival().await;
    let second = two_and_an_arrival().await;
    assert_eq!(first, second);
}

#[tokio::test(start_paused = true)]
async fn low_hp_rests_in_a_quiet_room_then_walks_back() {
    let (mut g, run) = in_arena(&[]).await;
    // 20 of 35 is 57%: below 80. The Arena spawns; the Narrow Road, up the stairs, doesn't.
    start(&mut g, &[], 20).await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 20));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(20)));
    g.narrated("Recover: walking to Newhaven, Narrow Road to rest (Newhaven, Arena spawns)")
        .await;
    g.narrated("Recover: resting to 95% in Newhaven, Narrow Road")
        .await;
    // The resting prompt, redrawn as HP comes back; 34 of 35 is 97%.
    for hp in [24, 28, 31] {
        g.output(&resting(hp)).await;
        g.idle(Duration::from_secs(20)).await;
    }
    g.expect("d", arena(&[], 34));
    g.output(&resting(34)).await;
    g.narrated("Recover: rested; walking back to Newhaven, Arena")
        .await;
    g.narrated("Waiting here").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "u", "rest", "d"]);
    let said = notices(&seen);
    // The walks go through the walker, each step confirmed.
    assert!(
        said.contains(&"Arrived: Newhaven, Narrow Road".to_owned()),
        "{said:?}"
    );
    assert!(
        said.contains(&"Arrived: Newhaven, Arena".to_owned()),
        "{said:?}"
    );
    assert_eq!(as_ui_sees(&seen).position.unwrap().room, Some(ARENA.1));
}

#[tokio::test(start_paused = true)]
async fn started_where_the_position_is_unknown_home_is_where_the_resync_finds_it() {
    // Seen on the rig: a brain just attached, its history showing no room. Start re-syncs
    // and its `look` shows the Arena; home is the Arena, so after the rest up the stairs the
    // bot walks back down and fights there again.
    let (mut g, run) = Game::start_lost(SessionState::Realm, prompt(35)).await;
    g.idle(Duration::from_secs(20)).await;
    g.expect("st", status(20));
    g.expect("look", arena(&[], 20));
    g.bot(BotAction::Start).await;
    g.narrated("Starting in an unknown room: re-syncing").await;
    g.narrated("Playing from Newhaven, Arena").await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 20));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(20)));
    g.narrated("Recover: resting to 95% in Newhaven, Narrow Road")
        .await;
    for hp in [24, 28, 31] {
        g.output(&resting(hp)).await;
        g.idle(Duration::from_secs(20)).await;
    }
    g.expect("d", arena(&["large giant rat"], 34));
    g.expect("a rat", line(ENGAGED, 34));
    g.output(&resting(34)).await;
    g.narrated("Recover: rested; walking back to Newhaven, Arena")
        .await;
    g.narrated("Fight: large giant rat").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "u", "rest", "d", "a rat"]);
    let said = notices(&seen);
    assert!(!said.contains(&"Waiting here".to_owned()), "{said:?}");
    for arrived in ["Arrived: Newhaven, Narrow Road", "Arrived: Newhaven, Arena"] {
        assert!(said.contains(&arrived.to_owned()), "{said:?}");
    }
}

#[tokio::test(start_paused = true)]
async fn rest_is_sent_again_when_the_prompt_lacks_resting() {
    let (mut g, run) = in_arena(&[]).await;
    start(&mut g, &[], 20).await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 20));
    g.expect("rest", prompt(20));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(20)));
    g.narrated("Recover: resting to 95% in Newhaven, Narrow Road")
        .await;
    g.idle(Duration::from_secs(10)).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "u", "rest", "rest"]);
}

#[tokio::test(start_paused = true)]
async fn a_monster_that_attacks_while_resting_wakes_fight() {
    let (mut g, run) = in_arena(&[]).await;
    start(&mut g, &[], 20).await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 20));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(20)));
    g.narrated("Recover: resting to 95% in Newhaven, Narrow Road")
        .await;
    g.output(&resting(22)).await;
    g.idle(Duration::from_secs(5)).await;
    // It arrives: Recover keeps resting; it attacks: Fight takes the character.
    g.output(&format!(
        "\x1b[79D\x1b[KA thin giant rat creeps into the room from nowhere.\r\n{}",
        resting(22)
    ))
    .await;
    g.idle(Duration::from_secs(5)).await;
    g.expect("a rat", line(ENGAGED, 21));
    g.output(&line("The thin giant rat lunges at you!", 21))
        .await;
    g.narrated("Fight: thin giant rat").await;
    g.round("The thin giant rat lunges at you!", 21).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "u", "rest", "a rat"]);
}

#[tokio::test(start_paused = true)]
async fn below_30_percent_the_bot_stops_itself() {
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.round("The large giant rat lunges at you!", 20).await;
    // 8 of 35 is 22%.
    g.output(&line("The large giant rat lunges at you!", 8))
        .await;
    g.narrated("Stopped at 22% HP: flee is not built yet; fight or run by hand")
        .await;
    // Nothing more: not even after the fight.
    g.output(&kill(8)).await;
    g.idle(Duration::from_secs(60)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat"]);
    let b = bot(&seen);
    assert_eq!(b.state, BotState::Stopped);
    assert!(
        seen.iter()
            .any(|m| matches!(m, ToGateway::Warning { text, .. }
            if text.starts_with("Stopped at 22% HP"))),
        "a warning"
    );
}

#[tokio::test(start_paused = true)]
async fn a_typed_line_pauses_after_the_action_in_flight_and_resume_resyncs() {
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.after_cmd = Some((
        "a rat".into(),
        ToClient::Sent {
            session: ID.into(),
            seq: 1,
            text: "say hi".into(),
        },
    ));
    let said = g.narrated("Paused: you took over").await;
    assert!(
        said.contains(&"You took over Aldric".to_owned()),
        "{said:?}"
    );
    // Paused: the fight goes on by itself, and nothing is sent after the kill.
    g.round("The large giant rat lunges at you!", 33).await;
    g.output(&kill(33)).await;
    g.idle(Duration::from_secs(60)).await;
    // Resume: re-sync (`st` ... `look`) first, then decide.
    g.expect("st", status(33));
    g.expect("look", arena(&[], 33));
    g.bot(BotAction::Resume).await;
    g.narrated("Resuming: re-syncing").await;
    g.narrated("Waiting here").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "st", "look"]);
    assert_eq!(bot(&seen).state, BotState::Running);
}

#[tokio::test(start_paused = true)]
async fn pause_and_stop_from_the_ui() {
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.bot(BotAction::Pause).await;
    g.narrated("Paused: asked to pause").await;
    g.bot(BotAction::Stop).await;
    g.narrated("Stopped: asked to stop").await;
    g.output(&kill(35)).await;
    g.idle(Duration::from_secs(60)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat"]);
    let b = bot(&seen);
    assert_eq!(
        (b.state, b.reason.as_deref()),
        (BotState::Stopped, Some("asked to stop"))
    );
}

#[tokio::test(start_paused = true)]
async fn an_unknown_combat_state_counts_as_engaged_until_the_game_is_quiet() {
    // The brain attaches mid-fight: the character's blows, but no `*Combat Engaged*` it saw.
    // The guard takes the unknown state as engaged; only once its blows stop does the bot
    // take the fight to have lapsed and attack.
    let blow = "You whap large giant rat for 6 damage!";
    let history = format!("{}{}", arena(&["large giant rat"], 35), line(blow, 35));
    let (mut g, run) = Game::start_with(SessionState::Realm, history).await;
    g.expect("st", status(35));
    g.expect("look", arena(&["large giant rat"], 34));
    g.bot(BotAction::Start).await;
    // The blows go on through the re-sync (its `exp` and `i` wait out their answers) and
    // after it.
    for _ in 0..8 {
        g.round(blow, 34).await;
    }
    // Quiet: the fight lapsed (pitfall 42); now it attacks.
    g.expect("a rat", line(ENGAGED, 34));
    g.narrated("Fight: large giant rat").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat"]);
    let said = notices(&seen);
    let waited = said.iter().position(|n| n == "Fight: in combat");
    let attacked = said.iter().position(|n| n == "Fight: large giant rat");
    assert!(waited.is_some() && waited < attacked, "{said:?}");
}

#[tokio::test(start_paused = true)]
async fn a_monster_attacking_a_character_that_is_not_fighting_is_fought() {
    // Its blows are not the character's: it is not engaged, so the bot engages at once
    // (seen live: a rat lunging at an idle character in the Arena).
    let (mut g, run) = in_arena(&["fat giant rat"]).await;
    g.output(&line("The fat giant rat lunges at you!", 35))
        .await;
    g.expect("st", status(35));
    g.expect("look", arena(&["fat giant rat"], 35));
    g.expect("a rat", line(ENGAGED, 35));
    g.bot(BotAction::Start).await;
    for _ in 0..3 {
        g.output(&line("The fat giant rat lunges at you!", 35))
            .await;
        g.idle(Duration::from_secs(2)).await;
    }
    let said = g.narrated("Fight: fat giant rat").await;
    assert!(!said.contains(&"Fight: in combat".to_owned()), "{said:?}");
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat"]);
}

#[tokio::test(start_paused = true)]
async fn a_refused_target_is_skipped_for_the_session() {
    let (mut g, run) = in_arena(&[]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line("You do not see rat here!", 35));
    g.expect("look", arena(&["large giant rat"], 35));
    g.narrated("Fight: large giant rat").await;
    g.narrated("Waiting here").await;
    g.idle(Duration::from_secs(30)).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "look"]);
}

#[tokio::test(start_paused = true)]
async fn a_refusal_the_game_words_otherwise_skips_the_target_too() {
    // `unbalanced`, one of the executor's other attack refusals: no `*Combat Engaged*` comes,
    // and the attack does not go again.
    let (mut g, run) = in_arena(&[]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect(
        "a rat",
        line(
            "Such an attack would result in a very unbalanced combat round.",
            35,
        ),
    );
    g.expect("look", arena(&["large giant rat"], 35));
    g.narrated("Fight: large giant rat").await;
    g.narrated("Waiting here").await;
    g.idle(Duration::from_secs(60)).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "look"]);
}

#[tokio::test(start_paused = true)]
async fn an_attack_never_answered_is_tried_again_then_left_alone() {
    // A bare prompt: neither engaged nor refused. Each try waits out the action's timeout.
    let (mut g, run) = in_arena(&[]).await;
    start(&mut g, &["large giant rat"], 35).await;
    for _ in 0..ATTACK_TRIES {
        g.expect("a rat", prompt(35));
        g.expect("look", arena(&["large giant rat"], 35));
    }
    g.narrated("Fight: large giant rat").await;
    g.narrated("Waiting here").await;
    g.idle(Duration::from_secs(60)).await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "look", "a rat", "look"]);
}

#[test]
fn every_attack_refusal_line_refuses_the_attack() {
    // Each refusal's own example line (patterns.json), through the parser and the executor.
    let registry = world().registry;
    for name in ATTACK_REFUSALS {
        let pattern = registry
            .patterns()
            .iter()
            .find(|p| p.name == name && p.template.is_none())
            .unwrap_or_else(|| panic!("no pattern {name}"));
        let mut parser = Parser::new(registry.clone());
        let mut events = parser.feed(&format!("{}\n", pattern.example));
        events.extend(parser.finish());
        let mut ex = Executor::default();
        let now = tokio::time::Instant::now();
        ex.submit(Action::new("a rat", Expect::Engaged), Some(false))
            .unwrap();
        ex.take_ready(now);
        for ev in &events {
            ex.event(&GameEvent::from_event(ev));
        }
        assert_eq!(
            ex.finished(true, now).map(|(_, o)| o),
            Some(Outcome::Refused(Refused::Target)),
            "{name}: {events:?}"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn below_30_percent_mid_walk_the_bot_stops_and_so_does_the_walk() {
    // Recover walks out of the room beyond the Arena (it spawns) to the Narrow Road: south,
    // then up. A monster hits the character on the way; at 22% the bot stops, and the walk
    // with it, without waiting for the bot's next tick (which a walk holds back).
    let beyond = |hp| at_hp(&room(BEYOND_ARENA), hp);
    let (mut g, run) = Game::start_with(SessionState::Realm, beyond(35)).await;
    g.idle(Duration::from_secs(20)).await;
    g.expect("st", status(20));
    g.expect("look", beyond(20));
    g.expect(
        "s",
        format!(
            "{}{}",
            line("The thin giant rat lunges at you!", 8),
            arena(&["thin giant rat"], 8)
        ),
    );
    g.bot(BotAction::Start).await;
    let said = g
        .narrated("Stopped at 22% HP: flee is not built yet; fight or run by hand")
        .await;
    assert!(
        said.iter().any(|n| n.starts_with("Recover: walking to")),
        "{said:?}"
    );
    g.idle(Duration::from_secs(60)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "s"]);
    let ui = as_ui_sees(&seen);
    assert_eq!(ui.bot.unwrap().state, BotState::Stopped);
    assert_eq!(ui.walking, None);
}

#[tokio::test(start_paused = true)]
async fn walk_to_pauses_the_bot() {
    let (mut g, run) = in_arena(&[]).await;
    start(&mut g, &[], 35).await;
    g.narrated("Waiting here").await;
    g.expect("u", room(NARROW_ROAD));
    g.walk_to(NARROW_ROAD).await;
    let said = g.narrated("Paused: you took over").await;
    assert!(
        said.contains(&"You took over Aldric".to_owned()),
        "{said:?}"
    );
    g.narrated("Arrived: Newhaven, Narrow Road").await;
    // Paused: nothing more goes, not even back to the Arena.
    g.idle(Duration::from_secs(60)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "u"]);
    let b = bot(&seen);
    assert_eq!(
        (b.state, b.reason.as_deref()),
        (BotState::Paused, Some("you took over"))
    );
}

#[tokio::test(start_paused = true)]
async fn walk_to_while_the_bots_look_is_in_flight_walks_once_it_is_answered() {
    // Seen live: the player asked for a walk while the bot's `look` after a kill was out. The
    // walk's first step waits for that answer (one action at a time), then goes.
    let (mut g, run) = in_arena(&["large giant rat"]).await;
    start(&mut g, &["large giant rat"], 35).await;
    g.expect("a rat", line(ENGAGED, 35));
    g.narrated("Fight: large giant rat").await;
    g.after_cmd = Some((
        "look".into(),
        ToClient::WalkTo {
            session: ID.into(),
            map: NARROW_ROAD.0,
            room: NARROW_ROAD.1,
        },
    ));
    g.expect("look", arena(&[], 35));
    g.expect("u", room(NARROW_ROAD));
    g.output(&kill(35)).await;
    g.narrated("Arrived: Newhaven, Narrow Road").await;
    g.idle(Duration::from_secs(30)).await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "a rat", "look", "u"]);
    assert_eq!(bot(&seen).state, BotState::Paused);
}

#[tokio::test(start_paused = true)]
async fn the_bot_needs_the_realm() {
    let (mut g, run) = Game::start_with(SessionState::Menu, arena(&[], 35)).await;
    g.bot(BotAction::Start).await;
    assert_eq!(
        g.narration().await,
        "bot: the character is not in the realm"
    );
    // Leaving the realm stops a running bot.
    g.set_state(SessionState::Realm).await;
    g.idle(Duration::from_secs(20)).await;
    start(&mut g, &[], 35).await;
    g.narrated("Waiting here").await;
    g.set_state(SessionState::Menu).await;
    g.narrated("Stopped: the session left the realm").await;
    let (seen, played) = g.close(run).await;
    assert_eq!(played, ["st", "look"]);
    assert_eq!(bot(&seen).state, BotState::Stopped);
}

// The policies, on the world pack.

#[test]
fn the_arena_spawns_and_the_narrow_road_is_where_to_rest() {
    let a = atlas();
    let anyone = wayfarer_world::Traveller::anyone();
    assert!(policy::spawns_here(&a, Some(ARENA), true));
    assert!(!policy::spawns_here(&a, Some(NARROW_ROAD), true));
    // Not sure where it is: as if it spawned.
    assert!(policy::spawns_here(&a, Some(NARROW_ROAD), false));
    assert_eq!(
        policy::rest_at(&a, Some(ARENA), true, &anyone),
        RestAt::Walk(NARROW_ROAD)
    );
    assert_eq!(
        policy::rest_at(&a, Some(NARROW_ROAD), true, &anyone),
        RestAt::Here
    );
    assert_eq!(
        policy::rest_at(&a, None, false, &anyone),
        RestAt::HereAnyway
    );
}

#[test]
fn targets_are_hostile_attackers_first_and_never_skipped() {
    let ms = &atlas().monsters;
    assert!(policy::hostile(ms, "large giant rat"));
    assert!(!policy::hostile(ms, "healer"), "an NPC");
    assert!(!policy::hostile(ms, "Thornwyn"), "no such monster");
    let here: Vec<String> = ["healer", "large giant rat", "fat giant rat"]
        .map(String::from)
        .to_vec();
    let none = BTreeSet::new();
    assert_eq!(
        policy::target(ms, &here, &[], &none),
        Some("large giant rat")
    );
    let attackers = vec!["fat giant rat".to_owned()];
    assert_eq!(
        policy::target(ms, &here, &attackers, &none),
        Some("fat giant rat")
    );
    let skip: BTreeSet<String> = ["fat giant rat".to_owned()].into();
    assert_eq!(
        policy::target(ms, &here, &attackers, &skip),
        Some("large giant rat")
    );
    assert!(policy::is_one_of(
        ms,
        "fat giant rat",
        &["giant rat".into()]
    ));
}
