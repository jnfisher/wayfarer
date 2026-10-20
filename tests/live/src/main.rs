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
//! `--scenario fight` walks it to the Newhaven Arena and starts the bot there;
//! `--scenario under-attack` then stops the gateway while a monster is attacking (the brain
//! flees first); `--scenario restart` restarts the brain while the bot fights there, and checks
//! it resumes; `--scenario flee` raises the flee line so the first blow sends the character
//! out of the Arena to rest and back; `--scenario typing` types beside the bot while it fights
//! there (`st` goes without a pause, `u` pauses it and moves).

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
    /// C-027: as "fight", but the gateway is stopped while a monster attacks, the brain still
    /// attached; then the next logon is checked and the character walks out of the Arena by
    /// hand. Until the brain flees before leaving (C-028), the monster interrupts every `x`
    /// and the character is dropped in the realm (`docs/kb/recovery.md`).
    UnderAttack,
    /// M2's generated loops: the Newhaven loop is listed, and `walk_loop` starts on it and is
    /// stopped before the Arena (its monsters would start a fight).
    Loop,
    /// C-029: as "fight", but `wayfarer brain` is restarted while the bot runs; the new brain
    /// resumes it running in the Arena, and `stats_request` shows the kills so far.
    Restart,
    /// C-028: as "fight", from full HP with the flee line at 100% (`--flee-below 100`): the
    /// first blow sends the character up to the Narrow Road, where it rests and from where it
    /// walks back.
    Flee,
    /// C-031: as "fight", then lines typed beside the bot while it fights: `st` shows the
    /// status screen with the bot still running, `u` pauses it and goes up the stairs.
    Typing,
}

