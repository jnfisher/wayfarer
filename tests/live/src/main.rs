//! `wayfarer-live`: the M0 scenario against a game server that is already running, run by
//! `scripts/live-check.sh` on the laptop and never in CI (tests/live/README.md).
//!
//! It claims a test character from the pool (creating its account and character the first
//! time), starts `wayfarer gateway` for it, and through the gateway's WebSocket checks that it
//! reaches the realm, that `st` shows the status screen, that stopping the gateway logs it off
//! cleanly, and that the next logon carries no punishment notice. Then it prints a summary for
//! the pull request. `--scenario brain` (M1) instead attaches `wayfarer brain` to that gateway
//! and checks its snapshot, its re-sync and that stopping it logs nobody off. `--scenario
//! walk` (M2) has that brain walk the character to a room a few steps away and back, and
//! `--scenario fight` walks it to the Newhaven Arena and starts the bot there.

mod brain;
mod claim;
mod fixtures;
mod gateway;
mod screen;

use std::net::ToSocketAddrs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use wayfarer_proto::SessionState;

use claim::{Attempt, claim};
use fixtures::{Fixture, Passwords, Pool, generate_password};
use gateway::{Gateway, SESSION, run_dir};

/// From connecting to the realm prompt, retries included (the gateway retries an account
/// whose last connection is still closing after 5 s, then 10 s).
const LOGON_TIMEOUT: Duration = Duration::from_secs(60);
/// The punishment notice for dropping in the realm (kb/connection.md).
const PUNISHED: &str = "disconnected while playing";
const START_SILVERMERE: &str = "cd ~/repos/wayfarer-dev/silvermere-wayfarer \
    && ~/repos/wayfarer-dev/silvermere/target/release/silvermere serve";

struct Args {
    addr: String,
    pool: PathBuf,
    credentials: PathBuf,
    wayfarer: PathBuf,
    out: PathBuf,
    only: Option<String>,
    scenario: Scenario,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scenario {
    /// M0: log on, a command, a clean log-off, not punished.
    M0,
    /// M1: the brain attaches, fills the snapshot, re-syncs, and stops with the character
    /// still in the realm.
    Brain,
    /// M2: the brain walks the character to a room and back, confirming each step.
    Walk,
    /// M2's "fight here": in the Newhaven Arena, the bot fights, then is stopped; the
    /// character walks out and logs off.
    Fight,
}

fn usage() -> ! {
    eprintln!(
        "usage: wayfarer-live --wayfarer BIN --out DIR [--addr HOST:PORT] [--pool FILE] \
         [--credentials FILE] [--char ACCOUNT] [--scenario m0|brain|walk|fight]\n(run it through \
         scripts/live-check.sh)"
    );
    std::process::exit(2);
}

fn args() -> Args {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut a = Args {
        addr: std::env::var("WAYFARER_LIVE_ADDR").unwrap_or_else(|_| "127.0.0.1:2323".into()),
        pool: here.join("fixtures.toml"),
        credentials: here.join(".credentials.toml"),
        wayfarer: PathBuf::new(),
        out: PathBuf::new(),
        only: None,
        scenario: Scenario::M0,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().unwrap_or_else(|| usage());
        match flag.as_str() {
            "--addr" => a.addr = value(),
            "--pool" => a.pool = value().into(),
            "--credentials" => a.credentials = value().into(),
            "--wayfarer" => a.wayfarer = value().into(),
            "--out" => a.out = value().into(),
            "--char" => a.only = Some(value()),
            "--scenario" => {
                a.scenario = match value().as_str() {
                    "m0" => Scenario::M0,
                    "brain" => Scenario::Brain,
                    "walk" => Scenario::Walk,
                    "fight" => Scenario::Fight,
                    _ => usage(),
                }
            }
            _ => usage(),
        }
    }
    if a.wayfarer.as_os_str().is_empty() || a.out.as_os_str().is_empty() {
        usage();
    }
    a
}

/// One line of the summary.
struct Step {
    name: String,
    ok: bool,
    secs: f64,
    note: String,
}

struct Ctx {
    args: Args,
    host: String,
    port: u16,
    /// Gateways started so far (each gets its own directory under `args.out`).
    runs: u32,
    steps: Vec<Step>,
}

impl Ctx {
    fn record(&mut self, name: &str, started: Instant, result: &Result<String, String>) {
        let (ok, note) = match result {
            Ok(n) => (true, n.clone()),
            Err(e) => (false, e.clone()),
        };
        self.steps.push(Step {
            name: name.to_owned(),
            ok,
            secs: started.elapsed().as_secs_f64(),
            note,
        });
    }

    /// `wayfarer.toml` for the one character. With `signup`, the logon script makes a new
    /// account through the `NEW` dialogue first, with the credentials' user and password.
    fn config(&self, fixture: &Fixture, signup: bool) -> String {
        format!(
            "[[server]]\nid = \"live\"\nhost = \"{}\"\nport = {}\npreset = \"silvermere\"\n{}\n\
             [[character]]\nid = \"{SESSION}\"\nname = \"{}\"\nserver = \"live\"\nautostart = true\n",
            self.host,
            self.port,
            if signup { SIGNUP } else { "" },
            fixture.name()
        )
    }

