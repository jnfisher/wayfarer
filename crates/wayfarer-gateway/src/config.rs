//! The gateway's files: `wayfarer.toml` (servers and characters), `credentials.toml` (account
//! names and passwords, kept apart so settings can be shared without them) and where the logs
//! go.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::logon::{Script, ScriptSpec};

/// Where the gateway's files are. Every path can be set on its own (tests, flags).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub config: PathBuf,
    pub credentials: PathBuf,
    /// Holds `<character id>/output.log`, `raw.bin` and `sent.bin`.
    pub logs: PathBuf,
}

impl Paths {
    /// `wayfarer.toml`, `credentials.toml` and `logs/` in `dir`.
    pub fn in_dir(dir: impl AsRef<Path>) -> Self {
        let dir = dir.as_ref();
        Self {
            config: dir.join("wayfarer.toml"),
            credentials: dir.join("credentials.toml"),
            logs: dir.join("logs"),
        }
    }

    /// A config file given by path (`--config`): credentials and logs go beside it.
    pub fn for_config(config: impl Into<PathBuf>) -> Self {
        let config = config.into();
        let dir = config.parent().map(Path::to_path_buf).unwrap_or_default();
        Self {
            config,
            ..Self::in_dir(dir)
        }
    }

    /// The user's data directory for wayfarer (on Linux `~/.local/share/wayfarer`), if the
    /// platform has one.
    pub fn default_data_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "wayfarer").map(|d| d.data_dir().to_path_buf())
    }
}

/// `wayfarer.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default, rename = "server")]
    pub servers: Vec<Server>,
    #[serde(default, rename = "character")]
    pub characters: Vec<Character>,
}

/// A game server (`[[server]]`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Server {
    pub id: String,
    pub host: String,
    pub port: u16,
    /// The logon script preset, `"mbbsemu"` or `"silvermere"` ([`crate::logon`]).
    #[serde(default)]
    pub preset: Option<String>,
    /// The server's own logon script, or the parts of the preset it replaces
    /// (`[server.logon]`).
    #[serde(default)]
    pub logon: Option<ScriptSpec>,
}

impl Server {
    /// `host:port`, as shown to UIs.
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// The logon script: the preset with the inline parts over it, `None` if neither is given.
    pub fn script(&self) -> Result<Option<Script>, String> {
        Script::build(self.preset.as_deref(), self.logon.as_ref())
            .map_err(|e| format!("server {:?}: {e}", self.id))
    }
}

/// A character (`[[character]]`): one session.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Character {
    /// The session id: letters, digits, `-` and `_` (it names the log directory).
    pub id: String,
    /// The display name.
    pub name: String,
    /// A `[[server]]` id.
    pub server: String,
    /// Connect when the gateway starts.
    #[serde(default)]
    pub autostart: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("{path}: {message}")]
    Invalid { path: PathBuf, message: String },
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(&text).map_err(|message| ConfigError::Invalid {
            path: path.to_path_buf(),
            message,
        })
    }

    /// Parse and check `wayfarer.toml`'s text.
    pub fn parse(text: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        config.check()?;
        Ok(config)
    }

    fn check(&self) -> Result<(), String> {
        let mut servers = HashSet::new();
        for s in &self.servers {
            check_id("server", &s.id)?;
            if !servers.insert(s.id.as_str()) {
                return Err(format!("server {:?} is listed twice", s.id));
            }
            if s.host.is_empty() {
                return Err(format!("server {:?} has no host", s.id));
            }
            if s.port == 0 {
                return Err(format!("server {:?} has port 0", s.id));
            }
            s.script()?;
        }
        let mut characters = HashSet::new();
        for c in &self.characters {
            check_id("character", &c.id)?;
            if !characters.insert(c.id.as_str()) {
                return Err(format!("character {:?} is listed twice", c.id));
            }
            if !servers.contains(c.server.as_str()) {
                return Err(format!(
                    "character {:?} names server {:?}, which is not listed",
                    c.id, c.server
                ));
            }
        }
        Ok(())
    }

    pub fn server(&self, id: &str) -> Option<&Server> {
        self.servers.iter().find(|s| s.id == id)
    }
}