fn usage() -> ! {
    eprintln!(
        "usage: wayfarer-live --wayfarer BIN --out DIR [--addr HOST:PORT] [--pool FILE] \
         [--credentials FILE] [--char ACCOUNT] [--scenario m0|brain|walk|fight|loop|under-attack|restart|flee|typing]\n(run it through \
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
                    "loop" => Scenario::Loop,
                    "under-attack" => Scenario::UnderAttack,
                    "restart" => Scenario::Restart,
                    "flee" => Scenario::Flee,
                    "typing" => Scenario::Typing,
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
    if matches!(ctx.args.scenario, Scenario::Fight | Scenario::UnderAttack) {
        let under_attack = ctx.args.scenario == Scenario::UnderAttack;
        let ok = match fight_scenario(ctx, &mut gw, under_attack).await {
            Fought::UnderAttack(running) => {
                let left = logoff_under_attack(ctx, gw, &running).await;
                let brain = stop_brain(ctx, *running).await;
                let again = log_on_again(ctx, fixture, true).await;
                left && brain && again
            }
            Fought::Out(fought) => {
                let logged_off = stop(ctx, "stop the gateway: log off", gw).await;
                fought && logged_off
            }
        };
        return if ok { Ok(who) } else { Err(who) };
    }
    if ctx.args.scenario == Scenario::Flee {
        let fled = flee_scenario(ctx, &mut gw).await;
        let logged_off = stop(ctx, "stop the gateway: log off", gw).await;
        let again = logged_off && log_on_again(ctx, fixture, true).await;
        return if fled && again { Ok(who) } else { Err(who) };
    }
    if ctx.args.scenario == Scenario::Typing {
        let typed = typing_scenario(ctx, &mut gw).await;
        let logged_off = stop(ctx, "stop the gateway: log off", gw).await;
        return if typed && logged_off {
            Ok(who)
        } else {
            Err(who)
        };
    }
    if ctx.args.scenario == Scenario::Restart {
        let restarted = restart_scenario(ctx, &mut gw).await;
        let logged_off = stop(ctx, "stop the gateway: log off", gw).await;
        return if restarted && logged_off {
            Ok(who)
        } else {
            Err(who)
        };
    }
    if ctx.args.scenario == Scenario::Loop {
        let looped = loop_scenario(ctx, &mut gw).await;
        let logged_off = stop(ctx, "stop the gateway: log off", gw).await;
        return if looped && logged_off {
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
    log_on_again(ctx, fixture, false).await;
    Ok(who)
}

/// Log on again and check nothing says the character was punished; out of the Arena first
/// (`u`, the stairs up to the Narrow Road) when `leave_arena` and it is there; then log off.
/// Whether every step passed.
async fn log_on_again(ctx: &mut Ctx, fixture: &Fixture, leave_arena: bool) -> bool {
    let started = Instant::now();
    let password = Passwords::load(&ctx.args.credentials)
        .ok()
        .and_then(|p| p.get(&fixture.account).map(str::to_owned));
    let Some(password) = password else {
        let e = "the password went missing from .credentials.toml".to_owned();
        ctx.record("log on again: not punished", started, &Err(e));
        return false;
    };
    let mut gw = match ctx.start("relogon", fixture, &password, false).await {
        Ok(gw) => gw,
        Err(e) => {
            ctx.record("log on again: not punished", started, &Err(e));
            return false;
        }
    };
    // The state can come before the output that shows the realm: wait for its prompt too.
    let again = gw
        .wait_for(LOGON_TIMEOUT, "the realm prompt", |g| {
            (g.state == SessionState::Realm && g.transcript().contains("[HP=")).then_some(())
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
    let mut out = true;
    // Punished or not, a character left in the Arena is walked out before it logs off.
    let in_arena = gw.transcript().contains("Arena");
    if leave_arena && in_arena {
        let started = Instant::now();
        let left = leave_the_arena(&mut gw).await;
        out = left.is_ok();
        ctx.record("out of the Arena by hand", started, &left);
    }
    let logged_off = stop(ctx, "stop the gateway: log off", gw).await;
    again.is_ok() && out && logged_off
}

/// `u` up the stairs to the Narrow Road, again if a move is refused (a monster's attack).
async fn leave_the_arena(gw: &mut Gateway) -> Result<String, String> {
    let mut last = Err("not tried".to_owned());
    for _ in 0..3 {
        let seq = gw.last_seq();
        gw.send_line("u").await?;
        last = gw
            .wait_for(Duration::from_secs(15), "the Narrow Road", |g| {
                screen::plain(&g.text_after(seq))
                    .contains("Narrow Road")
                    .then(|| "up the stairs to the Narrow Road".to_owned())
            })
            .await;
        if last.is_ok() {
            break;
        }
    }
    last
}

/// The bot's narration in a brain's log (`Notice("...")`, `Warning("...")`), from the first
/// line containing `from`.
fn narration_from(log: &[String], from: &str) -> Vec<String> {
    let said: Vec<String> = log
        .iter()
        .filter_map(|l| {
            let at = l
                .find("Notice(\"")
                .map(|i| i + 8)
                .or_else(|| l.find("Warning(\"").map(|i| i + 9))?;
            let rest = &l[at..];
            Some(rest[..rest.rfind("\")").unwrap_or(rest.len())].to_owned())
        })
        .collect();
    let first = said
        .iter()
        .position(|n| n.contains(from))
        .unwrap_or(said.len());
    said[first..].to_vec()
}

/// Stop the gateway (SIGTERM) with the brain attached and a monster attacking: the brain is
/// asked first and flees (an attack cancels the exit), then answers; the log-off keeps sending
/// `x` until the character is out. Whether it logged off cleanly after a flee.
async fn logoff_under_attack(ctx: &mut Ctx, gw: Gateway, brain: &brain::Brain) -> bool {
    let started = Instant::now();
    let stopped = gw.stop().await;
    let fled = narration_from(&brain.log(), "Logging off under attack");
    let flee = match fled.first() {
        None => Err("the brain did not flee before answering logging_off".to_owned()),
        Some(_)
            if fled
                .iter()
                .any(|n| n.starts_with("Safe in") || n.starts_with("Fled ")) =>
        {
            Ok(fled.join("; "))
        }
        Some(_) => Err(format!("the flee did not get away: {}", fled.join("; "))),
    };
    ctx.record(
        "logging_off under attack: the brain flees first",
        started,
        &flee,
    );
    let lines = |s: &str| stopped.log.iter().filter(|l| l.contains(s)).count();
    let retries = lines("log-off interrupted");
    let result = if !stopped.clean {
        Err(format!(
            "{} (`x` sent again {retries} times)",
            stopped.detail
        ))
    } else if lines("the brain is ready") == 0 {
        Err("the brain did not answer logging_off".to_owned())
    } else if retries == 0 {
        Ok(format!(
            "{}; the brain answered ready_to_leave; nothing attacked during the meditation, \
             so no retry was needed",
            stopped.detail
        ))
    } else {
        Ok(format!(
            "{}; `x` sent again {retries} times; the brain answered ready_to_leave",
            stopped.detail
        ))
    };
    ctx.record("stop the gateway under attack: log off", started, &result);
    flee.is_ok() && result.is_ok()
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
    extra: &[String],
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
    let running =
        match brain::Brain::start_with(&wayfarer, &dir, "brain", &gw.addr, &world, extra).await {
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
    let Some((running, start)) = placed_brain(ctx, gw, "walk", &[]).await else {
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

/// Newhaven, Village Entrance: the loop scenario's anchor and where it starts.
const VILLAGE_ENTRANCE: (i64, i64) = (1, 2140);

/// The `loops` around the Village Entrance: the generated Newhaven loop's id, and what was
/// listed; an error when the list is not what the stock world gives.
async fn newhaven_loops(gw: &mut Gateway) -> Result<(String, String), String> {
    gw.loops_request(VILLAGE_ENTRANCE, 8).await?;
    let warnings = gw.warnings.len();
    gw.wait_for(Duration::from_secs(10), "the loops", |g| {
        if let Some(w) = g.warnings[warnings..].first() {
            return Some(Err(w.clone()));
        }
        let Some(wayfarer_proto::ToClient::Loops {
            generated, megamud, ..
        }) = &g.loops
        else {
            return None;
        };
        let check = || {
            let l = generated
                .iter()
                .find(|l| l.name == "Newhaven")
                .ok_or_else(|| format!("no loop \"Newhaven\" in {generated:?}"))?;
            let stops: Vec<(i64, i64)> = l.stops.iter().map(|s| (s.map, s.room)).collect();
            if stops != [ARENA, (1, 2152), (1, 2156)] || !l.stops[0].arena || l.steps >= 40 {
                return Err(format!("the Newhaven loop is {l:?}"));
            }
            Ok((
                l.id.clone(),
                format!(
                    "{}: {} ({} steps), {} MegaMud",
                    l.name,
                    l.stops
                        .iter()
                        .map(|s| s.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                    l.steps,
                    megamud.len()
                ),
            ))
        };
        Some(check())
    })
    .await?
}

/// The loop scenario, with the gateway in the realm: a brain; to the Village Entrance; the
/// loops listed; `walk_loop` on the Newhaven loop, stopped as its first walk starts, so the
/// character stops a step or two short of the Arena and never fights; back to the Village
/// Entrance; then `run_loop` on it for [`LOOP_RUN`], the Small Cavern's cave bears unticked
/// (the run: kills at the Arena, the stops narrated, the Dungeon Entrance's respawn minute
/// honoured, no death); Stop, out of the Arena, rest by hand if low; the brain stopped.
/// Whether all of it went.
async fn loop_scenario(ctx: &mut Ctx, gw: &mut Gateway) -> bool {
    // 1. The brain attaches and re-syncs.
    let Some((running, start)) = placed_brain(ctx, gw, "loop", &[]).await else {
        return false;
    };
    let mut ok = true;
    // 2. To the Village Entrance, through rooms nothing spawns in.
    if start != VILLAGE_ENTRANCE {
        let started = Instant::now();
        let there = walk(gw, VILLAGE_ENTRANCE).await;
        ctx.record("walk to the Village Entrance", started, &there);
        ok = there.is_ok();
    }
    // 3. The loops around it.
    let mut id = None;
    if ok {
        let started = Instant::now();
        let listed = newhaven_loops(gw).await;
        ctx.record(
            "loops_request: the Newhaven loop",
            started,
            &listed
                .as_ref()
                .map(|(_, s)| s.clone())
                .map_err(Clone::clone),
        );
        id = listed.ok().map(|(id, _)| id);
    }
    // 4. Walk it, and stop as the walk to its first stop (the Arena) starts.
    if let Some(id) = id {
        let started = Instant::now();
        let out = walk_loop_and_stop(gw, &id).await;
        ctx.record("walk_loop, stopped before the Arena", started, &out);
        ok = out.is_ok();
        if believed(&gw.character).map(|(k, _)| k) == Some(ARENA) {
            // Too late: out of the Arena before anything else.
            walk_out(ctx, gw).await;
            ok = false;
        }
        // 5. Back to the start.
        let started = Instant::now();
        let back = walk(gw, VILLAGE_ENTRANCE).await;
        ctx.record("walk back to the Village Entrance", started, &back);
        ok &= back.is_ok();
        // 6. Run it.
        if ok {
            ok = run_the_loop(ctx, gw, &id).await;
        }
    } else {
        ok = false;
    }
    // 7. Stop the brain; the character stays in the realm.
    stop_brain(ctx, running).await && ok
}

/// How long the loop scenario runs the loop.
const LOOP_RUN: Duration = Duration::from_secs(300);
/// The Newhaven loop's Lair of cave bears, north of the Dungeon Entrance: too much for a
/// level-1 character, so the run passes it by.
const SMALL_CAVERN: (i64, i64) = (1, 2156);
/// The Dungeon Entrance's respawn delay (`regendelay` 1 in the stock world), less a little for
/// the time a notice takes to come.
const DUNGEON_ENTRANCE_REGEN: Duration = Duration::from_secs(58);

/// `run_loop` on the Newhaven loop for [`LOOP_RUN`], then Stop, out of the Arena, and rest by
/// hand if low. Whether the run held: no death, the bot never stopping itself, at least one
/// kill at the Arena, every stop narrated, the Small Cavern never walked to, and no walk back
/// to the Dungeon Entrance within its respawn minute of clearing it.
async fn run_the_loop(ctx: &mut Ctx, gw: &mut Gateway, id: &str) -> bool {
    use wayfarer_proto::BotAction;

    let started = Instant::now();
    let from = gw.notices.len();
    let warned = gw.warnings.len();
    let ran = run_for(gw, id, from, warned).await;
    ctx.record(
        &format!("run_loop for {} minutes", LOOP_RUN.as_secs() / 60),
        started,
        &ran,
    );

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
    ran.is_ok() && stopped.is_ok() && out && rested
}

/// `run_loop`, the bot running, then followed until [`LOOP_RUN`] is up or it stops (a death,
/// or stopping itself); the verdict.
async fn run_for(gw: &mut Gateway, id: &str, from: usize, warned: usize) -> Result<String, String> {
    gw.run_loop(id, &[SMALL_CAVERN]).await?;
    gw.wait_for(Duration::from_secs(10), "the bot running", |g| {
        (bot_field(&g.character, "state") == Some("running")).then_some(())
    })
    .await?;
    let end = Instant::now() + LOOP_RUN;
    let _ = gw
        .wait_for(LOOP_RUN, "the run's end", |g| {
            (Instant::now() >= end || bot_field(&g.character, "state") == Some("stopped"))
                .then_some(())
        })
        .await;
    judge_the_run(gw, from, warned)
}

/// The run's verdict from the notices and warnings since `from` and `warned`: a summary, or
/// what went wrong.
fn judge_the_run(gw: &Gateway, from: usize, warned: usize) -> Result<String, String> {
    let said: Vec<(&str, Instant)> = gw.notices[from..]
        .iter()
        .map(String::as_str)
        .zip(gw.notice_at[from..].iter().copied())
        .collect();
    let warnings = &gw.warnings[warned..];
    let state = bot_field(&gw.character, "state").unwrap_or("?");
    if state != "running" {
        let why = bot_field(&gw.character, "narration").unwrap_or("");
        return Err(format!("the bot is {state}: {why}; warnings {warnings:?}"));
    }
    if let Some(w) = warnings.iter().find(|w| w.contains("killed")) {
        return Err(w.clone());
    }
    let count = |prefix: &str| said.iter().filter(|(s, _)| s.starts_with(prefix)).count();
    let arena_kills = said
        .iter()
        .filter(|(s, _)| s.starts_with("Stop 1/3 cleared: ") && !s.ends_with("nothing there"))
        .count();
    if arena_kills == 0 {
        return Err(format!("no kill at the Arena; said {:?}", tail_of(&said)));
    }
    if said
        .iter()
        .any(|(s, _)| s.starts_with("Loop Newhaven: to stop 3/3"))
    {
        return Err("walked to the unticked Small Cavern".to_owned());
    }
    for stop in ["1/3", "2/3"] {
        if count(&format!("Stop {stop} cleared")) == 0 {
            return Err(format!(
                "stop {stop} never cleared; said {:?}",
                tail_of(&said)
            ));
        }
    }
    // Each walk to the Dungeon Entrance after the first: its minute since it was cleared.
    let mut cleared: Option<Instant> = None;
    let mut revisits = Vec::new();
    for &(s, at) in &said {
        if s.starts_with("Stop 2/3 cleared") {
            cleared = Some(at);
        } else if s.starts_with("Loop Newhaven: to stop 2/3")
            && let Some(c) = cleared
        {
            let after = at.duration_since(c);
            if after < DUNGEON_ENTRANCE_REGEN {
                return Err(format!(
                    "back to the Dungeon Entrance {} s after clearing it",
                    after.as_secs()
                ));
            }
            revisits.push(after.as_secs());
        }
    }
    let run = &gw.character["run"];
    Ok(format!(
        "{} kills ({} exp/h), {} flees, {} rests; Arena cleared {} times with kills, {} legs,          {} waits for respawns, {} passings-by of the Small Cavern; back to the Dungeon          Entrance after {:?} s",
        run["kills"],
        run["exp_per_hour"],
        run["flees"],
        run["rests"],
        arena_kills,
        count("Loop Newhaven: to stop"),
        count("Waiting for respawns"),
        count("Skipping Small Cavern"),
        revisits
    ))
}

/// The last few notices, for an error.
fn tail_of<'a>(said: &[(&'a str, Instant)]) -> Vec<&'a str> {
    said.iter().rev().take(8).rev().map(|(s, _)| *s).collect()
}

/// `walk_loop`, then `walk_stop` as soon as the brain narrates the loop's first walk; the
/// stop narrated and the walk ended short of the Arena.
async fn walk_loop_and_stop(gw: &mut Gateway, id: &str) -> Result<String, String> {
    let (notices, warnings) = (gw.notices.len(), gw.warnings.len());
    gw.walk_loop(id).await?;
    gw.wait_for(
        Duration::from_secs(10),
        "\"Walking to Newhaven, Arena\"",
        |g| {
            if let Some(w) = g.warnings[warnings..].first() {
                return Some(Err(w.clone()));
            }
            g.notices[notices..]
                .iter()
                .any(|n| n.starts_with("Walking to Newhaven, Arena"))
                .then_some(Ok(()))
        },
    )
    .await??;
    gw.walk_stop().await?;
    let said = gw
        .wait_for(WALK_TIMEOUT, "\"Loop Newhaven stopped\"", |g| {
            let new = &g.notices[notices..];
            (new.iter()
                .any(|n| n == "Loop Newhaven stopped: asked to stop")
                && new.iter().any(|n| n.starts_with("Stopped walking")))
            .then(|| new.join("; "))
        })
        .await?;
    // The snapshot's patch (the walk gone, the room it stopped in) follows the notices.
    let _ = gw
        .wait_for(Duration::from_secs(5), "the walk to end", |g| {
            g.character
                .get("walking")
                .is_none_or(serde_json::Value::is_null)
                .then_some(())
        })
        .await;
    let at = believed(&gw.character);
    match at {
        Some((k, title)) if k != ARENA => Ok(format!("{said}; stopped in {title}")),
        other => Err(format!("{said}; but the position is {other:?}")),
    }
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
/// How long, after the first kill, for the bot to be fighting again with HP to spare (it may
/// walk out to rest and come back first).
const UNDER_ATTACK_TIMEOUT: Duration = Duration::from_secs(600);

fn bot_field<'a>(c: &'a serde_json::Value, field: &str) -> Option<&'a str> {
    c.get("bot")?.get(field)?.as_str()
}

fn hp(c: &serde_json::Value) -> Option<(i64, i64)> {
    Some((c.get("hp")?.as_i64()?, c.get("hp_max")?.as_i64()?))
}

/// How the fight scenario ended.
enum Fought {
    /// Fighting in the Arena with HP to spare, the brain still attached: stop the gateway now.
    UnderAttack(Box<brain::Brain>),
    /// Out of the Arena, the brain stopped; whether all of it went.
    Out(bool),
}

/// The fight scenario, with the gateway in the realm: a brain; a walk to the Arena; Start;
/// `*Combat Engaged*` within a minute and then a kill or the bot stopping itself, HP tracked.
/// With `under_attack`, then the bot fighting again with HP to spare (it rests first if it
/// must), for the gateway to be stopped under attack. Otherwise, or if that never comes: Stop;
/// the fight left to end; a walk out to the Narrow Road; rest by hand if HP is low; the brain
/// stopped.
async fn fight_scenario(ctx: &mut Ctx, gw: &mut Gateway, under_attack: bool) -> Fought {
    use wayfarer_proto::BotAction;

    // 1. The brain attaches and re-syncs.
    let Some((running, start)) = placed_brain(ctx, gw, "fight", &[]).await else {
        return Fought::Out(false);
    };
    let ok = fight_in_the_arena(ctx, gw, start, true).await;

    // 5. A monster attacking, the bot fighting it, HP to spare.
    if ok && under_attack {
        let started = Instant::now();
        let ready = gw
            .wait_for(UNDER_ATTACK_TIMEOUT, "a fight with HP to spare", |g| {
                let c = &g.character;
                if bot_field(c, "state") == Some("stopped") {
                    let why = bot_field(c, "narration").unwrap_or("");
                    return Some(Err(format!("the bot stopped: {why}")));
                }
                let (now, max) = hp(c)?;
                let engaged = c.get("in_combat").and_then(serde_json::Value::as_bool);
                // Attacked just now, so the log-off meets an attack, not luck.
                let attack = g
                    .attacks
                    .last()
                    .filter(|a| a.at.elapsed() < RECENT_ATTACK)?;
                (engaged == Some(true) && now * 10 >= max * 6).then(|| {
                    Ok(format!(
                        "engaged at {now}/{max} HP; attacked by {}: \"{}\" ({} attacks seen)",
                        attack.who,
                        attack.line,
                        g.attacks.len()
                    ))
                })
            })
            .await
            .and_then(|r| r);
        ctx.record(
            "a monster attacking, HP 60% or more, attacked just now",
            started,
            &ready,
        );
        if ready.is_ok() {
            return Fought::UnderAttack(Box::new(running));
        }
    }

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
    let brain = stop_brain(ctx, running).await;
    // With `under_attack`, its step was not reached: the scenario fails whatever else went.
    Fought::Out(brain && ok && stopped.is_ok() && out && rested && !under_attack)
}

/// The restart scenario, with the gateway in the realm: a brain; to the Arena; Start and a
/// kill (or the bot stopping itself, which fails it); the brain restarted on the same store;
/// the new brain resumes the bot running in the Arena; `stats_request` shows the kills so far;
/// then as the fight scenario ends: Stop, out of the Arena, rest by hand, the brain stopped.
async fn restart_scenario(ctx: &mut Ctx, gw: &mut Gateway) -> bool {
    use wayfarer_proto::BotAction;

    let Some((running, start)) = placed_brain(ctx, gw, "restart", &[]).await else {
        return false;
    };
    let mut ok = fight_in_the_arena(ctx, gw, start, true).await;
    let kills = |c: &serde_json::Value| c["run"]["kills"].as_u64().unwrap_or(0);
    let kills_before = kills(&gw.character);
    let mut running = Some(running);
    if ok && bot_field(&gw.character, "state") != Some("running") {
        let why = bot_field(&gw.character, "narration")
            .unwrap_or("")
            .to_owned();
        ctx.record(
            "the bot runs before the restart",
            Instant::now(),
            &Err(format!("it is not running: {why}")),
        );
        ok = false;
    }

    // 5. Restart the brain while the bot runs: the new one resumes it in the Arena.
    if ok {
        let started = Instant::now();
        let notices = gw.notices.len();
        let brain = running.take().expect("the first brain");
        let restarted = match brain.restart("brain-2").await {
            Ok((how, again)) => {
                running = Some(again);
                ctx.record("stop the brain mid-run", started, &Ok(how));
                Ok(())
            }
            Err(e) => {
                ctx.record("stop the brain mid-run", started, &Err(e.clone()));
                Err(e)
            }
        };
        if restarted.is_ok() {
            let started = Instant::now();
            // The old brain's `brain_state false` first, so a stale `true` is not taken.
            let _ = gw
                .wait_for(Duration::from_secs(10), "brain_state false", |g| {
                    (g.brain_attached == Some(false)
                        || g.notices[notices..]
                            .iter()
                            .any(|n| n.starts_with("Resumed")))
                    .then_some(())
                })
                .await;
            let resumed = gw
                .wait_for(
                    Duration::from_secs(60),
                    "the bot resumed, its home the Arena",
                    |g| {
                        // Its home, not where it stands: Recover may have walked it out of the
                        // Arena to rest before the restart, and walks it back once rested.
                        let said = g.notices[notices..]
                            .iter()
                            .find(|n| n.starts_with("Resumed") && n.contains("Newhaven, Arena"))?;
                        let room = believed(&g.character).map_or_else(
                            || "an unknown room".to_owned(),
                            |(k, title)| format!("{title} ({}/{})", k.0, k.1),
                        );
                        (g.brain_attached == Some(true)
                            && bot_field(&g.character, "state") == Some("running")
                            && g.notices[notices..].iter().any(|n| n == "re-synced"))
                        .then(|| {
                            let doing = bot_field(&g.character, "narration").unwrap_or("");
                            format!("{said}; running in {room}: {doing}")
                        })
                    },
                )
                .await;
            ctx.record(
                "a restarted brain resumes the bot, home the Arena",
                started,
                &resumed,
            );
            ok = resumed.is_ok();
        } else {
            ok = false;
        }
    }

    // 6. `stats_request`: the run goes on, with the kills seen before the restart.
    if ok {
        let started = Instant::now();
        let stats = match gw.stats_request().await {
            Ok(()) => gw
                .wait_for(Duration::from_secs(10), "stats", |g| {
                    let Some(wayfarer_proto::ToClient::Stats { runs, samples, .. }) = &g.stats
                    else {
                        return None;
                    };
                    let r = runs.first()?;
                    Some(if r.ended_at.is_none() && r.kills >= kills_before {
                        Ok(format!(
                            "run since {}: {} kills, {} exp, {} exp/hr, {} samples, {} runs listed",
                            r.started_at,
                            r.kills,
                            r.exp_gained,
                            r.exp_per_hour,
                            samples.len(),
                            runs.len()
                        ))
                    } else {
                        Err(format!(
                            "the newest run: {r:?}; {kills_before} kills before the restart"
                        ))
                    })
                })
                .await
                .and_then(|r| r),
            Err(e) => Err(e),
        };
        ctx.record("stats_request shows the kills so far", started, &stats);
        ok = stats.is_ok();
    }

    // Whatever happened: the bot stopped, out of the Arena, HP back, the brain stopped.
    let started = Instant::now();
    let stopped = match gw.bot(BotAction::Stop).await {
        Ok(()) => {
            gw.wait_for(Duration::from_secs(10), "the bot stopped", |g| {
                (bot_field(&g.character, "state") != Some("running")
                    && bot_field(&g.character, "state") != Some("paused"))
                .then(|| {
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
    let brain = match running {
        Some(b) => stop_brain(ctx, b).await,
        None => false,
    };
    ok && stopped.is_ok() && out && rested && brain
}

/// The typing scenario, with the gateway in the realm: a brain; to the Arena and Start; once
/// the bot is fighting, `st` typed from the UI connection shows the status screen while the bot
/// keeps running (no "took over"); then `u` typed pauses it ("You took over", "Paused: you took
/// over") and the character goes up the stairs to the Narrow Road. Then the bot stopped, rest by
/// hand if HP is low, the brain stopped.
async fn typing_scenario(ctx: &mut Ctx, gw: &mut Gateway) -> bool {
    use wayfarer_proto::BotAction;

    let Some((running, start)) = placed_brain(ctx, gw, "typing", &[]).await else {
        return false;
    };
    let mut ok = fight_in_the_arena(ctx, gw, start, false).await;

    // 4. The bot fighting: its own `*Combat Engaged*` after Start.
    if ok {
        let started = Instant::now();
        let seq = gw.last_seq();
        let engaged = gw
            .wait_for(ENGAGE_TIMEOUT, "the bot fighting", |g| {
                if bot_field(&g.character, "state") != Some("running") {
                    let why = bot_field(&g.character, "narration").unwrap_or("");
                    return Some(Err(format!("the bot is not running: {why}")));
                }
                let fighting =
                    bot_field(&g.character, "narration").is_some_and(|n| n.starts_with("Fight:"));
                (fighting && screen::plain(&g.text_after(seq)).contains("*Combat Engaged*")).then(
                    || {
                        Ok(bot_field(&g.character, "narration")
                            .unwrap_or("")
                            .to_owned())
                    },
                )
            })
            .await
            .and_then(|r| r);
        ctx.record("the bot fighting in the Arena", started, &engaged);
        ok = engaged.is_ok();
    }

    // 5. `st` beside the bot: the status screen, and the bot still running.
    if ok {
        let started = Instant::now();
        let notices = gw.notices.len();
        let status = status_screen(gw).await.and_then(|name| {
            let took = gw.notices[notices..]
                .iter()
                .find(|n| n.contains("took over") || n.starts_with("Paused"));
            match (took, bot_field(&gw.character, "state")) {
                (Some(n), _) => Err(format!("the bot paused: {n}")),
                (None, Some("running")) => Ok(format!(
                    "{name}; the bot still running: {}",
                    bot_field(&gw.character, "narration").unwrap_or("")
                )),
                (None, state) => Err(format!("the bot is {state:?}")),
            }
        });
        ctx.record(
            "`st` shows the status screen without a pause",
            started,
            &status,
        );
        ok = status.is_ok();
    }

    // 6. `u` beside the bot: it pauses, and the character goes up the stairs.
    if ok {
        let started = Instant::now();
        let notices = gw.notices.len();
        let moved = match gw.send_line("u").await {
            Ok(()) => {
                gw.wait_for(Duration::from_secs(20), "the pause and the move", |g| {
                    let said = &g.notices[notices..];
                    let took = said.iter().find(|n| n.starts_with("You took over"))?;
                    said.iter().find(|n| *n == "Paused: you took over")?;
                    let (room, title) = believed(&g.character)?;
                    (bot_field(&g.character, "state") == Some("paused") && room == WALK_TO)
                        .then(|| format!("{took}; paused; now in {title}"))
                })
                .await
            }
            Err(e) => Err(e),
        };
        ctx.record("`u` pauses the bot and moves", started, &moved);
        ok = moved.is_ok();
    }

    // Whatever happened: the bot stopped, out of the Arena, HP back, the brain stopped.
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
    let brain = stop_brain(ctx, running).await;
    ok && stopped.is_ok() && out && rested && brain
}

/// Steps 2 to 4 of the fight scenario: the walk there, Start, `*Combat Engaged*` and the fight
/// to a kill or a stop; without `to_the_end`, only the walk there and Start.
async fn fight_in_the_arena(
    ctx: &mut Ctx,
    gw: &mut Gateway,
    start: (i64, i64),
    to_the_end: bool,
) -> bool {
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
    if !to_the_end {
        let running = gw
            .wait_for(Duration::from_secs(10), "the bot running", |g| {
                (bot_field(&g.character, "state") == Some("running")).then(|| {
                    bot_field(&g.character, "narration")
                        .unwrap_or("")
                        .to_owned()
                })
            })
            .await;
        ctx.record("start: the bot running", started, &running);
        return running.is_ok();
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

/// An attack this recent counts as the character being under attack.
const RECENT_ATTACK: Duration = Duration::from_secs(6);
/// The flee scenario's flee line: at full HP, the first blow that takes any sets it off (a
/// level-1 warrior has 40 HP and the Arena's rats take 1 or 2: 38/40 is still 95%).
const FLEE_LINE: &str = "100";
/// From the flee to the walk back into the Arena: a rest from just under 95% to 95%.
const FLEE_BACK_TIMEOUT: Duration = Duration::from_secs(600);

/// The notices since `from` and whether the bot stopped meanwhile (a warning ends the wait).
fn after(g: &Gateway, from: usize) -> &[String] {
    &g.notices[from.min(g.notices.len())..]
}

/// The flee scenario, with the gateway in the realm: a brain fleeing below 100%; rest to full
/// HP; to the Arena; Start; the first blow: Flee, by a confirmed exit to a quiet room; rested
/// there; back to the Arena. Then as "fight" ends: Stop, out of the Arena, rest by hand,
/// the brain stopped; and no death on the way. Whether all of it passed.
async fn flee_scenario(ctx: &mut Ctx, gw: &mut Gateway) -> bool {
    use wayfarer_proto::BotAction;

    let extra = ["--flee-below".to_owned(), FLEE_LINE.to_owned()];
    let Some((running, start)) = placed_brain(ctx, gw, "flee", &extra).await else {
        return false;
    };
    let notices = gw.notices.len();
    let attacks = gw.attacks.len();
    let mut ok = rest_to_full(ctx, gw).await && fight_in_the_arena(ctx, gw, start, false).await;

    // 5. The first blow: Flee, out by a confirmed exit, to a room where nothing follows.
    if ok {
        let started = Instant::now();
        let fled = gw
            .wait_for(FIGHT_TIMEOUT, "Flee to a quiet room", |g| {
                if bot_field(&g.character, "state") == Some("stopped") {
                    let why = bot_field(&g.character, "narration").unwrap_or("");
                    return Some(Err(format!("the bot stopped: {why}")));
                }
                let said = after(g, notices);
                let from = said.iter().position(|n| n.starts_with("Fleeing: HP"))?;
                let safe = said[from..]
                    .iter()
                    .position(|n| n.starts_with("Safe in") || n.starts_with("Fled "))?;
                let attack = g.attacks[attacks.min(g.attacks.len())..]
                    .first()
                    .map_or_else(
                        || "no attack seen".to_owned(),
                        |a| format!("attacked by {}: \"{}\"", a.who, a.line),
                    );
                Some(Ok(format!(
                    "{attack}; {}",
                    said[from..=from + safe].join("; ")
                )))
            })
            .await
            .and_then(|r| r);
        ctx.record("the first blow: flee to a quiet room", started, &fled);
        ok = fled.is_ok();
    }

    // 6. Rest there, then walk back to the Arena (home).
    if ok {
        let started = Instant::now();
        let back = gw
            .wait_for(FLEE_BACK_TIMEOUT, "rested, back in the Arena", |g| {
                if bot_field(&g.character, "state") == Some("stopped") {
                    let why = bot_field(&g.character, "narration").unwrap_or("");
                    return Some(Err(format!("the bot stopped: {why}")));
                }
                let said = after(g, notices);
                let rest = said
                    .iter()
                    .position(|n| n.starts_with("Recover: resting"))?;
                let walk = rest
                    + said[rest..]
                        .iter()
                        .position(|n| n == "Recover: rested; walking back to Newhaven, Arena")?;
                said[walk..]
                    .iter()
                    .any(|n| n == "Arrived: Newhaven, Arena")
                    .then(|| {
                        Ok(format!(
                            "{}; {}; Arrived: Newhaven, Arena",
                            said[rest], said[walk]
                        ))
                    })
            })
            .await
            .and_then(|r| r);
        ctx.record("rests in the quiet room, walks back", started, &back);
        ok = back.is_ok();
    }

    // Whatever happened: the bot stopped, out of the Arena, HP back, the brain stopped.
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
    let started = Instant::now();
    let died = gw.transcript().contains("You have been killed");
    let flees = gw.notices[notices.min(gw.notices.len())..]
        .iter()
        .filter(|n| n.starts_with("Fleeing: HP"))
        .count();
    let alive = if died {
        Err("the character was killed".to_owned())
    } else {
        Ok(format!(
            "no death; {flees} flees, {} attacks seen",
            gw.attacks.len() - attacks
        ))
    };
    ctx.record("no death", started, &alive);
    let brain = stop_brain(ctx, running).await;
    ok && stopped.is_ok() && out && rested && alive.is_ok() && brain
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

/// `rest` by hand to full HP, where the character stands (the flee scenario starts at full HP,
/// so the first blow sets the flee off).
async fn rest_to_full(ctx: &mut Ctx, gw: &mut Gateway) -> bool {
    let Some((now, max)) = hp(&gw.character) else {
        return true;
    };
    if now >= max {
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
            .wait_for(Duration::from_secs(60), "full HP", |g| {
                let (hp, max) = hp(&g.character)?;
                (hp >= max).then(|| format!("rested from {now} to {hp} of {max}"))
            })
            .await;
        if r.is_ok() {
            rested = r;
            break;
        }
    }
    ctx.record("rest to full HP", started, &rested);
    rested.is_ok()
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
