//! The gateway's files: `wayfarer.toml` (servers and characters), `credentials.toml` (account
//! names and passwords, kept apart so settings can be shared without them) and where the logs
//! go. The Accounts panel writes the first two back ([`save`]), and [`diff`] says which
//! sessions a save touches.

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default, rename = "server")]
    pub servers: Vec<Server>,
    #[serde(default, rename = "character")]
    pub characters: Vec<Character>,
}

/// A game server (`[[server]]`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Server {
    pub id: String,
    pub host: String,
    pub port: u16,
    /// The logon script preset, `"mbbsemu"` or `"silvermere"` ([`crate::logon`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    /// The server's own logon script, or the parts of the preset it replaces
    /// (`[server.logon]`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Character {
    /// The session id: letters, digits, `-` and `_` (it names the log directory).
    pub id: String,
    /// The display name.
    pub name: String,
    /// A `[[server]]` id.
    pub server: String,
    /// Connect when the gateway starts.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
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

    pub fn character(&self, id: &str) -> Option<&Character> {
        self.characters.iter().find(|c| c.id == id)
    }

    /// Check the config as [`Config::parse`] does.
    pub fn validate(&self) -> Result<(), String> {
        self.check()
    }

    /// The text of `wayfarer.toml` for this config, as the Accounts panel's save writes it.
    pub fn to_toml(&self) -> String {
        let body = toml::to_string(self).expect("a config always serializes");
        format!("{CONFIG_HEADER}\n{body}")
    }
}

/// What a saved `wayfarer.toml` starts with.
const CONFIG_HEADER: &str = "\
# wayfarer.toml: the game servers and the characters the gateway holds sessions for.
# Passwords go in credentials.toml beside this file, never here.
# Written by the web UI's Accounts panel: comments added by hand are not kept (the file before
# the last save is wayfarer.toml.bak).
";

/// What a saved `credentials.toml` starts with.
const CREDENTIALS_HEADER: &str = "\
# wayfarer credentials: one table per character id from wayfarer.toml. Keep this file private;
# it is separate from the settings so they can be shared without them.
# Written by the web UI's Accounts panel: comments added by hand are not kept.
";

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
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Credentials {
    #[serde(default)]
    character: BTreeMap<String, Login>,
}

/// One character's account. `Debug` never shows the password.
#[derive(Clone, PartialEq, Eq, Deserialize, Serialize)]
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

    pub fn set(&mut self, character: &str, login: Login) {
        self.character.insert(character.to_owned(), login);
    }

    /// The text of `credentials.toml`, as the Accounts panel's save writes it.
    pub fn to_toml(&self) -> String {
        let body = toml::to_string(self).expect("credentials always serialize");
        format!("{CREDENTIALS_HEADER}\n{body}")
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

/// Write `config` and `credentials` to their files, each through a temporary file renamed over
/// the old one; `credentials.toml` stays private (0600 on Unix). The old `wayfarer.toml` is
/// copied to `wayfarer.toml.bak` first. Nothing is renamed until both are written, so a failed
/// write leaves the old files as they were.
pub fn save(paths: &Paths, config: &Config, credentials: &Credentials) -> Result<(), ConfigError> {
    config.check().map_err(|message| ConfigError::Invalid {
        path: paths.config.clone(),
        message,
    })?;
    let err = |path: &Path| {
        let path = path.to_path_buf();
        move |source| ConfigError::Io { path, source }
    };
    if paths.config.exists() {
        let bak = paths.config.with_extension("toml.bak");
        std::fs::copy(&paths.config, &bak).map_err(err(&bak))?;
    }
    let config_tmp = temporary(&paths.config);
    let credentials_tmp = temporary(&paths.credentials);
    let written = write_new(&config_tmp, config.to_toml().as_bytes(), false)
        .map_err(err(&paths.config))
        .and_then(|()| {
            write_new(&credentials_tmp, credentials.to_toml().as_bytes(), true)
                .map_err(err(&paths.credentials))
        });
    if let Err(e) = written {
        let _ = std::fs::remove_file(&config_tmp);
        let _ = std::fs::remove_file(&credentials_tmp);
        return Err(e);
    }
    std::fs::rename(&config_tmp, &paths.config).map_err(err(&paths.config))?;
    std::fs::rename(&credentials_tmp, &paths.credentials).map_err(err(&paths.credentials))
}

/// `<file>.tmp` beside `path`.
fn temporary(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

/// Write `contents` to `path`, replacing any leftover file there; `private`: 0600 on Unix.
fn write_new(path: &Path, contents: &[u8], private: bool) -> io::Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if private {
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    }
    #[cfg(not(unix))]
    let _ = private;
    let mut file = options.open(path)?;
    file.write_all(contents)?;
    file.sync_all()
}

/// What a save changes, by character id ([`diff`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
    /// New characters: a session to add and connect.
    pub added: Vec<String>,
    /// Gone from the config: hang up and drop the session.
    pub removed: Vec<String>,
    /// Their server (its address or logon script) or login changed: hang up and log on again.
    pub reconnect: Vec<String>,
    /// Only the name or autostart changed: no reconnect.
    pub updated: Vec<String>,
    /// New characters and those whose BBS user changed: the next logon may make the account.
    pub new_user: Vec<String>,
}

