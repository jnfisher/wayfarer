//! The test-character pool (`fixtures.toml`) and the generated passwords
//! (`.credentials.toml`, untracked).

use std::collections::{BTreeMap, HashSet};
use std::io::{self, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

/// `fixtures.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pool {
    #[serde(rename = "character")]
    pub characters: Vec<Fixture>,
}

/// One test character (`[[character]]`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    /// The BBS user id, which the editor offers as the given name.
    pub account: String,
    pub family: String,
    pub race: String,
    pub class: String,
    pub stats: Stats,
}

/// The stat totals to set in the editor, in the editor's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stats {
    pub strength: u16,
    pub intellect: u16,
    pub willpower: u16,
    pub agility: u16,
    pub health: u16,
    pub charm: u16,
}

impl Stats {
    /// Name and total, in the editor's field order.
    pub fn in_order(&self) -> [(&'static str, u16); 6] {
        [
            ("strength", self.strength),
            ("intellect", self.intellect),
            ("willpower", self.willpower),
            ("agility", self.agility),
            ("health", self.health),
            ("charm", self.charm),
        ]
    }
}

impl Fixture {
    /// The full name the game will show: the given name upper-cases its first letter.
    pub fn name(&self) -> String {
        let mut given = self.account.clone();
        given[..1].make_ascii_uppercase();
        format!("{given} {}", self.family).trim_end().to_owned()
    }

    /// A character that is not the pool's: an existing account, whose password is in the
    /// credentials file given (a gateway `credentials.toml`). Never signed up or created.
    pub fn external(user: &str) -> Self {
        Self {
            account: user.to_owned(),
            family: String::new(),
            race: String::new(),
            class: String::new(),
            stats: Stats {
                strength: 0,
                intellect: 0,
                willpower: 0,
                agility: 0,
                health: 0,
                charm: 0,
            },
        }
    }

    pub fn is_external(&self) -> bool {
        self.race.is_empty()
    }
}

impl Pool {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let pool: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        if pool.characters.is_empty() {
            return Err("no [[character]] listed".into());
        }
        let mut seen = HashSet::new();
        for c in &pool.characters {
            // The editor's rules (kb/connection.md, "Stat editor rules").
            let letters = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphabetic());
            if !letters(&c.account) || c.account.len() > 10 {
                return Err(format!(
                    "account {:?}: the given name must be 1-10 letters",
                    c.account
                ));
            }
            if !letters(&c.family) || c.family.len() > 18 {
                return Err(format!(
                    "{}: the family name must be 1-18 letters",
                    c.account
                ));
            }
            if !seen.insert(c.account.to_ascii_lowercase()) {
                return Err(format!("account {:?} is listed twice", c.account));
            }
            if let Some((stat, v)) = c
                .stats
                .in_order()
                .into_iter()
                .find(|(_, v)| !(10..=250).contains(v))
            {
                return Err(format!("{}: {stat} {v} is outside 10-250", c.account));
            }
        }
        Ok(pool)
    }

    /// Only the character whose account or given name is `name`, or all of them.
    pub fn only(self, name: Option<&str>) -> Result<Self, String> {
        let Some(name) = name else { return Ok(self) };
        let characters: Vec<_> = self
            .characters
            .into_iter()
            .filter(|c| c.account.eq_ignore_ascii_case(name))
            .collect();
        if characters.is_empty() {
            return Err(format!("no test character {name:?} in the pool"));
        }
        Ok(Self { characters })
    }
}

/// `.credentials.toml`: `[account.<user id>] password`. Written only by the live check.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Passwords {
    #[serde(default)]
    account: BTreeMap<String, Entry>,
    /// A gateway `credentials.toml`'s `[character.<id>] user, password`, read so that
    /// `--credentials "$WAYFARER_DATA_DIR/credentials.toml"` works on the rig. Never written.
    #[serde(default, skip_serializing)]
    character: BTreeMap<String, Login>,
}

/// One of a gateway `credentials.toml`'s logins. `Debug` never shows the password.
#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Login {
    user: String,
    password: String,
}

impl std::fmt::Debug for Login {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Login {{ user: {:?}, password: ******** }}", self.user)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    password: String,
}

impl Passwords {
    /// The file's passwords; none if it doesn't exist yet.
    pub fn load(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// The password for the BBS user id `account`: the live check's own, else a gateway
    /// login for that user.
    pub fn get(&self, account: &str) -> Option<&str> {
        self.account
            .get(account)
            .map(|e| e.password.as_str())
            .or_else(|| {
                self.character
                    .values()
                    .find(|l| l.user.eq_ignore_ascii_case(account))
                    .map(|l| l.password.as_str())
            })
    }

    /// The BBS user id of the gateway login for character `id` (any case).
    pub fn character_user(&self, id: &str) -> Option<&str> {
        self.character
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(id))
            .map(|(_, l)| l.user.as_str())
    }

