//! The Accounts panel's view of the config: [`Accounts`] on the wire, `wayfarer.toml` and
//! `credentials.toml` in the gateway.

use std::collections::HashMap;

use wayfarer_proto::{AccountCharacter, AccountServer, Accounts};

use crate::config::{Character, Config, Credentials, Login, Server};
use crate::logon::PRESETS;
use crate::session::Signup;
use crate::{DEFAULT_EMAIL, DEFAULT_GENDER};

/// What the panel shows: every server and character, with the logins and each session's
/// `reason` (from `reason`, by character id).
pub fn to_wire(
    config: &Config,
    credentials: &Credentials,
    reason: impl Fn(&str) -> Option<String>,
) -> Accounts {
    Accounts {
        servers: config
            .servers
            .iter()
            .map(|s| AccountServer {
                id: s.id.clone(),
                host: s.host.clone(),
                port: s.port,
                preset: s.preset.clone(),
                custom_logon: s.logon.is_some(),
                can_sign_up: s
                    .script()
                    .ok()
                    .flatten()
                    .is_some_and(|s| s.signup.is_some()),
            })
            .collect(),
        characters: config
            .characters
            .iter()
            .map(|c| {
                let login = credentials.get(&c.id);
                AccountCharacter {
                    id: c.id.clone(),
                    name: c.name.clone(),
                    server: c.server.clone(),
                    autostart: c.autostart,
                    user: login.map(|l| l.user.clone()).unwrap_or_default(),
                    password: login.map(|l| l.password.clone()).unwrap_or_default(),
                    email: None,
                    gender: None,
                    reason: reason(&c.id),
                }
            })
            .collect(),
        presets: PRESETS.iter().map(|(name, _)| (*name).to_owned()).collect(),
    }
}

/// What a save asks for: the config (each server keeping its `[server.logon]` from `old`, which
/// the panel does not edit), the logins (a character with no user has none) and the sign-up
/// details by character id.
pub fn from_wire(
    accounts: &Accounts,
    old: &Config,
) -> (Config, Credentials, HashMap<String, Signup>) {
    let config = Config {
        servers: accounts
            .servers
            .iter()
            .map(|s| Server {
                id: s.id.trim().to_owned(),
                host: s.host.trim().to_owned(),
                port: s.port,
                preset: s.preset.clone().filter(|p| !p.is_empty()),
                logon: old.server(s.id.trim()).and_then(|o| o.logon.clone()),
            })
            .collect(),
        characters: accounts
            .characters
            .iter()
            .map(|c| Character {
                id: c.id.trim().to_owned(),
                name: c.name.trim().to_owned(),
                server: c.server.clone(),
                autostart: c.autostart,
            })
            .collect(),
    };
    let mut credentials = Credentials::default();
    let mut signups = HashMap::new();
    for c in &accounts.characters {
        let id = c.id.trim();
        if !c.user.trim().is_empty() {
            credentials.set(
                id,
                Login {
                    user: c.user.trim().to_owned(),
                    password: c.password.clone(),
                },
            );
        }
        let given = |v: &Option<String>, default: &str| {
            v.as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .unwrap_or(default)
                .to_owned()
        };
        signups.insert(
            id.to_owned(),
            Signup {
                email: given(&c.email, DEFAULT_EMAIL),
                gender: given(&c.gender, DEFAULT_GENDER).to_uppercase(),
            },
        );
    }
    (config, credentials, signups)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wire_and_the_files_agree() {
        let old = Config::parse(
            "[[server]]\nid = \"s\"\nhost = \"h\"\nport = 23\npreset = \"silvermere\"\n\
             [[server]]\nid = \"odd\"\nhost = \"h2\"\nport = 2323\n\
             [server.logon]\nrealm = 'R>'\n[[server.logon.step]]\nexpect = 'L:'\nsend = '{user}'\n\
             [[character]]\nid = \"a\"\nname = \"A\"\nserver = \"s\"\nautostart = true\n\
             [[character]]\nid = \"b\"\nname = \"B\"\nserver = \"odd\"\n",
        )
        .unwrap();
        let mut creds = Credentials::default();
        creds.set(
            "a",
            Login {
                user: "ua".into(),
                password: "pa".into(),
            },
        );
        let wire = to_wire(&old, &creds, |id| {
            (id == "a").then(|| "bad credentials".into())
        });
        assert_eq!(wire.presets, ["mbbsemu", "silvermere"]);
        assert!(wire.servers[0].can_sign_up);
        assert!(!wire.servers[0].custom_logon);
        assert!(wire.servers[1].custom_logon);
        assert!(
            !wire.servers[1].can_sign_up,
            "its own script has no sign-up"
        );
        assert_eq!(wire.characters[0].password, "pa");
        assert_eq!(
            wire.characters[0].reason.as_deref(),
            Some("bad credentials")
        );
        assert_eq!(wire.characters[1].user, "", "no login");

        let (config, credentials, signups) = from_wire(&wire, &old);
        assert_eq!(config, old, "the server's own logon script is kept");
        assert_eq!(credentials, creds);
        assert_eq!(
            signups["b"],
            Signup {
                email: "h@h.com".into(),
                gender: "F".into()
            }
        );

        let mut wire = wire;
        wire.characters[1].email = Some(" me@example.org ".into());
        wire.characters[1].gender = Some("m".into());
        wire.characters[1].name = " Bee ".into();
        let (config, _, signups) = from_wire(&wire, &old);
        assert_eq!(config.characters[1].name, "Bee");
        assert_eq!(
            signups["b"],
            Signup {
                email: "me@example.org".into(),
                gender: "M".into()
            }
        );
    }
}