    async fn start(
        &mut self,
        label: &str,
        fixture: &Fixture,
        password: &str,
        signup: bool,
    ) -> Result<Gateway, String> {
        let dir = run_dir(&self.args.out, &mut self.runs, label);
        let credentials = format!(
            "[character.{SESSION}]\nuser = \"{}\"\npassword = \"{password}\"\n",
            fixture.account
        );
        Gateway::start(
            &self.args.wayfarer,
            &dir,
            &self.config(fixture, signup),
            &credentials,
        )
        .await
    }
}

/// The `NEW` dialogue as logon steps (silvermere-auth's prompts, which are MBBSEmu's), then on
/// into the game, where a new account meets character creation. Laid over the silvermere
/// preset, so its realm prompt and log-off stay.
const SIGNUP: &str = r#"
[server.logon]
realm_timeout = 120

[[server.logon.step]]
expect = "Username:"
send = "NEW"

[[server.logon.step]]
expect = "Max. 29"
send = "{user}"

[[server.logon.step]]
expect = "strong Password"
send = "{password}"

[[server.logon.step]]
expect = "confirm:"
send = "{password}"

[[server.logon.step]]
expect = "e-Mail"
send = "probe@example.org"

[[server.logon.step]]
expect = "gender"
send = "M"

[[server.logon.step]]
expect = "Make your selection"
send = "M"
optional = true

[[server.logon.step]]
expect = "[MAJORMUD]:"
send = "E"

[[server.logon.failure]]
expect = "Username is unavailable"
reason = "account exists"
retry = false

[[server.logon.failure]]
expect = "created by the sysop"
reason = "signup closed"
retry = false
"#;

/// Where a logon got to.
enum Landed {
    Realm,
    Creation,
    InUse,
    /// `Invalid Credentials`: a wrong password, or (silvermere says the same) no such account.
    BadCredentials,
    Refused(String),
}

fn landed(g: &Gateway) -> Option<Landed> {
    let t = g.transcript();
    if g.state == SessionState::Realm {
        Some(Landed::Realm)
    } else if t.contains("choose your race") {
        Some(Landed::Creation)
    } else if t.contains("already logged on") || t.contains("already logged in") {
        Some(Landed::InUse)
    } else if t.contains("Invalid Credentials") {
        Some(Landed::BadCredentials)
    } else if t.contains("Username is unavailable") {
        Some(Landed::Refused(
            "the account exists on the server with another password than .credentials.toml's"
                .into(),
        ))
    } else if t.contains("created by the sysop") {
        Some(Landed::Refused(
            "the server does not allow new accounts (compat.open_signup)".into(),
        ))
    } else if g.state == SessionState::Closed && g.last_seq() > 0 {
        Some(Landed::Refused("the gateway closed the session".into()))
    } else {
        None
    }
}

/// Save `password` for `account`, reloading first: a check running beside this one may have
/// saved its own.
fn save_password(path: &Path, account: &str, password: &str) -> Result<(), String> {
    let mut p = Passwords::load(path)?;
    p.set(account, password.to_owned());
    p.save(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Log `fixture` on, making its account and character first if need be.
async fn try_claim(ctx: &mut Ctx, fixture: &Fixture) -> Attempt<Gateway> {
    let saved = match Passwords::load(&ctx.args.credentials) {
        Ok(p) => p.get(&fixture.account).map(str::to_owned),
        Err(e) => return Attempt::Abort(e),
    };
    // The password is saved before the account is made, so it is never lost. A saved one
    // whose account was never made gets `Invalid Credentials`; then sign up with it.
    let (password, mut signup) = match saved {
        Some(p) => (p, false),
        None => {
            let p = generate_password();
            if let Err(e) = save_password(&ctx.args.credentials, &fixture.account, &p) {
                return Attempt::Abort(e);
            }
            (p, true)
        }
    };
    let started = Instant::now();
    let (mut gw, creation) = loop {
        let mut gw = match ctx.start("logon", fixture, &password, signup).await {
            Ok(g) => g,
            Err(e) => return Attempt::Abort(e),
        };
        let landed = gw
            .wait_for(LOGON_TIMEOUT, "the realm or character creation", landed)
            .await;
        let refused = match landed {
            Ok(Landed::Realm) => break (gw, false),
            Ok(Landed::Creation) => break (gw, true),
            Ok(Landed::BadCredentials) if !signup => {
                gw.stop().await;
                signup = true;
                continue;
            }
            Ok(Landed::BadCredentials) => Attempt::Unusable("bad credentials".into()),
            Ok(Landed::InUse) => Attempt::InUse,
            Ok(Landed::Refused(why)) | Err(why) => Attempt::Unusable(why),
        };
        gw.stop().await;
        return refused;
    };
    let mut note = match (signup, creation) {
        (true, _) => format!(
            "account {} made through NEW, at character creation",
            fixture.account
        ),
        (false, true) => "at character creation".to_owned(),
        (false, false) => "in the realm".to_owned(),
    };
    if gw.transcript().contains(PUNISHED) {
        note.push_str("; punished for an earlier drop");
    }
    ctx.record("log on", started, &Ok(note));
    if creation {
        let started = Instant::now();
        let made = create_character(&mut gw, fixture).await;
        ctx.record("create character", started, &made);
    }
    Attempt::Claimed(gw)
}

/// Make the character through the game's own editor, from the race list to the realm
/// (kb/connection.md, "Character creation" and "Driving the creation editor over ANSI").
async fn create_character(gw: &mut Gateway, f: &Fixture) -> Result<String, String> {
    let wait = Duration::from_secs(15);
    let quiet = Duration::from_millis(300);
    let mut from = 0;
    for (list, name, next) in [
        ("race", &f.race, "choose a class"),
        ("class", &f.class, "Lawful?"),
    ] {
        let text = gw
            .wait_for(wait, &format!("the {list} list"), |g| {
                let t = g.text_after(from);
                screen::plain(&t)
                    .contains(&format!("choose your {list}"))
                    .then_some(t)
            })
            .await?;
        let n = screen::list_number(&text, name)
            .ok_or_else(|| format!("no {list} {name:?} in the game's list"))?;
        from = gw.last_seq();
        gw.send_line(&n.to_string()).await?;
        let seen = from;
        gw.wait_for(wait, next, |g| {
            screen::plain(&g.text_after(seen))
                .contains(next)
                .then_some(())
        })
        .await?;
    }
    gw.send_line("N").await?;

    // The editor: Enter accepts a field; the cursor shows which field is active.
    let at = |row: u16, col: u16| {
        move |g: &Gateway| screen::last_cursor(&g.text_after(0)) == Some((row, col))
    };
    gw.wait_quiet(wait, quiet, "the editor's given name field", at(4, 20))
        .await?;
    gw.send_keys(b"\r").await?;
    gw.wait_quiet(wait, quiet, "the family name field", at(5, 20))
        .await?;
    gw.send_keys(f.family.as_bytes()).await?;
    gw.send_keys(b"\r").await?;
    gw.wait_quiet(wait, quiet, "the strength field", at(9, 34))
        .await?;
    for (i, (stat, value)) in f.stats.in_order().into_iter().enumerate() {
        // Each field takes the total; the first key typed replaces what is there.
        gw.send_keys(value.to_string().as_bytes()).await?;
        gw.send_keys(b"\r").await?;
        let next = if i == 5 {
            (16, 22)
        } else {
            (10 + i as u16, 34)
        };
        gw.wait_quiet(
            wait,
            quiet,
            &format!("{stat} {value} accepted"),
            at(next.0, next.1),
        )
        .await?;
    }
    // Hair length, hair colour and eye colour as offered, then Exit (SAVE).
    for next in [(17, 22), (18, 22), (20, 14)] {
        gw.send_keys(b"\r").await?;
        gw.wait_quiet(wait, quiet, "the next looks field", at(next.0, next.1))
            .await?;
    }
    let saved = gw.last_seq();
    gw.send_keys(b"\r").await?;
    let outcome = gw
        .wait_for(Duration::from_secs(120), "the realm after SAVE", |g| {
            let t = screen::plain(&g.text_after(saved));
            if g.state == SessionState::Realm {
                Some(Ok(()))
            } else if t.contains("You may not use") || t.contains("start over") {
                Some(Err(gateway::tail(&t, 4)))
            } else {
                None
            }
        })
        .await?;
    outcome.map_err(|e| format!("the game refused the character (change the fixture):\n{e}"))?;
    Ok(format!("{}, {} {}", f.name(), f.race, f.class))
}

/// `st` shows the status screen.
async fn status_screen(gw: &mut Gateway) -> Result<String, String> {
    let seq = gw.last_seq();
    gw.send_line("st").await?;
    gw.wait_for(Duration::from_secs(15), "the status screen", |g| {
        let t = screen::plain(&g.text_after(seq));
        ["Name:", "Race:", "Class:", "Hits:"]
            .iter()
            .all(|w| t.contains(w))
            .then(|| {
                t.lines()
                    .find(|l| l.contains("Name:"))
                    .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
                    .unwrap_or_default()
            })
    })
    .await
}

async fn stop(ctx: &mut Ctx, name: &str, gw: Gateway) -> bool {
    let started = Instant::now();
    let stopped = gw.stop().await;
    let result = if stopped.clean {
        Ok(stopped.detail)
    } else {
        Err(stopped.detail)
    };
    ctx.record(name, started, &result);
    result.is_ok()
}

async fn run(ctx: &mut Ctx, pool: &Pool) -> Result<String, String> {
    let claimed = claim(&pool.characters, async |_, f: &Fixture| {
        try_claim(ctx, f).await
    })
    .await;
    let (index, mut gw, passed) = claimed
        .map_err(|passed| format!("no test character could be claimed: {}", passed.join("; ")))?;
    let fixture = &pool.characters[index];
    let mut who = format!(
        "{} (account {}, #{} of {})",
        fixture.name(),
        fixture.account,
        index + 1,
        pool.characters.len()
    );
    if !passed.is_empty() {
        who.push_str(&format!("; passed over {}", passed.join(", ")));
    }

    // 1. The realm (logging on, or making the character, got there or failed).
    if gw.state != SessionState::Realm {
        stop(ctx, "stop the gateway: log off", gw).await;
        return Err(who);
    }
    if ctx.args.scenario == Scenario::Fight {
        let fought = fight_scenario(ctx, &mut gw).await;
        let logged_off = stop(ctx, "stop the gateway: log off", gw).await;
        return if fought && logged_off {
            Ok(who)
        } else {
            Err(who)
        };
    }
    if ctx.args.scenario == Scenario::Walk {
        let walked = walk_scenario(ctx, &mut gw).await;
        let logged_off = stop(ctx, "stop the gateway: log off", gw).await;
        return if walked && logged_off {
            Ok(who)
        } else {
            Err(who)
        };
    }
    if ctx.args.scenario == Scenario::Brain {
        let attached = brain_scenario(ctx, &mut gw).await;
        let logged_off = stop(ctx, "stop the gateway: log off", gw).await;
        return if attached && logged_off {
            Ok(who)
        } else {
            Err(who)
        };
    }

    // 2. A command.
    let started = Instant::now();
    let status = status_screen(&mut gw).await;
    ctx.record("`st` shows the status screen", started, &status);

    // 3. A clean log-off when the gateway stops.
    if !stop(ctx, "stop the gateway: log off", gw).await {
        return Err(who);
    }

    // 4. The next logon is not punished.
    let started = Instant::now();
    let password = Passwords::load(&ctx.args.credentials)?
        .get(&fixture.account)
        .map(str::to_owned)
        .ok_or("the password went missing from .credentials.toml")?;
    let mut gw = ctx.start("relogon", fixture, &password, false).await?;
    let again = gw
        .wait_for(LOGON_TIMEOUT, "the realm prompt", |g| {
            (g.state == SessionState::Realm).then_some(())
        })
        .await
        .and_then(|()| {
            let t = gw.transcript();
            match t.lines().find(|l| l.contains(PUNISHED)) {
                Some(line) => Err(format!("punished: {}", line.trim())),
                None => Ok("in the realm, no punishment notice".into()),
            }
        });
    ctx.record("log on again: not punished", started, &again);
    stop(ctx, "stop the gateway: log off", gw).await;
    Ok(who)
}

/// The brain scenario, with the gateway in the realm; whether it got as far as stopping the
/// brain.
async fn brain_scenario(ctx: &mut Ctx, gw: &mut Gateway) -> bool {
    let dir = run_dir(&ctx.args.out, &mut ctx.runs, "brain");
    let started = Instant::now();
    let world = brain::world_pack(&ctx.args.wayfarer, &dir).await;
    ctx.record(
        "build the world pack",
        started,
        &world
            .as_ref()
            .map(|p| p.display().to_string())
            .map_err(Clone::clone),
    );
    let Ok(world) = world else { return false };

    // 1. It attaches.
    let started = Instant::now();
    let wayfarer = ctx.args.wayfarer.clone();
    let running = match brain::Brain::start(&wayfarer, &dir, "brain", &gw.addr, &world).await {
        Ok(b) => b,
        Err(e) => {
            ctx.record("the brain attaches", started, &Err(e));
            return false;
        }
    };
    let attached = gw
        .wait_for(Duration::from_secs(20), "brain_state true", |g| {
            (g.brain_attached == Some(true)).then(|| "brain_state true".to_owned())
        })
        .await;
    ctx.record("the brain attaches", started, &attached);
    if attached.is_err() {
        return false;
    }

    // 2. Its snapshot's HP is the prompt's.
    let started = Instant::now();
    let hp = gw
        .wait_for(
            Duration::from_secs(10),
            "a snapshot with the prompt's HP",
            |g| {
                let hp = g.character.get("hp")?.as_i64()?;
                (Some(hp) == brain::last_prompt_hp(&g.transcript()))
                    .then(|| format!("hp {hp}, as the last prompt"))
            },
        )
        .await;
    ctx.record("the snapshot's hp is the prompt's", started, &hp);

    // 3. It re-syncs (`st`, `exp`, `i`, `look` at an idle prompt).
    let started = Instant::now();
    let synced = gw
        .wait_for(Duration::from_secs(30), "the notice \"re-synced\"", |g| {
            g.notices.iter().any(|n| n == "re-synced").then(|| {
                let c = &g.character;
                let s = |k: &str| c.get(k).map(|v| v.to_string()).unwrap_or("?".into());
                format!(
                    "level {}, exp {}, room {}, synced_at {}",
                    s("level"),
                    s("exp"),
                    s("room_title"),
                    s("synced_at")
                )
            })
        })
        .await;
    ctx.record("the brain re-syncs", started, &synced);

    // 4. A second brain is refused.
    let started = Instant::now();
    let second = match brain::Brain::start(&wayfarer, &dir, "brain-2", &gw.addr, &world).await {
        Ok(b) => b.exited().await,
        Err(e) => Err(e),
    }
    .and_then(|(status, log)| {
        if status.success() {
            Err(format!("exited with {status}"))
        } else if !log.contains("a brain is already attached") {
            Err(format!(
                "exited with {status} but not for the reason expected:\n{log}"
            ))
        } else {
            Ok(format!("{status}: a brain is already attached"))
        }
    });
    ctx.record("a second brain is refused", started, &second);

    // 5. Stopping it leaves the character in the realm.
    let started = Instant::now();
    let stopped = running.stop().await.and_then(|(status, _)| {
        status
            .success()
            .then(|| format!("exited with {status}"))
            .ok_or_else(|| format!("exited with {status}"))
    });
    ctx.record("stop the brain", started, &stopped);
    let started = Instant::now();
    let detached = gw
        .wait_for(Duration::from_secs(10), "brain_state false", |g| {
            (g.brain_attached == Some(false)).then_some(())
        })
        .await
        .and_then(|()| {
            (gw.state == SessionState::Realm)
                .then(|| "brain_state false; the session is still in the realm".to_owned())
                .ok_or_else(|| format!("the session is {:?}", gw.state))
        });
    ctx.record("the character stays in the realm", started, &detached);
    let started = Instant::now();
    let status = status_screen(gw).await;
    ctx.record("`st` shows the status screen", started, &status);

    // 6. Started again, it attaches and re-syncs again.
    let started = Instant::now();
    let restarted = match brain::Brain::start(&wayfarer, &dir, "brain-3", &gw.addr, &world).await {
        Ok(b) => {
            let synced = gw
                .wait_for(Duration::from_secs(30), "a second \"re-synced\"", |g| {
                    (g.brain_attached == Some(true)
                        && g.notices.iter().filter(|n| *n == "re-synced").count() == 2)
                        .then(|| "brain_state true, re-synced".to_owned())
                })
                .await;
            let stopped = b.stop().await;
            synced.and_then(|s| match stopped {
                Ok((status, _)) if status.success() => Ok(s),
                Ok((status, _)) => Err(format!("stopping it: exited with {status}")),
                Err(e) => Err(format!("stopping it: {e}")),
            })
        }
        Err(e) => Err(e),
    };
    ctx.record("a restarted brain re-syncs", started, &restarted);
    true
}

/// The room the walk scenario walks to: the Newhaven Narrow Road (1/2146), two or three steps
/// from where characters start, by rooms nothing spawns in. Not through a door: the only door
/// near Newhaven is the Arena's into the Dungeon (`kb/pitfalls.md` 36), and the Arena's
/// monsters would start a fight, which stops a walk. The in-process tests cover doors.
const WALK_TO: (i64, i64) = (1, 2146);
/// Where it walks instead when the character is already there: the Weapons Shop.
const WALK_TO_ELSE: (i64, i64) = (1, 2141);
/// How long one walk may take.
const WALK_TIMEOUT: Duration = Duration::from_secs(90);

/// The believed room in the character snapshot, and its title.
fn believed(c: &serde_json::Value) -> Option<((i64, i64), String)> {
    let p = c.get("position")?;
    let key = (p.get("map")?.as_i64()?, p.get("room")?.as_i64()?);
    Some((key, p.get("title")?.as_str()?.to_owned()))
}

/// `walk_to` `to`, and wait for "Arrived: ..." with the room title to match.
async fn walk(gw: &mut Gateway, to: (i64, i64)) -> Result<String, String> {
    let (notices, warnings) = (gw.notices.len(), gw.warnings.len());
    gw.walk_to(to.0, to.1).await?;
    let said = gw
        .wait_for(WALK_TIMEOUT, "\"Arrived: ...\"", |g| {
            if let Some(w) = g.warnings[warnings..].first() {
                return Some(Err(w.clone()));
            }
            let new = &g.notices[notices..];
            new.iter()
                .find_map(|n| n.strip_prefix("Arrived: "))
                .map(|name| Ok((name.to_owned(), new.join("; "))))
        })
        .await??;
    let (name, narration) = said;
    // The snapshot's patch and the map view follow the notice.
    let checked = gw
        .wait_for(Duration::from_secs(5), "the room title and position", |g| {
            let title = g.character.get("room_title").and_then(|t| t.as_str());
            let at = believed(&g.character).map(|(k, _)| k);
            (title == Some(name.as_str()) && at == Some(to) && g.map_centers.contains(&to))
                .then_some(())
        })
        .await;
    checked.map_err(|e| {
        format!(
            "arrived at {name:?}, but: room title {:?}, position {:?}, map views {:?}: {e}",
            gw.character.get("room_title"),
            believed(&gw.character),
            gw.map_centers
        )
    })?;
    Ok(narration)
}

/// The world pack and a brain attached to `gw`, re-synced, its `look` placing the character:
/// the brain and the room it believes the character is in; `None` (recorded) when not.
async fn placed_brain(
    ctx: &mut Ctx,
    gw: &mut Gateway,
    label: &str,
) -> Option<(brain::Brain, (i64, i64))> {
    let dir = run_dir(&ctx.args.out, &mut ctx.runs, label);
    let started = Instant::now();
    let world = brain::world_pack(&ctx.args.wayfarer, &dir).await;
    ctx.record(
        "build the world pack",
        started,
        &world
            .as_ref()
            .map(|p| p.display().to_string())
            .map_err(Clone::clone),
    );
    let Ok(world) = world else { return None };

    // The brain attaches and re-syncs: the `look` places the character.
    let started = Instant::now();
    let wayfarer = ctx.args.wayfarer.clone();
    let running = match brain::Brain::start(&wayfarer, &dir, "brain", &gw.addr, &world).await {
        Ok(b) => b,
        Err(e) => {
            ctx.record("the brain attaches and re-syncs", started, &Err(e));
            return None;
        }
    };
    let placed = gw
        .wait_for(Duration::from_secs(40), "a re-sync and a position", |g| {
            if !g.notices.iter().any(|n| n == "re-synced") {
                return None;
            }
            let p = &g.character["position"];
            believed(&g.character).map(|(k, title)| {
                (
                    k,
                    format!("{title} ({}/{}, {} candidates)", k.0, k.1, p["candidates"]),
                )
            })
        })
        .await;
    ctx.record(
        "the brain attaches and re-syncs",
        started,
        &placed
            .as_ref()
            .map(|(_, s)| s.clone())
            .map_err(Clone::clone),
    );
    match placed {
        Ok((start, _)) => Some((running, start)),
        Err(_) => {
            stop_brain(ctx, running).await;
            None
        }
    }
}

/// Stop the brain; the character stays in the realm. Whether it exited cleanly.
async fn stop_brain(ctx: &mut Ctx, running: brain::Brain) -> bool {
    let started = Instant::now();
    let stopped = running.stop().await.and_then(|(status, _)| {
        status
            .success()
            .then(|| format!("exited with {status}"))
            .ok_or_else(|| format!("exited with {status}"))
    });
    ctx.record("stop the brain", started, &stopped);
    stopped.is_ok()
}

/// The walk scenario, with the gateway in the realm: a brain, a walk there and back, the
/// brain stopped. Whether it got as far as stopping the brain.
async fn walk_scenario(ctx: &mut Ctx, gw: &mut Gateway) -> bool {
    // 1. The brain attaches and re-syncs: the `look` places the character.
    let Some((running, start)) = placed_brain(ctx, gw, "walk").await else {
        return false;
    };
    // 2. There, confirming each room; 3. and back.
    let there = if start == WALK_TO {
        WALK_TO_ELSE
    } else {
        WALK_TO
    };
    let started = Instant::now();
    let out = walk(gw, there).await;
    ctx.record(&format!("walk to {}/{}", there.0, there.1), started, &out);
    let mut ok = false;
    if out.is_ok() {
        let started = Instant::now();
        let back = walk(gw, start).await;
        ctx.record(
            &format!("walk back to {}/{}", start.0, start.1),
            started,
            &back,
        );
        ok = back.is_ok();
    }

    // 4. Stop the brain; the character stays in the realm.
    stop_brain(ctx, running).await && ok
}

/// The fight scenario's room: the Newhaven Arena (1/2150), for level 3 or below (the pool's
/// characters are level 1). Its monsters are giant rats and the like.
const ARENA: (i64, i64) = (1, 2150);
/// The bot must be fighting this soon after Start (its re-sync comes first).
const ENGAGE_TIMEOUT: Duration = Duration::from_secs(60);
/// A kill (or the bot stopping itself) this soon after the fight starts.
const FIGHT_TIMEOUT: Duration = Duration::from_secs(180);
/// How long the scenario rests by hand at most.
const SETTLE_TIMEOUT: Duration = Duration::from_secs(180);
/// How long it waits for a fight to end by itself before walking out anyway.
const FIGHT_END: Duration = Duration::from_secs(60);

fn bot_field<'a>(c: &'a serde_json::Value, field: &str) -> Option<&'a str> {
    c.get("bot")?.get(field)?.as_str()
}

fn hp(c: &serde_json::Value) -> Option<(i64, i64)> {
    Some((c.get("hp")?.as_i64()?, c.get("hp_max")?.as_i64()?))
}

/// The fight scenario, with the gateway in the realm: a brain; a walk to the Arena; Start;
/// `*Combat Engaged*` within a minute and then a kill or the bot stopping itself, HP tracked;
/// Stop; the fight left to end; a walk out to the Narrow Road; rest by hand if HP is low; the
/// brain stopped. Whether all of it went.
async fn fight_scenario(ctx: &mut Ctx, gw: &mut Gateway) -> bool {
    use wayfarer_proto::BotAction;

    // 1. The brain attaches and re-syncs.
    let Some((running, start)) = placed_brain(ctx, gw, "fight").await else {
        return false;
    };
    let ok = fight_in_the_arena(ctx, gw, start).await;

    // Whatever happened: the bot stopped, out of the Arena, HP back before logging off.
    let started = Instant::now();
    let stopped = match gw.bot(BotAction::Stop).await {
        Ok(()) => {
            gw.wait_for(Duration::from_secs(10), "the bot stopped", |g| {
                (bot_field(&g.character, "state") == Some("stopped")).then(|| {
                    bot_field(&g.character, "narration")
                        .unwrap_or("")
                        .to_owned()
                })
            })
            .await
        }
        Err(e) => Err(e),
    };
    ctx.record("stop the bot", started, &stopped);
    let out = walk_out(ctx, gw).await;
    let rested = rest_by_hand(ctx, gw).await;
    stop_brain(ctx, running).await && ok && stopped.is_ok() && out && rested
}

/// Steps 2 to 4 of the fight scenario: the walk there, Start, the fight.
async fn fight_in_the_arena(ctx: &mut Ctx, gw: &mut Gateway, start: (i64, i64)) -> bool {
    use wayfarer_proto::BotAction;

    let level = gw
        .character
        .get("level")
        .and_then(serde_json::Value::as_i64);
    if level.is_none_or(|l| l > 3) {
        ctx.record(
            "the character may use the Arena",
            Instant::now(),
            &Err(format!(
                "level {level:?}: the Arena is for level 3 or below"
            )),
        );
        return false;
    }

    // 2. To the Arena: by the Narrow Road, so the last step is the stairs down.
    for to in [WALK_TO, ARENA] {
        if start == ARENA || (to == WALK_TO && start == WALK_TO) {
            continue;
        }
        let started = Instant::now();
        let out = walk(gw, to).await;
        ctx.record(&format!("walk to {}/{}", to.0, to.1), started, &out);
        if out.is_err() {
            return false;
        }
    }

    // 3. Start: re-sync, then the bot attacks what is there or comes.
    let started = Instant::now();
    let seq = gw.last_seq();
    let warnings = gw.warnings.len();
    if let Err(e) = gw.bot(BotAction::Start).await {
        ctx.record("start: *Combat Engaged*", started, &Err(e));
        return false;
    }
    let engaged = gw
        .wait_for(ENGAGE_TIMEOUT, "*Combat Engaged*", |g| {
            if let Some(w) = g.warnings[warnings..].first() {
                return Some(Err(w.clone()));
            }
            screen::plain(&g.text_after(seq))
                .contains("*Combat Engaged*")
                .then(|| {
                    Ok(bot_field(&g.character, "narration")
                        .unwrap_or("")
                        .to_owned())
                })
        })
        .await
        .and_then(|r| r);
    ctx.record("start: *Combat Engaged*", started, &engaged);
    if engaged.is_err() {
        return false;
    }

    // 4. A kill, or the bot stopping itself; HP tracked from the snapshot.
    let started = Instant::now();
    let mut seen_hp: Vec<i64> = Vec::new();
    let fought = gw
        .wait_for(FIGHT_TIMEOUT, "a kill, or the bot stopping", |g| {
            if let Some((now, _)) = hp(&g.character)
                && seen_hp.last() != Some(&now)
            {
                seen_hp.push(now);
            }
            let text = screen::plain(&g.text_after(seq));
            // A line can follow a prompt on the screen: `[HP=33]:You gain 9 experience.`.
            if let Some(at) = text.find("You gain ") {
                let line = text[at..].lines().next().unwrap_or_default();
                return Some(format!("kill: {}", line.trim()));
            }
            (bot_field(&g.character, "state") == Some("stopped")).then(|| {
                format!(
                    "the bot stopped: {}",
                    bot_field(&g.character, "narration").unwrap_or("")
                )
            })
        })
        .await
        .map(|what| {
            let max = hp(&gw.character).map_or(0, |(_, m)| m);
            format!(
                "{what}; HP seen {} of {max}",
                seen_hp
                    .iter()
                    .map(i64::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        });
    ctx.record("a kill or a stop, HP tracked", started, &fought);
    fought.is_ok()
}

/// Let a fight end by itself (stopping the bot does not stop auto-combat), then walk to the
/// Narrow Road, trying again if a fight stops the walk.
async fn walk_out(ctx: &mut Ctx, gw: &mut Gateway) -> bool {
    let started = Instant::now();
    let mut out = Err("not tried".to_owned());
    for _ in 0..3 {
        // Not engaged (a monster may still be attacking: the walk leaves it behind). A fight
        // can lapse without `*Combat Off*`, so after a while the walk is tried anyway.
        let _ = gw
            .wait_for(FIGHT_END, "the fight over", |g| {
                let engaged = g
                    .character
                    .get("in_combat")
                    .and_then(serde_json::Value::as_bool);
                (engaged != Some(true)).then_some(())
            })
            .await;
        let room = believed(&gw.character).map(|(k, _)| k);
        if room == Some(WALK_TO) {
            out = Ok("already there".to_owned());
            break;
        }
        out = walk(gw, WALK_TO).await;
        if out.is_ok() {
            break;
        }
    }
    ctx.record(
        &format!("walk out to {}/{}", WALK_TO.0, WALK_TO.1),
        started,
        &out,
    );
    out.is_ok()
}

/// Below half HP: `rest` by hand until 80% (resting is broken by an attack: sent again).
async fn rest_by_hand(ctx: &mut Ctx, gw: &mut Gateway) -> bool {
    let Some((now, max)) = hp(&gw.character) else {
        return true;
    };
    if now * 2 >= max {
        return true;
    }
    let started = Instant::now();
    let deadline = Instant::now() + SETTLE_TIMEOUT;
    let mut rested = Err(format!("HP {now}/{max}"));
    while Instant::now() < deadline {
        if let Err(e) = gw.send_line("rest").await {
            rested = Err(e);
            break;
        }
        let r = gw
            .wait_for(Duration::from_secs(30), "HP back to 80%", |g| {
                let (hp, max) = hp(&g.character)?;
                (hp * 10 >= max * 8).then(|| format!("rested from {now} to {hp} of {max}"))
            })
            .await;
        if r.is_ok() {
            rested = r;
            break;
        }
    }
    ctx.record("rest by hand", started, &rested);
    rested.is_ok()
}

/// Whether something answers at `addr` within a few seconds.
fn answers(addr: &str) -> bool {
    let Ok(addrs) = addr.to_socket_addrs() else {
        return false;
    };
    addrs
        .into_iter()
        .any(|a| std::net::TcpStream::connect_timeout(&a, Duration::from_secs(3)).is_ok())
}

fn main() -> ExitCode {
    let args = args();
    if !answers(&args.addr) {
        eprintln!(
            "live check: nothing answers at {}: start silvermere first (docs/dev-rig.md):\n  {START_SILVERMERE}",
            args.addr
        );
        return ExitCode::from(2);
    }
    let Some((host, port)) = args
        .addr
        .rsplit_once(':')
        .and_then(|(h, p)| Some((h.to_owned(), p.parse().ok()?)))
    else {
        eprintln!("live check: {:?} is not HOST:PORT", args.addr);
        return ExitCode::from(2);
    };
    let pool = match Pool::load(&args.pool).and_then(|p| p.only(args.only.as_deref())) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("live check: {e}");
            return ExitCode::from(2);
        }
    };
    let out = args.out.clone();
    let mut ctx = Ctx {
        args,
        host,
        port,
        runs: 0,
        steps: Vec::new(),
    };
    let started = Instant::now();
    let result = tokio::runtime::Runtime::new()
        .expect("a tokio runtime")
        .block_on(run(&mut ctx, &pool));
    let passed = result.is_ok() && ctx.steps.iter().all(|s| s.ok);
    print_summary(&ctx, &result, started.elapsed(), passed, &out);
    if passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn print_summary(
    ctx: &Ctx,
    result: &Result<String, String>,
    total: Duration,
    passed: bool,
    out: &Path,
) {
    println!();
    println!("### Live check: {}", if passed { "PASS" } else { "FAIL" });
    println!();
    println!(
        "Server `{}`, character {}.",
        ctx.args.addr,
        match result {
            Ok(who) => who.clone(),
            Err(who) => who.lines().next().unwrap_or_default().to_owned(),
        }
    );
    println!();
    println!("| Step | Result | Time |");
    println!("|---|---|---|");
    for s in &ctx.steps {
        let first = s
            .note
            .lines()
            .next()
            .unwrap_or_default()
            .replace('|', "\\|");
        println!(
            "| {} | {} {} | {:.2} s |",
            s.name,
            if s.ok { "pass" } else { "**FAIL**" },
            if first.is_empty() {
                String::new()
            } else {
                format!("({first})")
            },
            s.secs
        );
    }
    println!();
    println!(
        "Total {:.1} s. Logs: `{}`.",
        total.as_secs_f64(),
        out.display()
    );
    for s in ctx.steps.iter().filter(|s| !s.ok && s.note.contains('\n')) {
        println!("\n{}:\n```\n{}\n```", s.name, s.note);
    }
    if let Err(e) = result
        && e.contains('\n')
    {
        println!("\n```\n{e}\n```");
    }
}