fn check_id(kind: &str, id: &str) -> Result<(), String> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(format!(
            "{kind} id {id:?} must be 1-64 letters, digits, '-' or '_'"
        ))
    }
}

/// `credentials.toml`: `[character.<id>] user, password`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Credentials {
    #[serde(default)]
    character: BTreeMap<String, Login>,
}

/// One character's account. `Debug` never shows the password.
#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Login {
    pub user: String,
    pub password: String,
}

impl fmt::Debug for Login {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Login")
            .field("user", &self.user)
            .field("password", &"********")
            .finish()
    }
}

/// What a new `credentials.toml` holds.
const CREDENTIALS_TEMPLATE: &str = "\
# wayfarer credentials: one table per character id from wayfarer.toml. Keep this file private;
# it is separate from the settings so they can be shared without it.
#
# [character.example]
# user = \"ACCOUNT\"
# password = \"PASSWORD\"
";

impl Credentials {
    pub fn get(&self, character: &str) -> Option<&Login> {
        self.character.get(character)
    }

    /// Parse `credentials.toml`'s text. The error never quotes the file, which holds
    /// passwords; it gives the line.
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| {
            let line = e.span().map(|span| {
                let before = &text.as_bytes()[..span.start.min(text.len())];
                1 + before.iter().filter(|&&b| b == b'\n').count()
            });
            match line {
                Some(line) => format!("not valid at line {line} (contents not shown)"),
                None => "not valid (contents not shown)".to_owned(),
            }
        })
    }

    /// Read `credentials.toml`, creating an empty one (mode 0600 on Unix) if there is none,
    /// and warn if its permissions let anyone else read it.
    pub fn load_or_create(path: &Path) -> Result<Self, ConfigError> {
        let io_err = |source| ConfigError::Io {
            path: path.to_path_buf(),
            source,
        };
        if !path.exists() {
            create_private(path, CREDENTIALS_TEMPLATE.as_bytes()).map_err(io_err)?;
            tracing::info!("created {}", path.display());
        }
        if permissions_too_wide(path).map_err(io_err)? {
            tracing::warn!(
                "{} can be read by other users; make it private (chmod 600)",
                path.display()
            );
        }
        let text = std::fs::read_to_string(path).map_err(io_err)?;
        Self::parse(&text).map_err(|message| ConfigError::Invalid {
            path: path.to_path_buf(),
            message,
        })
    }
}

/// Create a new file only its owner can read and write (0600 on Unix).
fn create_private(path: &Path, contents: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(contents)
}

