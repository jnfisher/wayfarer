//! The Newhaven Arena of the `fight-arena` capture, for the tests that play the bot there
//! (`fight.rs`, `store.rs`): its rooms, prompts, status screen and kill, and the bot's controls.

use std::sync::OnceLock;

use wayfarer_proto::BotAction;

use super::*;

pub fn fight_capture() -> &'static str {
    static RAW: OnceLock<String> = OnceLock::new();
    RAW.get_or_init(|| {
        let raw =
            std::fs::read(common::repo("tests/fixtures/captures/fight-arena/raw.bin")).unwrap();
        wayfarer_world::cp437::decode(&raw)
    })
}

pub fn prompt(hp: i32) -> String {
    format!("\x1b[79D\x1b[K\x1b[0;37m[HP={hp}\x1b[0;37m]:")
}

/// The prompt while resting, as the `fight-arena` capture shows it.
pub fn resting(hp: i32) -> String {
    format!("\x1b[79D\x1b[K\x1b[0;37m[HP={hp}\x1b[0;37m (Resting) ]:")
}

/// A line of the game's, then the prompt.
pub fn line(text: &str, hp: i32) -> String {
    format!("\x1b[79D\x1b[K\x1b[1;31m{text}\r\n{}", prompt(hp))
}

/// `text` (a room from `game::room`) with the prompt at `hp`.
pub fn at_hp(text: &str, hp: i32) -> String {
    let cut = text.rfind(PROMPT).unwrap();
    format!("{}{}", &text[..cut], prompt(hp))
}

/// The status screen of the `fight-arena` capture (Aldric, `Hits: 35/35`), then the prompt.
pub fn status(hp: i32) -> String {
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
pub fn arena(monsters: &[&str], hp: i32) -> String {
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

pub const ENGAGED: &str = "*Combat Engaged*";

/// The kill at the end of a fight with a giant rat, as the capture has it.
pub fn kill(hp: i32) -> String {
    [
        "The giant rat falls to the ground with a tortured squeak.",
        "You gain 9 experience.",
        "*Combat Off*",
    ]
    .map(|t| line(t, hp))
    .concat()
}

impl Game {
    pub async fn bot(&self, action: BotAction) {
        self.send(ToClient::BotControl {
            session: ID.into(),
            action,
        })
        .await;
    }

    /// A round of the fight: the monster's blow, and the clock moves on a few seconds.
    pub async fn round(&mut self, text: &str, hp: i32) {
        self.output(&line(text, hp)).await;
        self.idle(Duration::from_secs(4)).await;
    }
}

/// A brain attached in the Arena, its first re-sync done (with bare prompts).
pub async fn in_arena(monsters: &[&str]) -> (Game, Run) {
    let (mut g, run) = Game::start_with(SessionState::Realm, arena(monsters, 35)).await;
    g.idle(Duration::from_secs(20)).await;
    (g, run)
}

/// Start the bot: its re-sync's `st` and `look` are answered with `hp` and `monsters`.
pub async fn start(g: &mut Game, monsters: &[&str], hp: i32) {
    g.expect("st", status(hp));
    g.expect("look", arena(monsters, hp));
    g.bot(BotAction::Start).await;
    g.narrated("Starting in Newhaven, Arena: re-syncing").await;
}

/// The bot's state as the UIs see it.
pub fn bot(seen: &[ToGateway]) -> wayfarer_proto::Bot {
    as_ui_sees(seen).bot.expect("the bot on the character")
}

pub fn notices(seen: &[ToGateway]) -> Vec<String> {
    seen.iter()
        .filter_map(|m| match m {
            ToGateway::Notice { text, .. } | ToGateway::Warning { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}
