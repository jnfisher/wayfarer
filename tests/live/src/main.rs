//! `wayfarer-live`: the M0 scenario against a game server that is already running, run by
//! `scripts/live-check.sh` on the laptop and never in CI (tests/live/README.md).
//!
//! It claims a test character from the pool (creating its account and character the first
//! time), starts `wayfarer gateway` for it, and through the gateway's WebSocket checks that it
//! reaches the realm, that `st` shows the status screen, that stopping the gateway logs it off
//! cleanly, and that the next logon carries no punishment notice. Then it prints a summary for
//! the pull request.

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
}

fn usage() -> ! {
    eprintln!(
        "usage: wayfarer-live --wayfarer BIN --out DIR [--addr HOST:PORT] [--pool FILE] \
         [--credentials FILE] [--char ACCOUNT]\n(run it through scripts/live-check.sh)"
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
