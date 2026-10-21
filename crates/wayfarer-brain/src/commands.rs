//! What a typed line is to the game, for typing beside the bot (docs/brain.md "Typing beside
//! the bot", docs/plan/03-engine.md section 6): its first word through the game's own command
//! table, and from the command a [`Kind`]: talk and look, an act, an attack (forbidden while
//! engaged), or no command at all.
//!
//! The table is MajorMUD's parser as silvermere reproduces it (`commands/table.rs` there):
//! the game takes a word as soon as the letters typed so far name a command, case not
//! counting, and none once a letter does not continue a known word. Every accepted
//! abbreviation of a word is therefore a run of prefixes, from the shortest the game accepts
//! to the full word; each run is one [`CommandWord`]. A command whose forms are not one run has
//! an entry per run (`N`/`NORTH`, `Q`/`QUIT`, `PU[NCH]`/`PUR[G]`). The runs never overlap.
//! The oracle, `tests/fixtures/command-words/command_words.json`, is every letter sequence the
//! game tells apart; a test feeds each one through [`parse_command`].
//!
//! The game's quirks it keeps (`kb/screen-parsing.md` "Command words"): `AA`, `ALLOUT` and
//! `FORCE` are BASH and `AU` is ATTACK; `KIL`..`KILLLBLO` are KILLBLOW and `KILLBLOW` typed in
//! full is none; `N` and `NORTH` but not `NO`..`NORT`; `QU`..`QUIE` are QUIET; `PUR`, `PURG`
//! are PUNCH; `?` only alone; `'`, `-`, `>`, `/`, `VERB`, `VERS` with anything after; `BORROW`
//! with up to two more characters; `WHO` is SCAN.

use wayfarer_world::mm;

macro_rules! commands {
    ($($v:ident = $name:literal,)*) => {
        /// A command the game's parser tells apart: its `CMD_` code.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Cmd {
            $($v,)*
        }

        impl Cmd {
            /// Every command, in name order.
            pub const ALL: &[Cmd] = &[$(Cmd::$v,)*];

            /// The code's name without `CMD_`: `ATTACK`, `NE`.
            pub fn name(self) -> &'static str {
                match self {
                    $(Cmd::$v => $name,)*
                }
            }
        }
    };
}

commands! {
Action = "ACTION",
Aid = "AID",
Appraise = "APPRAISE",
Arm = "ARM",
Ask = "ASK",
Attack = "ATTACK",
Auction = "AUCTION",
Autocombat = "AUTOCOMBAT",
BackRank = "BACKRANK",
Backstab = "BACKSTAB",
BankBook = "BANKBOOK",
Bash = "BASH",
Borrow = "BORROW",
Break = "BREAK",
Brief = "BRIEF",
Broadcast = "BROADCAST",
Broadgang = "BROADGANG",
Buy = "BUY",
Cast = "CAST",
Close = "CLOSE",
Create = "CREATE",
Defend = "DEFEND",
Demote = "DEMOTE",
Deposit = "DEPOSIT",
DirectMsg = "DIRECTMSG",
Disarm = "DISARM",
Disband = "DISBAND",
Dismiss = "DISMISS",
Donate = "DONATE",
Down = "DOWN",
Drag = "DRAG",
Drink = "DRINK",
Drop = "DROP",
East = "EAST",
Eat = "EAT",
Equip = "EQUIP",
Exits = "EXITS",
Experience = "EXPERIENCE",
Feedback = "FEEDBACK",
Fire = "FIRE",
Follow = "FOLLOW",
Forget = "FORGET",
Forgive = "FORGIVE",
FrontRank = "FRONTRANK",
Get = "GET",
Give = "GIVE",
Global = "GLOBAL",
Gossip = "GOSSIP",
Greet = "GREET",
Guard = "GUARD",
Health = "HEALTH",
Help = "HELP",
Hide = "HIDE",
Hunt = "HUNT",
Identity = "IDENTITY",
Inventory = "INVENTORY",
Invite = "INVITE",
Invoke = "INVOKE",
Join = "JOIN",
Jumpkick = "JUMPKICK",
Keys = "KEYS",
Kick = "KICK",
Killblow = "KILLBLOW",
Leave = "LEAVE",
Light = "LIGHT",
List = "LIST",
Lock = "LOCK",
Look = "LOOK",
Map = "MAP",
MarkUp = "MARKUP",
Meditate = "MEDITATE",
Mesmerize = "MESMERIZE",
Midrank = "MIDRANK",
North = "NORTH",
NorthEast = "NE",
NorthWest = "NW",
Open = "OPEN",
Page = "PAGE",
Party = "PARTY",
PickLock = "PICKLOCK",
Play = "PLAY",
Powers = "POWERS",
Profile = "PROFILE",
Promote = "PROMOTE",
Punch = "PUNCH",
Purge = "PURGE",
Quiet = "QUIET",
Quit = "QUIT",
Read = "READ",
Ready = "READY",
Remove = "REMOVE",
Repay = "REPAY",
Reroll = "REROLL",
Research = "RESEARCH",
Rest = "REST",
Rob = "ROB",
Save = "SAVE",
Scan = "SCAN",
Search = "SEARCH",
Sell = "SELL",
Set = "SET",
Share = "SHARE",
Shoot = "SHOOT",
Smash = "SMASH",
Sneak = "SNEAK",
Sound = "SOUND",
South = "SOUTH",
SouthEast = "SE",
SouthWest = "SW",
Spells = "SPELLS",
Stash = "STASH",
StatLine = "STATLINE",
Status = "STATUS",
Stock = "STOCK",
Suicide = "SUICIDE",
Sysop = "SYSOP",
Take = "TAKE",
Talk = "TALK",
Tame = "TAME",
Text = "TEXT",
Throw = "THROW",
Topten = "TOPTEN",
Track = "TRACK",
Train = "TRAIN",
Uninvite = "UNINVITE",
Unready = "UNREADY",
Unstock = "UNSTOCK",
Up = "UP",
Use = "USE",
Verbose = "VERBOSE",
Version = "VERSION",
Wealth = "WEALTH",
Wear = "WEAR",
West = "WEST",
Whisper = "WHISPER",
Wield = "WIELD",
Withdraw = "WITHDRAW",}