/// Whether anyone but the owner may read or write `path` (always false off Unix).
pub fn permissions_too_wide(path: &Path) -> io::Result<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Ok(std::fs::metadata(path)?.permissions().mode() & 0o077 != 0)
    }
    #[cfg(not(unix))]
    {
        std::fs::metadata(path).map(|_| false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE_CONFIG: &str = include_str!("../../../examples/wayfarer.toml");
    const EXAMPLE_CREDENTIALS: &str = include_str!("../../../examples/credentials.toml");

    #[test]
    fn examples_parse() {
        let config = Config::parse(EXAMPLE_CONFIG).unwrap();
        assert!(!config.characters.is_empty());
        for c in &config.characters {
            assert!(config.server(&c.server).is_some());
        }
        let creds = Credentials::parse(EXAMPLE_CREDENTIALS).unwrap();
        for c in &config.characters {
            assert!(creds.get(&c.id).is_some(), "{} has no example login", c.id);
        }
    }

    #[test]
    fn full_config() {
        let config = Config::parse(
            r#"
            [[server]]
            id = "home"
            host = "bbs.example.org"
            port = 23
            preset = "mbbsemu"

            [[character]]
            id = "kasimir"
            name = "Kasimir"
            server = "home"
            autostart = true

            [[character]]
            id = "second_one"
            name = "Other"
            server = "home"
            "#,
        )
        .unwrap();
        assert_eq!(config.servers[0].address(), "bbs.example.org:23");
        assert_eq!(config.servers[0].preset.as_deref(), Some("mbbsemu"));
        assert!(config.characters[0].autostart);
        assert!(!config.characters[1].autostart);
        assert!(config.servers[0].script().unwrap().is_some());
    }

    #[test]
    fn inline_logon_scripts() {
        let config = Config::parse(
            r#"
            [[server]]
            id = "odd"
            host = "bbs.example.org"
            port = 23

            [server.logon]
            realm = 're:\[HP=\d+\]:'

            [[server.logon.step]]
            expect = "Login:"
            send = "{user}"

            [[server.logon.step]]
            expect = "re:[Pp]assword:"
            send = "{password}"
            timeout = 10
            "#,
        )
        .unwrap();
        let script = config.servers[0].script().unwrap().unwrap();
        assert_eq!(script.steps.len(), 2);
        assert!(script.logoff.is_empty());
    }

    #[test]
    fn bad_configs_are_refused() {
        let server = "[[server]]\nid = \"s\"\nhost = \"h\"\nport = 23\n";
        let character = |id: &str, srv: &str| {
            format!("[[character]]\nid = \"{id}\"\nname = \"N\"\nserver = \"{srv}\"\n")
        };
        let cases = [
            (format!("{server}{}", character("a", "nope")), "not listed"),
            (
                format!("{server}{}{}", character("a", "s"), character("a", "s")),
                "twice",
            ),
            (format!("{server}{server}"), "twice"),
            (format!("{server}{}", character("../x", "s")), "must be"),
            (format!("{server}{}", character("", "s")), "must be"),
            (
                "[[server]]\nid = \"s\"\nhost = \"h\"\nport = 0\n".to_owned(),
                "port 0",
            ),
            (format!("{server}colour = \"red\"\n"), "unknown field"),
            (format!("{server}preset = \"nope\"\n"), "no logon preset"),
            (
                format!("{server}[server.logon]\nrealm = \"re:(\"\n"),
                "server \"s\"",
            ),
        ];
        for (text, want) in cases {
            let err = Config::parse(&text).unwrap_err();
            assert!(err.contains(want), "{text}\n=> {err}");
        }
    }

    #[test]
    fn credentials_never_show_the_password() {
        let creds = Credentials::parse(
            "[character.kasimir]\nuser = \"ACCOUNT\"\npassword = \"s3cret-pw\"\n",
        )
        .unwrap();
        let login = creds.get("kasimir").unwrap();
        assert_eq!(login.password, "s3cret-pw");
        assert!(!format!("{creds:?}").contains("s3cret-pw"));
        assert!(format!("{creds:?}").contains("ACCOUNT"));

        // A broken file's error names the line, not the contents.
        let err =
            Credentials::parse("[character.kasimir]\nuser = \"ACCOUNT\"\npassword = s3cret-pw\n")
                .unwrap_err();
        assert!(err.contains("line 3"), "{err}");
        assert!(!err.contains("s3cret-pw"), "{err}");
        let err = Credentials::parse("[character.kasimir]\nuser = \"A\"\npassword = 12345\n")
            .unwrap_err();
        assert!(!err.contains("12345"), "{err}");
    }

    #[test]
    fn paths() {
        let p = Paths::for_config("/data/w/wayfarer.toml");
        assert_eq!(p.credentials, Path::new("/data/w/credentials.toml"));
        assert_eq!(p.logs, Path::new("/data/w/logs"));
        assert_eq!(Paths::in_dir("/d").config, Path::new("/d/wayfarer.toml"));
    }

    #[test]
    fn credentials_file_is_created_private() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/credentials.toml");
        let creds = Credentials::load_or_create(&path).unwrap();
        assert_eq!(creds, Credentials::default());
        assert!(!permissions_too_wide(&path).unwrap());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert!(permissions_too_wide(&path).unwrap());
            // Still readable; the gateway warns rather than refusing.
            Credentials::load_or_create(&path).unwrap();
        }
    }
}
