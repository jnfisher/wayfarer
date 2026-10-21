//! Settings (docs/plan/04-settings.md): the brain reads the thresholds it used to have as
//! constants from the settings in force for each character: fixed ones, or the layers of the
//! files in a settings directory. Against the fake gateway of `game/mod.rs`, on a paused clock.

mod game;

use std::path::Path;
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use game::arena::*;
use game::*;
use wayfarer_brain::{Brain, FixedClock, HIDDEN_FOR, MAX_SEARCHES};
use wayfarer_proto::BotAction;
use wayfarer_settings::{Layer, Overrides, Settings, SettingsDir};

fn settings(text: &str) -> Settings {
    let over = Overrides::parse(text).unwrap();
    Settings::resolve(&[(Layer::Character, &over)]).unwrap()
}

fn clock() -> Arc<FixedClock> {
    Arc::new(FixedClock(UNIX_EPOCH + Duration::from_secs(T0)))
}

fn brain(text: &str) -> Brain {
    Brain::new(world(), clock(), None).with_settings(settings(text))
}

/// A brain whose settings are the files in `dir`.
fn brain_in(dir: &Path) -> Brain {
    Brain::new(world(), clock(), None).with_settings_dir(SettingsDir::new(dir))
}

/// The Newhaven low-HP rest (`low_hp_rests_in_a_quiet_room_then_walks_back`) with `brain`:
/// 20 of 35 HP in the Arena, up to the Narrow Road to rest. The narration of the rest.
async fn rest_narration(brain: Brain, hp_steps: &[i32], end_hp: i32) -> String {
    let (mut g, run) = Game::start_brain(SessionState::Realm, arena(&[], 35), brain).await;
    g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
        .await;
    g.idle(Duration::from_secs(20)).await;
    start(&mut g, &[], 20).await;
    g.expect("u", at_hp(&room(NARROW_ROAD), 20));
    g.expect("rest", format!("You are now resting.\r\n{}", resting(20)));
    let said = loop {
        let n = g.narration().await;
        if n.starts_with("Recover: resting to") {
            break n;
        }
    };
    for &hp in hp_steps {
        g.output(&resting(hp)).await;
        g.idle(Duration::from_secs(20)).await;
    }
    g.expect("d", arena(&[], end_hp));
    g.output(&resting(end_hp)).await;
    g.narrated("Recover: rested; walking back to Newhaven, Arena")
        .await;
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["st", "look", "u", "rest", "d"]);
    said
}

#[tokio::test(start_paused = true)]
async fn the_rest_line_is_a_setting() {
    // Rest until 90: 32 of 35 is 91%.
    let said = rest_narration(brain("[health]\nrest_until_hp = 90\n"), &[24, 28], 32).await;
    assert_eq!(said, "Recover: resting to 90% in Newhaven, Narrow Road");
}

#[tokio::test(start_paused = true)]
async fn the_search_limit_is_a_setting() {
    // Three searches and no more: the walker blocks the hidden exit (the default is 300).
    assert_eq!(
        settings("").int("movement.search_limit"),
        MAX_SEARCHES as i64
    );
    let hidden_keep: Key = (9, 724);
    let keep_wall: Key = (9, 725);
    let (mut g, run) = Game::start_brain(
        SessionState::Realm,
        room(hidden_keep),
        brain("[movement]\nsearch_limit = 3\n"),
    )
    .await;
    g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
        .await;
    // Str 50: the doors of the long way round are beyond it too.
    g.output(&format!(
        "\r\nStrength:  50     Agility: 40          Tracking:        0\r\n{PROMPT}"
    ))
    .await;
    // Found by the player, then hidden again five minutes on: the walker searches.
    g.typed("search n").await;
    g.output(&format!(
        "search n\r\n{}",
        says("You found an exit to the north!")
    ))
    .await;
    g.idle(HIDDEN_FOR + Duration::from_secs(1)).await;
    // A `look` after the first empty search, then two more: three in all.
    let nothing = says("You notice nothing different to the north.");
    g.expect("search n", nothing.clone());
    g.expect("look", room(hidden_keep));
    g.expect("search n", nothing.clone());
    g.expect("search n", nothing);
    g.walk_to(keep_wall).await;
    let stop = loop {
        let n = g.narration().await;
        if n.starts_with("Stopped walking") {
            break n;
        }
    };
    assert!(
        stop.starts_with("Stopped walking: no hidden exit north found"),
        "{stop}"
    );
    let (_, played) = g.close(run).await;
    assert_eq!(played, ["search n", "look", "search n", "search n"]);
}