/// One run of accepted abbreviations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandWord {
    /// The longest form, upper case.
    pub word: &'static str,
    /// The shortest accepted length: `word[..min]` is the shortest abbreviation.
    pub min: u8,
    pub code: Cmd,
    /// Any text after `word` is accepted too (`VERB`, `VERS`, `'`, `-`, `>`, `/`).
    pub open: bool,
}

const fn w(word: &'static str, min: u8, code: Cmd) -> CommandWord {
    CommandWord {
        word,
        min,
        code,
        open: false,
    }
}

const fn open(word: &'static str, min: u8, code: Cmd) -> CommandWord {
    CommandWord {
        word,
        min,
        code,
        open: true,
    }
}

/// The table: one entry per run of prefixes (the module doc).
pub const TABLE: &[CommandWord] = &[
    // leading characters
    open("'", 1, Cmd::Broadcast),
    open("-", 1, Cmd::Broadcast),
    w("?", 1, Cmd::Help),
    open(">", 1, Cmd::DirectMsg),
    open("/", 1, Cmd::Whisper),
    // A
    w("ATTACK", 1, Cmd::Attack),
    w("AU", 2, Cmd::Attack),
    w("AA", 2, Cmd::Bash),
    w("ALLOUT", 2, Cmd::Bash),
    w("ACTION", 2, Cmd::Action),
    w("AID", 2, Cmd::Aid),
    w("APPRAISE", 2, Cmd::Appraise),
    w("ARM", 2, Cmd::Arm),
    w("ASK", 2, Cmd::Ask),
    w("AUCTION", 3, Cmd::Auction),
    w("AUTOCOMBAT", 3, Cmd::Autocombat),
    // B
    w("BACKRANK", 5, Cmd::BackRank),
    w("BACKSTAB", 5, Cmd::Backstab),
    w("BS", 2, Cmd::Backstab),
    w("BALANCE", 3, Cmd::BankBook),
    w("BANKBOOK", 3, Cmd::BankBook),
    w("BASH", 3, Cmd::Bash),
    w("BG", 2, Cmd::Broadgang),
    w("BORROW", 2, Cmd::Borrow),
    w("BREAK", 3, Cmd::Break),
    w("BRIEF", 3, Cmd::Brief),
    w("BROADCAST", 2, Cmd::Broadcast),
    w("BROADGANG", 6, Cmd::Broadgang),
    w("BUY", 2, Cmd::Buy),
    // C
    w("CAST", 1, Cmd::Cast),
    w("CLOSE", 2, Cmd::Close),
    w("CREATE", 2, Cmd::Create),
    // D
    w("D", 1, Cmd::Down),
    w("DOWN", 3, Cmd::Down),
    w("DEFEND", 3, Cmd::Defend),
    w("DEMOTE", 3, Cmd::Demote),
    w("DEPOSIT", 3, Cmd::Deposit),
    w("DISARM", 2, Cmd::Disarm),
    w("DISBAND", 4, Cmd::Disband),
    w("DISMISS", 4, Cmd::Dismiss),
    w("DONATE", 3, Cmd::Donate),
    w("DRAG", 3, Cmd::Drag),
    w("DRINK", 3, Cmd::Drink),
    w("DROP", 3, Cmd::Drop),
    // E
    w("E", 1, Cmd::East),
    w("EAST", 3, Cmd::East),
    w("EAT", 3, Cmd::Eat),
    w("EQUIP", 2, Cmd::Equip),
    w("EXITS", 3, Cmd::Exits),
    w("EXPERIENCE", 3, Cmd::Experience),
    // F
    w("FEEDBACK", 2, Cmd::Feedback),
    w("FIRE", 2, Cmd::Fire),
    w("FOLLOW", 2, Cmd::Follow),
    w("FORCE", 4, Cmd::Bash),
    w("FORGET", 5, Cmd::Forget),
    w("FORGIVE", 4, Cmd::Forgive),
    w("FRONTRANK", 2, Cmd::FrontRank),
    // G
    w("GB", 2, Cmd::Broadgang),
    w("GET", 1, Cmd::Get),
    w("GIVE", 2, Cmd::Give),
    w("GLOBAL", 3, Cmd::Global),
    w("GOSSIP", 3, Cmd::Gossip),
    w("GREET", 2, Cmd::Greet),
    w("GUARD", 2, Cmd::Guard),
    // H
    w("HEALTH", 2, Cmd::Health),
    w("HELP", 3, Cmd::Help),
    w("HIDE", 3, Cmd::Hide),
    w("HUNT", 2, Cmd::Hunt),
    // I
    w("I", 1, Cmd::Inventory),
    w("INVENTORY", 4, Cmd::Inventory),
    w("IDENTITY", 2, Cmd::Identity),
    w("INVITE", 4, Cmd::Invite),
    w("INVOKE", 4, Cmd::Invoke),
    // J
    w("JOIN", 2, Cmd::Join),
    w("JUMPKICK", 2, Cmd::Jumpkick),
    // K
    w("KEYS", 2, Cmd::Keys),
    w("KICK", 3, Cmd::Kick),
    w("KILLLBLO", 3, Cmd::Killblow),
    // L
    w("LEAVE", 2, Cmd::Leave),
    w("LIGHT", 3, Cmd::Light),
    w("LIST", 3, Cmd::List),
    w("LOCK", 3, Cmd::Lock),
    w("LOOK", 1, Cmd::Look),
    // M
    w("MAP", 2, Cmd::Map),
    w("MARKUP", 3, Cmd::MarkUp),
    w("MEDITATE", 3, Cmd::Meditate),
    w("MESMERIZE", 2, Cmd::Mesmerize),
    w("MIDRANK", 2, Cmd::Midrank),
    // N
    w("N", 1, Cmd::North),
    w("NORTH", 5, Cmd::North),
    w("NE", 2, Cmd::NorthEast),
    w("NORTHEAST", 6, Cmd::NorthEast),
    w("NW", 2, Cmd::NorthWest),
    w("NORTHWEST", 6, Cmd::NorthWest),
    // O
    w("OPEN", 2, Cmd::Open),
    w("ORDER", 3, Cmd::Buy),
    // P
    w("PAGE", 3, Cmd::Page),
    w("PARTY", 3, Cmd::Party),
    w("PICKLOCK", 2, Cmd::PickLock),
    w("PLAY", 2, Cmd::Play),
    w("POWERS", 2, Cmd::Powers),
    w("PROFILE", 2, Cmd::Profile),
    w("PROMOTE", 4, Cmd::Promote),
    w("PUNCH", 2, Cmd::Punch),
    w("PURG", 3, Cmd::Punch),
    w("PURGE", 5, Cmd::Purge),
    // Q
    w("Q", 1, Cmd::Quit),
    w("QUIT", 4, Cmd::Quit),
    w("QUIET", 2, Cmd::Quiet),
    // R
    w("REA", 3, Cmd::Ready),
    w("READ", 4, Cmd::Read),
    w("READY", 5, Cmd::Ready),
    w("REMOVE", 3, Cmd::Remove),
    w("REPAY", 3, Cmd::Repay),
    w("REROLL", 6, Cmd::Reroll),
    w("RESEARCH", 4, Cmd::Research),
    w("REST", 4, Cmd::Rest),
    w("ROB", 2, Cmd::Rob),
    // S
    w("S", 1, Cmd::South),
    w("SOUTH", 5, Cmd::South),
    w("SE", 2, Cmd::SouthEast),
    w("SOUTHEAST", 6, Cmd::SouthEast),
    w("SW", 2, Cmd::SouthWest),
    w("SOUTHWEST", 6, Cmd::SouthWest),
    w("SAVE", 2, Cmd::Save),
    w("SCAN", 2, Cmd::Scan),
    w("SEARCH", 3, Cmd::Search),
    w("SELL", 3, Cmd::Sell),
    w("SET", 3, Cmd::Set),
    w("SHARE", 3, Cmd::Share),
    w("SHOOT", 3, Cmd::Shoot),
    w("SMASH", 3, Cmd::Smash),
    w("SNEAK", 2, Cmd::Sneak),
    w("SOUND", 4, Cmd::Sound),
    w("SPELLS", 2, Cmd::Spells),
    w("STASH", 4, Cmd::Stash),
    w("STATLINE", 5, Cmd::StatLine),
    w("STATUS", 2, Cmd::Status),
    w("STOCK", 3, Cmd::Stock),
    w("SUICIDE", 7, Cmd::Suicide),
    w("SYSOP", 2, Cmd::Sysop),
    // T
    w("TAKE", 3, Cmd::Take),
    w("TALK", 3, Cmd::Talk),
    w("TAME", 3, Cmd::Tame),
    w("TEXT", 2, Cmd::Text),
    w("THROW", 2, Cmd::Throw),
    w("TOPTEN", 2, Cmd::Topten),
    w("TRACK", 4, Cmd::Track),
    w("TRAIN", 4, Cmd::Train),
    // U
    w("UP", 1, Cmd::Up),
    w("UNINVITE", 3, Cmd::Uninvite),
    w("UNREADY", 3, Cmd::Unready),
    w("UNSTOCK", 3, Cmd::Unstock),
    w("USE", 2, Cmd::Use),
    // V
    open("VERB", 4, Cmd::Verbose),
    open("VERS", 4, Cmd::Version),
    // W
    w("W", 1, Cmd::West),
    w("WEST", 4, Cmd::West),
    w("WEALTH", 4, Cmd::Wealth),
    w("WEAR", 3, Cmd::Wear),
    w("WHISPER", 3, Cmd::Whisper),
    w("WHO", 3, Cmd::Scan),
    w("WIELD", 3, Cmd::Wield),
    w("WITHDRAW", 3, Cmd::Withdraw),
];

