//! `--scenario accounts` (C-062): a gateway with a server and no characters; the Accounts
//! panel's save adds one whose BBS user doesn't exist yet. The gateway finds the user unknown,
//! makes the account (`NEW`) and stops at the MajorMUD menu without entering. Then it is
//! stopped, which logs off from the menu. Each run makes one throwaway account.

use std::time::{Duration, Instant};

use wayfarer_proto::{AccountCharacter, SessionState, ToGateway};

use crate::Ctx;
use crate::fixtures::generate_password;
use crate::gateway::{Gateway, SESSION, run_dir, tail};

/// From the save to the MajorMUD menu: a failed logon, a reconnect and the sign-up.
const SIGNUP_TIMEOUT: Duration = Duration::from_secs(60);

/// A user name no one has: `wfa` and eight random letters.
fn throwaway_user() -> String {
    let letters: String = generate_password()
        .chars()
        .filter(char::is_ascii_lowercase)
        .collect();
    format!("Wfa{}", &format!("{letters}xxxxxxxx")[..8])
}

pub async fn scenario(ctx: &mut Ctx) -> Result<String, String> {
    let dir = run_dir(&ctx.args.out, &mut ctx.runs, "accounts");
    let config = format!(
        "[[server]]\nid = \"live\"\nhost = \"{}\"\nport = {}\npreset = \"silvermere\"\n",
        ctx.host, ctx.port
    );
    let started = Instant::now();
    let gw = Gateway::start(&ctx.args.wayfarer, &dir, &config, "").await;
    ctx.record(
        "a gateway with no characters",
        started,
        &gw.as_ref().map(|_| String::new()).map_err(Clone::clone),
    );
    let mut gw = gw?;
    let user = throwaway_user();

    let started = Instant::now();
    let saved = save(&mut gw, &user).await;
    ctx.record("save adds a character", started, &saved);
    let made = match saved {
        Ok(_) => {
            let started = Instant::now();
            let made = signed_up(&mut gw, &dir).await;
            ctx.record(
                "the account is made; the session stops at the MajorMUD menu",
                started,
                &made,
            );
            made.is_ok()
        }
        Err(_) => false,
    };

    let started = Instant::now();
    let stopped = gw.stop().await;
    let result = if stopped.clean {
        Ok(stopped.detail)
    } else {
        Err(stopped.detail)
    };
    ctx.record("stop the gateway: log off from the menu", started, &result);
    if made && result.is_ok() {
        Ok(format!("{user} (a new account)"))
    } else {
        Err(user)
    }
}

async fn save(gw: &mut Gateway, user: &str) -> Result<String, String> {
    gw.send(ToGateway::AccountsRequest).await?;
    let mut accounts = gw
        .wait_for(Duration::from_secs(10), "accounts", |g| g.accounts.clone())
        .await?;
    accounts.characters.push(AccountCharacter {
        id: SESSION.into(),
        name: user.into(),
        server: "live".into(),
        autostart: false,
        user: user.into(),
        password: generate_password(),
        email: Some("probe@example.org".into()),
        gender: Some("M".into()),
        reason: None,
    });
    gw.send(ToGateway::AccountsSave { accounts }).await?;
    match gw
        .wait_for(Duration::from_secs(10), "accounts_saved", |g| {
            g.accounts_saved.clone()
        })
        .await?
    {
        None => Ok(String::new()),
        Some(error) => Err(error),
    }
}

/// The session gets to the menu with the reason "account made", and stays there: nothing more
/// is sent (no `E`).
async fn signed_up(gw: &mut Gateway, dir: &std::path::Path) -> Result<String, String> {
    gw.wait_for(SIGNUP_TIMEOUT, "the MajorMUD menu", |g| {
        (g.state == SessionState::Menu && g.transcript().contains("[MAJORMUD]:")).then_some(())
    })
    .await?;
    gw.wait_quiet(
        Duration::from_secs(10),
        Duration::from_secs(3),
        "quiet at the menu",
        |g| g.state == SessionState::Menu,
    )
    .await?;
    gw.accounts = None;
    gw.send(ToGateway::AccountsRequest).await?;
    let accounts = gw
        .wait_for(Duration::from_secs(10), "accounts", |g| g.accounts.clone())
        .await?;
    let reason = accounts
        .characters
        .iter()
        .find(|c| c.id == SESSION)
        .and_then(|c| c.reason.clone())
        .unwrap_or_default();
    if !reason.contains("account made") {
        return Err(format!("the reason is {reason:?}"));
    }
    // What the scripts sent: the first logon's user and password, then the sign-up, and no E.
    let sent = std::fs::read_to_string(dir.join("logs").join(SESSION).join("sent.log"))
        .map_err(|e| format!("sent.log: {e}"))?;
    let lines: Vec<String> = sent
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| v.get("text")?.as_str().map(str::to_owned))
        .collect();
    if !lines.iter().any(|l| l == "NEW") {
        return Err(format!("no NEW was sent: {lines:?}"));
    }
    if lines.iter().any(|l| l == "E") {
        return Err(format!(
            "E was sent: {lines:?}\n{}",
            tail(&gw.transcript(), 12)
        ));
    }
    Ok(format!("sent {}", lines.join(", ")))
}