    pub fn set(&mut self, account: &str, password: String) {
        self.account.insert(account.to_owned(), Entry { password });
    }

    /// Write the file, readable by its owner only.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let text = format!(
            "# The live check's generated passwords (tests/live/README.md). Not in git.\n{}",
            toml::to_string(self).map_err(io::Error::other)?
        );
        write_private(path, &text)
    }
}

/// Write `text` to `path` with mode 0600 (the mode is set again if the file already existed).
pub fn write_private(path: &Path, text: &str) -> io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(text.as_bytes())
}

/// A new password: 9 lower-case letters and digits (MBBSEmu's limit is 5-9 characters).
pub fn generate_password() -> String {
    const ALPHABET: &[u8] = b"abcdefghijkmnpqrstuvwxyz23456789";
    let mut bytes = [0u8; 9];
    getrandom::fill(&mut bytes).expect("the system's random source");
    bytes
        .iter()
        .map(|b| ALPHABET[usize::from(*b) % ALPHABET.len()] as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE: &str = r#"
        [[character]]
        account = "waychecka"
        family = "Livecheck"
        race = "Human"
        class = "Warrior"
        stats = { strength = 60, intellect = 50, willpower = 50, agility = 50, health = 60, charm = 50 }
    "#;

    #[test]
    fn the_repository_pool_parses() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures.toml");
        let pool = Pool::load(&path).unwrap();
        assert!(
            pool.characters.len() >= 2,
            "one per concurrent card, at least two"
        );
    }

    #[test]
    fn a_character_parses_with_its_name_and_stats_in_editor_order() {
        let pool = Pool::parse(ONE).unwrap();
        let c = &pool.characters[0];
        assert_eq!(c.name(), "Waychecka Livecheck");
        assert_eq!(c.stats.in_order()[0], ("strength", 60));
        assert_eq!(c.stats.in_order()[5], ("charm", 50));
    }

    #[test]
    fn names_and_stats_follow_the_editor_rules() {
        let bad = |from: &str, to: &str| Pool::parse(&ONE.replace(from, to)).unwrap_err();
        assert!(bad("waychecka", "way probe").contains("1-10 letters"));
        assert!(bad("waychecka", "waycheckabc").contains("1-10 letters"));
        assert!(bad("Livecheck", "Live2").contains("family name"));
        assert!(bad("strength = 60", "strength = 260").contains("outside 10-250"));
        assert!(bad("charm = 50", "charm = 50, luck = 3").contains("luck"));
        assert!(Pool::parse("").unwrap_err().contains("character"));
        let twice = format!("{ONE}{}", ONE.replace("waychecka", "WayCheckA"));
        assert!(Pool::parse(&twice).unwrap_err().contains("listed twice"));
    }

    #[test]
    fn only_picks_one_character_by_account() {
        let two = format!("{ONE}{}", ONE.replace("waychecka", "waycheckb"));
        let pool = Pool::parse(&two).unwrap();
        assert_eq!(pool.clone().only(None).unwrap().characters.len(), 2);
        let b = pool.clone().only(Some("WAYCHECKB")).unwrap();
        assert_eq!(b.characters[0].account, "waycheckb");
        assert!(pool.only(Some("nobody")).is_err());
    }

    #[test]
    fn passwords_round_trip_in_a_private_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".credentials.toml");
        let mut p = Passwords::load(&path).unwrap();
        assert_eq!(p.get("waychecka"), None);
        p.set("waychecka", "secret123".into());
        p.save(&path).unwrap();
        assert_eq!(
            Passwords::load(&path).unwrap().get("waychecka"),
            Some("secret123")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn a_gateway_credentials_file_gives_a_character_its_account_and_password() {
        let text = "[character.hero]\nuser = \"heroacct\"\npassword = \"pw123456\"\n";
        let p: Passwords = toml::from_str(text).unwrap();
        assert_eq!(p.character_user("Hero"), Some("heroacct"));
        assert_eq!(p.character_user("nobody"), None);
        assert_eq!(p.get("heroacct"), Some("pw123456"));
        assert!(!format!("{p:?}").contains("pw123456"));
        // Saving writes only the check's own accounts, never the gateway's logins.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.toml");
        p.save(&path).unwrap();
        assert!(!std::fs::read_to_string(&path).unwrap().contains("pw123456"));
        let ext = Fixture::external("heroacct");
        assert!(ext.is_external());
        assert_eq!(ext.name(), "Heroacct");
    }

    #[test]
    fn generated_passwords_fit_the_bbs_limit() {
        let a = generate_password();
        assert_eq!(a.len(), 9);
        assert!(
            a.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        );
        assert_ne!(a, generate_password());
    }
}