/// What going from `old` to `new` changes for each character. Characters whose server and
/// login are the same in both, and whose name and autostart are too, are in no list.
pub fn diff(
    old: &Config,
    old_credentials: &Credentials,
    new: &Config,
    new_credentials: &Credentials,
) -> Changes {
    let mut changes = Changes::default();
    for c in &old.characters {
        if new.character(&c.id).is_none() {
            changes.removed.push(c.id.clone());
        }
    }
    for c in &new.characters {
        let login = new_credentials.get(&c.id);
        let Some(was) = old.character(&c.id) else {
            changes.added.push(c.id.clone());
            changes.new_user.push(c.id.clone());
            continue;
        };
        let old_login = old_credentials.get(&c.id);
        let server_changed =
            was.server != c.server || old.server(&was.server) != new.server(&c.server);
        if server_changed || old_login != login {
            changes.reconnect.push(c.id.clone());
            if old_login.map(|l| &l.user) != login.map(|l| &l.user) {
                changes.new_user.push(c.id.clone());
            }
        } else if was != c {
            changes.updated.push(c.id.clone());
        }
    }
    changes
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

    fn login(user: &str, password: &str) -> Login {
        Login {
            user: user.into(),
            password: password.into(),
        }
    }

    fn two() -> (Config, Credentials) {
        let config = Config::parse(
            "[[server]]\nid = \"s\"\nhost = \"h\"\nport = 23\npreset = \"silvermere\"\n\
             [[server]]\nid = \"t\"\nhost = \"h2\"\nport = 23\n\
             [[character]]\nid = \"a\"\nname = \"A\"\nserver = \"s\"\nautostart = true\n\
             [[character]]\nid = \"b\"\nname = \"B\"\nserver = \"s\"\n",
        )
        .unwrap();
        let mut creds = Credentials::default();
        creds.set("a", login("ua", "pa"));
        creds.set("b", login("ub", "pb"));
        (config, creds)
    }

    #[test]
    fn what_a_save_changes() {
        let (old, old_creds) = two();
        let same = diff(&old, &old_creds, &old, &old_creds);
        assert_eq!(same, Changes::default(), "untouched");

        type Change = fn(&mut Config, &mut Credentials);
        let cases: [(&str, Change, Changes); 7] = [
            (
                "rename",
                |c, _| c.characters[0].name = "Al".into(),
                Changes {
                    updated: vec!["a".into()],
                    ..Changes::default()
                },
            ),
            (
                "autostart",
                |c, _| c.characters[1].autostart = true,
                Changes {
                    updated: vec!["b".into()],
                    ..Changes::default()
                },
            ),
            (
                "password",
                |_, k| k.set("a", login("ua", "new")),
                Changes {
                    reconnect: vec!["a".into()],
                    ..Changes::default()
                },
            ),
            (
                "user",
                |_, k| k.set("b", login("other", "pb")),
                Changes {
                    reconnect: vec!["b".into()],
                    new_user: vec!["b".into()],
                    ..Changes::default()
                },
            ),
            (
                "host",
                |c, _| c.servers[0].host = "elsewhere".into(),
                Changes {
                    reconnect: vec!["a".into(), "b".into()],
                    ..Changes::default()
                },
            ),
            (
                "moved",
                |c, _| c.characters[0].server = "t".into(),
                Changes {
                    reconnect: vec!["a".into()],
                    ..Changes::default()
                },
            ),
            (
                "added and removed",
                |c, k| {
                    c.characters.remove(0);
                    c.characters.push(Character {
                        id: "n".into(),
                        name: "N".into(),
                        server: "t".into(),
                        autostart: false,
                    });
                    k.set("n", login("un", "pn"));
                },
                Changes {
                    added: vec!["n".into()],
                    removed: vec!["a".into()],
                    new_user: vec!["n".into()],
                    ..Changes::default()
                },
            ),
        ];
        for (what, change, want) in cases {
            let (mut new, mut creds) = two();
            change(&mut new, &mut creds);
            assert_eq!(diff(&old, &old_creds, &new, &creds), want, "{what}");
        }
    }

    #[test]
    fn saved_files_read_back_the_same() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::in_dir(dir.path());
        std::fs::write(&paths.config, "# old, by hand\n").unwrap();
        let (mut config, creds) = two();
        // A server's own logon script is kept.
        config.servers[1].logon = Some(
            toml::from_str("realm = 'R>'\n[[step]]\nexpect = 'Login:'\nsend = '{user}'\n").unwrap(),
        );
        save(&paths, &config, &creds).unwrap();
        assert_eq!(Config::load(&paths.config).unwrap(), config);
        assert_eq!(
            Credentials::load_or_create(&paths.credentials).unwrap(),
            creds
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("wayfarer.toml.bak")).unwrap(),
            "# old, by hand\n"
        );
        assert!(!permissions_too_wide(&paths.credentials).unwrap());
        assert!(!dir.path().join("wayfarer.toml.tmp").exists());

        // A config that doesn't check is refused, and the files stay as they were.
        let before = std::fs::read_to_string(&paths.config).unwrap();
        let mut bad = config.clone();
        bad.characters[0].server = "nope".into();
        assert!(
            save(&paths, &bad, &creds)
                .unwrap_err()
                .to_string()
                .contains("not listed")
        );
        assert_eq!(std::fs::read_to_string(&paths.config).unwrap(), before);
    }

    #[test]
    fn a_failed_write_leaves_the_old_files() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::in_dir(dir.path());
        let (config, creds) = two();
        save(&paths, &config, &creds).unwrap();
        let before = std::fs::read_to_string(&paths.config).unwrap();
        // credentials.toml's temporary file can't be made: a directory is in the way.
        let blocked = Paths {
            credentials: dir.path().join("sub"),
            ..paths.clone()
        };
        std::fs::create_dir_all(dir.path().join("sub.tmp/x")).unwrap();
        let mut changed = config.clone();
        changed.characters[0].name = "Changed".into();
        assert!(save(&blocked, &changed, &creds).is_err());
        assert_eq!(std::fs::read_to_string(&paths.config).unwrap(), before);
        assert!(!dir.path().join("wayfarer.toml.tmp").exists());
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