fn write(dir: &Path, file: &str, text: &str) {
    let path = dir.join(file);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

#[tokio::test(start_paused = true)]
async fn the_files_of_a_settings_directory_are_this_characters_layers() {
    // Everyone is Careful (rest below 70 to 90); the server asks for 93 and this character 90. The character
    // (`aldric`) is the layer that wins; the server's file (`bbs.example.org:23`) sits between.
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "settings.toml", "[bot]\nstyle = \"careful\"\n");
    write(
        dir.path(),
        "servers/bbs.example.org_23.toml",
        "[health]\nrest_until_hp = 93\n",
    );
    write(
        dir.path(),
        "characters/aldric.toml",
        "[health]\nrest_until_hp = 90\n",
    );
    // 32 of 35 is 91%: rested at 90.
    let said = rest_narration(brain_in(dir.path()), &[24, 28], 32).await;
    assert_eq!(said, "Recover: resting to 90% in Newhaven, Narrow Road");
}

#[tokio::test(start_paused = true)]
async fn another_characters_file_does_not_apply() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "characters/someone-else.toml",
        "[health]\nrest_until_hp = 90\n",
    );
    // The default 85%: 31 of 35 is 89%, rested; the others do not rest to 90.
    let said = rest_narration(brain_in(dir.path()), &[24, 28], 31).await;
    assert_eq!(said, "Recover: resting to 85% in Newhaven, Narrow Road");
}

#[tokio::test(start_paused = true)]
async fn a_file_that_is_not_valid_leaves_the_defaults_with_a_warning() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "characters/aldric.toml",
        "[health]\nflee_below_hp = 95\n",
    );
    let (mut g, run) =
        Game::start_brain(SessionState::Realm, arena(&[], 35), brain_in(dir.path())).await;
    // The warning may have come while the game was starting up: look through what was seen,
    // else wait for it.
    let is_it = |m: &ToGateway| match m {
        ToGateway::Warning { text, .. } if text.starts_with("Settings not used") => {
            Some(text.clone())
        }
        _ => None,
    };
    let warning = match g.seen.iter().find_map(is_it) {
        Some(w) => w,
        None => g.until(is_it).await,
    };
    assert!(warning.contains("characters/aldric.toml"), "{warning}");
    assert!(warning.contains("health.flee_below_hp is 95"), "{warning}");
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn the_careful_style_rests_to_ninety() {
    // Careful: rest below 70 to 90. 31 of 35 is 89%, short of it; 32 (91%) will do.
    let said = rest_narration(brain("[bot]\nstyle = \"careful\"\n"), &[24, 28, 31], 32).await;
    assert_eq!(said, "Recover: resting to 90% in Newhaven, Narrow Road");
}

#[tokio::test(start_paused = true)]
async fn an_edited_settings_file_is_read_again_and_what_changed_is_said() {
    let dir = tempfile::tempdir().unwrap();
    let (mut g, run) =
        Game::start_brain(SessionState::Realm, arena(&[], 35), brain_in(dir.path())).await;
    g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
        .await;
    g.idle(Duration::from_secs(10)).await;
    std::fs::write(
        dir.path().join("settings.toml"),
        "[spells]\nheal = \"off\"\n[health]\nflee_below_hp = 30\n",
    )
    .unwrap();
    g.narrated("Settings reloaded: health.flee_below_hp 25 → 30, spells.heal \"auto\" → \"off\"")
        .await;
    // A file that is not valid leaves them as they are, with a warning.
    std::fs::write(
        dir.path().join("settings.toml"),
        "[spells]\nheal = \"sometimes\"\n",
    )
    .unwrap();
    let said = g.narration().await;
    assert!(
        said.starts_with("Settings changed but not used, the earlier settings stay: "),
        "{said}"
    );
    // Mended, it reads again.
    std::fs::write(
        dir.path().join("settings.toml"),
        "[health]\nflee_below_hp = 28\n",
    )
    .unwrap();
    g.narrated("Settings reloaded: health.flee_below_hp 30 → 28, spells.heal \"off\" → \"auto\"")
        .await;
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn the_class_the_status_screen_shows_moves_the_hp_lines() {
    // The capture's status screen is a Warrior's (25 / 60 / 85, the defaults, so nothing is
    // said); a Mage's lines are 45 / 70 / 90, and the bot says so once it knows.
    let dir = tempfile::tempdir().unwrap();
    let (mut g, run) =
        Game::start_brain(SessionState::Realm, arena(&[], 35), brain_in(dir.path())).await;
    g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
        .await;
    g.idle(Duration::from_secs(20)).await;
    g.expect("st", status(35).replace("Warrior    ", "Mage       "));
    g.expect("look", arena(&[], 35));
    g.bot(BotAction::Start).await;
    g.narrated("Settings for a Mage: flee below 45%, rest below 70% to 90%")
        .await;
    g.idle(Duration::from_secs(30)).await;
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn a_warriors_status_screen_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut g, run) =
        Game::start_brain(SessionState::Realm, arena(&[], 35), brain_in(dir.path())).await;
    g.until(|m| matches!(m, ToGateway::MapView { .. }).then_some(()))
        .await;
    g.idle(Duration::from_secs(20)).await;
    start(&mut g, &[], 35).await;
    g.idle(Duration::from_secs(30)).await;
    let (seen, _) = g.close(run).await;
    let said = notices(&seen);
    assert!(
        !said.iter().any(|s| s.starts_with("Settings for")),
        "{said:?}"
    );
}