/// The command one typed word names, `None` when it is no command. Case does not matter.
pub fn parse_command(word: &str) -> Option<Cmd> {
    if word.is_empty() {
        return None;
    }
    let upper = word.to_ascii_uppercase();
    // The one run no (word, min) pair expresses: after the full word the game's parser tests
    // the 7th character and then the 9th, never the 8th.
    if let Some(tail) = upper.strip_prefix("BORROW") {
        return (tail.len() <= 2).then_some(Cmd::Borrow);
    }
    TABLE.iter().find_map(|e| {
        let fits = upper.len() >= usize::from(e.min)
            && if e.open {
                upper.starts_with(e.word)
            } else {
                e.word.starts_with(upper.as_str())
            };
        fits.then_some(e.code)
    })
}

/// What a typed line is to the bot beside it (docs/plan/03-engine.md section 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Talk and look: it goes between the bot's actions, and the bot carries on.
    Talk,
    /// It acts in the game: the bot pauses after the action in flight, then it goes.
    Act,
    /// An attack: held while combat is engaged, since it would restart the swing timer
    /// (`kb/combat.md`); otherwise an act.
    Attack,
    /// No command: a text exit's phrase or speech (`kb/screen-parsing.md` "Typing commands",
    /// step 5). Treated as an act, since `borrow skiff` moves.
    Unknown,
}

/// The kind of every command: the one place it is decided.
pub fn kind(cmd: Cmd) -> Kind {
    use Cmd::*;
    match cmd {
        // Talk.
        Broadcast | Gossip | Auction | Whisper | DirectMsg | Broadgang
        // Look: the character, the room, the players, the help.
        | Look | Status | Experience | Inventory | Health | Scan | Help | Wealth | Spells
        | Exits | Keys | Topten | Version
        // The player's own settings.
        | Set => Kind::Talk,
        Attack | Bash | Backstab | Killblow | Punch => Kind::Attack,
        // Codes with no routine, or whose routine answers "not a command" at once: the line
        // goes on to the room's text exits and to speech, like a word that is no command
        // (`kb/screen-parsing.md` "Command words"; silvermere's GLOBAL too).
        Sound | Throw | Fire | Donate | Play | Shoot | Take | Text | Autocombat | Defend | Hunt
        | Dismiss | Research | Guard | Identity | Borrow | Repay | Global => Kind::Unknown,
        _ => Kind::Act,
    }
}

/// A typed line, classified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Typed {
    /// The command its first word names.
    pub cmd: Option<Cmd>,
    pub kind: Kind,
}

/// Classify a line as the game would read it: its first word (up to the first space) through
/// [`parse_command`], then [`kind`]. Two arguments change the kind: `bash <dir>` bashes a door,
/// an act and not an attack; `look <dir>` shows the next room, which the position belief would
/// take for this one, so it is an act too. An empty line shows the prompt (or the room): talk;
/// `x` alone asks to leave: an act.
pub fn classify(line: &str) -> Typed {
    let mut words = line.split(' ').filter(|w| !w.is_empty());
    let Some(first) = words.next() else {
        return Typed {
            cmd: None,
            kind: Kind::Talk,
        };
    };
    // `x` alone asks to leave (the exit delay), before the parser sees the line.
    if first.eq_ignore_ascii_case("x") && words.clone().next().is_none() {
        return Typed {
            cmd: None,
            kind: Kind::Act,
        };
    }
    let cmd = parse_command(first);
    let at_dir = words.next().is_some_and(|w| mm::direction(w).is_some());
    let kind = match cmd {
        None => Kind::Unknown,
        Some(Cmd::Bash | Cmd::Look) if at_dir => Kind::Act,
        Some(c) => kind(c),
    };
    Typed { cmd, kind }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The oracle, read once per test: every entry's word must give its code.
    fn oracle() -> serde_json::Value {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/command-words/command_words.json"
        );
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    // The settings' list of attack commands (`combat.attack_command`) holds attacks only.
    #[test]
    fn every_attack_command_the_settings_offer_is_an_attack() {
        for word in wayfarer_settings::ATTACK_COMMANDS {
            assert_eq!(
                classify(&format!("{word} rat")).kind,
                Kind::Attack,
                "{word}"
            );
        }
    }

    // 1,761 words, each through the table's runs: milliseconds.
    #[test]
    fn every_oracle_entry_gives_its_code() {
        let oracle = oracle();
        let entries = oracle["entries"].as_array().unwrap();
        assert!(entries.len() > 1700, "{}", entries.len());
        for e in entries {
            let word = e["word"].as_str().unwrap();
            let want = e["code"].as_str().unwrap().strip_prefix("CMD_").unwrap();
            let got = parse_command(word).map_or("NONE", Cmd::name);
            assert_eq!(got, want, "{word:?} ({})", e["kind"]);
            // Case does not matter.
            let lower = parse_command(&word.to_ascii_lowercase()).map_or("NONE", Cmd::name);
            assert_eq!(lower, want, "{word:?} in lower case");
        }
        // Every code the oracle reaches is one of ours, and every one of ours is reached.
        let mut reached: Vec<&str> = oracle["codes_reachable"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_str().unwrap().strip_prefix("CMD_").unwrap())
            .filter(|c| *c != "NONE")
            .collect();
        reached.sort_unstable();
        let mut ours: Vec<&str> = Cmd::ALL.iter().map(|c| c.name()).collect();
        ours.sort_unstable();
        assert_eq!(ours, reached);
    }

    fn code(word: &str) -> Option<&'static str> {
        parse_command(word).map(Cmd::name)
    }

    #[test]
    fn the_quirks_the_kb_lists() {
        // `AA` is BASH, `AU` ATTACK.
        assert_eq!(code("AA"), Some("BASH"));
        assert_eq!(code("allout"), Some("BASH"));
        assert_eq!(code("forc"), Some("BASH"));
        assert_eq!(code("AU"), Some("ATTACK"));
        assert_eq!(code("a"), Some("ATTACK"));
        // `KIL`..`KILLLBLO` is KILLBLOW; the word in full is none.
        assert_eq!(code("KIL"), Some("KILLBLOW"));
        assert_eq!(code("killlblo"), Some("KILLBLOW"));
        assert_eq!(code("KILLBLOW"), None);
        assert_eq!(code("KI"), None);
        // `N` or `NORTH`, nothing between.
        assert_eq!(code("N"), Some("NORTH"));
        assert_eq!(code("north"), Some("NORTH"));
        for w in ["NO", "NOR", "NORT", "NORTHX"] {
            assert_eq!(code(w), None, "{w}");
        }
        // `?` alone.
        assert_eq!(code("?"), Some("HELP"));
        assert_eq!(code("?x"), None);
        // `BORROW` and up to two more characters.
        assert_eq!(code("BORROW"), Some("BORROW"));
        assert_eq!(code("borrowxy"), Some("BORROW"));
        assert_eq!(code("BORROWXYZ"), None);
        // The punctuation forms take anything after them.
        assert_eq!(code("'hello"), Some("BROADCAST"));
        assert_eq!(code("-hello"), Some("BROADCAST"));
        assert_eq!(code(">bob"), Some("DIRECTMSG"));
        assert_eq!(code("/bob"), Some("WHISPER"));
        assert_eq!(code("WHO"), Some("SCAN"));
        assert_eq!(code("WH"), None);
        assert_eq!(code("QU"), Some("QUIET"));
        assert_eq!(code("Q"), Some("QUIT"));
        assert_eq!(code("purg"), Some("PUNCH"));
        assert_eq!(code("purge"), Some("PURGE"));
    }

    fn kind_of(line: &str) -> Kind {
        classify(line).kind
    }

    #[test]
    fn kinds_as_the_plan_lists_them() {
        for line in [
            "gossip hi",
            "gos hi",
            "br hello all",
            "'hello",
            "-hello",
            ">bob hi",
            "/bob hi",
            "whisper bob hi",
            "auction sword",
            "look",
            "l",
            "look rat",
            "st",
            "stat",
            "exp",
            "i",
            "inve",
            "health",
            "who",
            "scan",
            "help",
            "?",
            "set statline full",
            "",
            "   ",
        ] {
            assert_eq!(kind_of(line), Kind::Talk, "{line:?}");
        }
        for line in [
            "n",
            "north",
            "se",
            "u",
            "d",
            "rest",
            "cast mmis rat",
            "c mmis rat",
            "get sword",
            "g sword",
            "drop sword",
            "buy sword",
            "sell sword",
            "wear vest",
            "wield sword",
            "open n",
            "search",
            "x",
            "train stats",
            "bash n",
            "look n",
            "l north",
        ] {
            assert_eq!(kind_of(line), Kind::Act, "{line:?}");
        }
        for line in [
            "a rat",
            "att rat",
            "attack",
            "au rat",
            "aa rat",
            "bash rat",
            "bs rat",
            "backstab rat",
            "kil rat",
            "pu rat",
            "purg rat",
        ] {
            assert_eq!(kind_of(line), Kind::Attack, "{line:?}");
        }
        for line in [
            "borrow skiff",
            "hello there",
            "say hi",
            "killblow rat",
            "nort",
            "?x",
        ] {
            assert_eq!(kind_of(line), Kind::Unknown, "{line:?}");
        }
        // `x` asks to leave before the parser sees it: an act, with no command.
        assert_eq!(classify("X").cmd, None);
        assert_eq!(kind_of("xy"), Kind::Unknown);
    }
}
