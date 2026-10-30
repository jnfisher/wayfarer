//! Logon scripts: the `prompt → reply` steps that take a character from connect to the realm
//! prompt, the log-off steps that take it out again, and following the text to know where a
//! session is (logon, menu, realm).
//!
//! Everything here is a plain state machine fed with decoded text; the session task feeds it
//! and acts on what it decides. Matching works on a rolling window of recent text with ANSI
//! escapes removed, consumed up to the end of each match.
//!
//! The presets (`presets/*.toml`, compiled in) say what the format is; a server picks one with
//! `preset` and may replace any part of it with its own `[server.logon]` table.

use std::sync::Arc;
use std::time::Duration;

use regex::Regex;
use serde::{Deserialize, Serialize};

/// The presets compiled into the binary, by name.
pub const PRESETS: [(&str, &str); 2] = [
    ("mbbsemu", include_str!("presets/mbbsemu.toml")),
    ("silvermere", include_str!("presets/silvermere.toml")),
];

/// A step's timeout when the script does not give one.
const STEP_TIMEOUT: Duration = Duration::from_secs(30);
/// A log-off step's timeout when the script does not give one.
const LOGOFF_TIMEOUT: Duration = Duration::from_secs(20);
/// How many times a log-off step is sent again after a `retry_on` text, when the script does
/// not say.
const LOGOFF_RETRIES: u32 = 20;
/// How much recent text a window keeps; prompts are short, the text before them can be long.
const WINDOW_BYTES: usize = 16 << 10;

/// A logon script as written in TOML: a preset file, or a server's `[server.logon]` table. Every
/// field is optional so a server can replace parts of a preset; [`Script::build`] checks the
/// result is whole. It serializes back to the same table (the Accounts panel's save keeps a
/// server's own `[server.logon]`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptSpec {
    /// The realm prompt: the script has succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realm: Option<String>,
    /// Seconds to wait for the realm prompt after the last step.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realm_timeout: Option<u64>,
    #[serde(rename = "step", skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<StepSpec>>,
    #[serde(rename = "failure", skip_serializing_if = "Option::is_none")]
    pub failures: Option<Vec<FailureSpec>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logoff: Option<Vec<LogoffSpec>>,
    /// Making a BBS account (`NEW`) for a user the BBS does not know.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signup: Option<SignupSpec>,
}

/// `[[step]]`: when `expect` shows, send `send`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StepSpec {
    pub expect: String,
    pub send: String,
    /// The step may not come; a later step's prompt passes it by.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
    /// Seconds to wait for `expect`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}

/// `[[failure]]`: text that means the logon has failed.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FailureSpec {
    pub expect: String,
    /// Shown as the session's reason.
    pub reason: String,
    /// Reconnect with backoff (otherwise the session closes).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub retry: bool,
    /// The failure may mean the BBS does not know the user: a character waiting for its
    /// account to be made signs up instead ([`Script::signup`]).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub signup: bool,
}

/// `[[logoff]]`: at `at` (the realm for the first step), send `send`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LogoffSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    pub send: String,
    /// Seconds to wait for the next step's `at` (or, after the last, for the server to close),
    /// from the step's first send: its retries included.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    /// Texts that mean "not yet" (`meditation has been interrupted`): the step is sent again
    /// at its place's next prompt.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retry_on: Vec<String>,
    /// How many times the step may be sent again (20 if not given).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retries: Option<u32>,
}

/// `[signup]`: the BBS's new-account dialogue, run instead of the logon steps for a character
/// whose account is to be made. Its steps may also use `{email}` and `{gender}`. It ends at
/// `stop_at` (the MajorMUD menu), where the session stops and sends nothing more, so the player
/// makes the character. A question asked again means the BBS refused the answer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SignupSpec {
    pub stop_at: String,
    /// Seconds to wait for `stop_at` after the last step.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_timeout: Option<u64>,
    #[serde(rename = "step")]
    pub steps: Vec<StepSpec>,
}

impl ScriptSpec {
    fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// `self`, with every part `over` gives replaced by it.
    fn overlay(mut self, over: &Self) -> Self {
        let over = over.clone();
        self.realm = over.realm.or(self.realm);
        self.realm_timeout = over.realm_timeout.or(self.realm_timeout);
        self.steps = over.steps.or(self.steps);
        self.failures = over.failures.or(self.failures);
        self.logoff = over.logoff.or(self.logoff);
        self.signup = over.signup.or(self.signup);
        self
    }
}

/// What a step's `expect` (or a log-off step's `at`) matches: a substring, or a regex written
/// `re:<regex>`.
#[derive(Debug, Clone)]
pub enum Pattern {
    Text(String),
    Regex(Regex),
}

impl Pattern {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.strip_prefix("re:") {
            Some(re) => Regex::new(re)
                .map(Self::Regex)
                .map_err(|e| format!("{s:?}: {e}")),
            None if s.is_empty() => Err("an empty pattern matches everything".to_owned()),
            None => Ok(Self::Text(s.to_owned())),
        }
    }

    /// Where the first match in `hay` ends.
    pub fn first_end(&self, hay: &str) -> Option<usize> {
        self.first(hay).map(|(_, end)| end)
    }

    /// Where the first match in `hay` starts and ends.
    pub fn first(&self, hay: &str) -> Option<(usize, usize)> {
        match self {
            Self::Text(t) => hay.find(t.as_str()).map(|i| (i, i + t.len())),
            Self::Regex(r) => r.find(hay).map(|m| (m.start(), m.end())),
        }
    }

    /// Where the last match in `hay` ends.
    pub fn last_end(&self, hay: &str) -> Option<usize> {
        match self {
            Self::Text(t) => hay.rfind(t.as_str()).map(|i| i + t.len()),
            Self::Regex(r) => r.find_iter(hay).last().map(|m| m.end()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Step {
    pub expect: Pattern,
    pub send: String,
    pub optional: bool,
    pub timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct Failure {
    pub expect: Pattern,
    pub reason: String,
    pub retry: bool,
    pub signup: bool,
}

#[derive(Debug, Clone)]
pub struct LogoffStep {
    /// `None` for the first step: the realm.
    pub at: Option<Pattern>,
    pub send: String,
    pub timeout: Duration,
    pub retry_on: Vec<Pattern>,
    pub retries: u32,
}

/// A checked logon script.
#[derive(Debug, Clone)]
pub struct Script {
    pub steps: Vec<Step>,
    pub realm: Pattern,
    pub realm_timeout: Duration,
    pub failures: Vec<Failure>,
    pub logoff: Vec<LogoffStep>,
    /// The new-account dialogue (`[signup]`), as a script of its own: its steps, `stop_at` as
    /// its realm, and this script's failures and log-off. `None`: this server's script can't
    /// make accounts.
    pub signup: Option<Arc<Script>>,
}

impl Script {
    /// A preset by name, unchanged.
    pub fn preset(name: &str) -> Result<Self, String> {
        Self::build(Some(name), None).map(|s| s.expect("a preset gives a script"))
    }

    /// The script for a server: its `preset` with its inline `logon` table over it. `None` when
    /// it gives neither (the player logs on by hand).
    pub fn build(
        preset: Option<&str>,
        inline: Option<&ScriptSpec>,
    ) -> Result<Option<Self>, String> {
        let base = match preset {
            Some(name) => {
                let (_, text) = PRESETS.iter().find(|(n, _)| *n == name).ok_or_else(|| {
                    let names: Vec<_> = PRESETS.iter().map(|(n, _)| *n).collect();
                    format!("no logon preset {name:?} (there are {})", names.join(", "))
                })?;
                ScriptSpec::parse(text).map_err(|e| format!("preset {name}: {e}"))?
            }
            None if inline.is_none() => return Ok(None),
            None => ScriptSpec::default(),
        };
        let spec = match inline {
            Some(inline) => base.overlay(inline),
            None => base,
        };
        Self::from_spec(&spec).map(Some)
    }

    fn from_spec(spec: &ScriptSpec) -> Result<Self, String> {
        let secs = |s: Option<u64>, default| s.map_or(default, Duration::from_secs);
        let realm = Pattern::parse(
            spec.realm
                .as_deref()
                .ok_or("the logon script has no realm")?,
        )?;
        let steps = spec
            .steps
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or("the logon script has no steps")?
            .iter()
            .map(|s| {
                Ok(Step {
                    expect: Pattern::parse(&s.expect)?,
                    send: s.send.clone(),
                    optional: s.optional,
                    timeout: secs(s.timeout, STEP_TIMEOUT),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let failures = spec
            .failures
            .iter()
            .flatten()
            .map(|f| {
                Ok(Failure {
                    expect: Pattern::parse(&f.expect)?,
                    reason: f.reason.clone(),
                    retry: f.retry,
                    signup: f.signup,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let logoff = spec
            .logoff
            .iter()
            .flatten()
            .enumerate()
            .map(|(i, l)| {
                let at = match (i, &l.at) {
                    (0, None) => None,
                    (0, Some(_)) => {
                        return Err("the first log-off step is at the realm: no `at`".to_owned());
                    }
                    (_, None) => return Err(format!("log-off step {} has no `at`", i + 1)),
                    (_, Some(at)) => Some(Pattern::parse(at)?),
                };
                Ok(LogoffStep {
                    at,
                    send: l.send.clone(),
                    timeout: secs(l.timeout, LOGOFF_TIMEOUT),
                    retry_on: l
                        .retry_on
                        .iter()
                        .map(|r| Pattern::parse(r))
                        .collect::<Result<_, _>>()?,
                    retries: l.retries.unwrap_or(LOGOFF_RETRIES),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let signup = match &spec.signup {
            None => None,
            Some(s) => {
                let inner = ScriptSpec {
                    realm: Some(s.stop_at.clone()),
                    realm_timeout: s.stop_timeout,
                    steps: Some(s.steps.clone()),
                    failures: spec.failures.clone(),
                    logoff: spec.logoff.clone(),
                    signup: None,
                };
                Some(Arc::new(
                    Self::from_spec(&inner).map_err(|e| format!("[signup]: {e}"))?,
                ))
            }
        };
        Ok(Self {
            steps,
            realm,
            realm_timeout: secs(spec.realm_timeout, STEP_TIMEOUT),
            failures,
            logoff,
            signup,
        })
    }
}

/// Recent text with ANSI escapes removed, kept to [`WINDOW_BYTES`]. An escape split across two
/// feeds is still removed.
#[derive(Debug, Default)]
struct Window {
    text: String,
    escape: Escape,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Escape {
    #[default]
    None,
    /// After ESC.
    Esc,
    /// In `ESC [ ...` until its final byte.
    Csi,
}

impl Window {
    fn push(&mut self, text: &str) {
        for c in text.chars() {
            self.escape = match (self.escape, c) {
                (Escape::None, '\x1b') => Escape::Esc,
                (Escape::None, c) => {
                    self.text.push(c);
                    Escape::None
                }
                (Escape::Esc, '[') => Escape::Csi,
                // A two-character escape (`ESC 7`, `ESC c`): both go.
                (Escape::Esc, _) => Escape::None,
                (Escape::Csi, '\x40'..='\x7e') => Escape::None,
                (Escape::Csi, _) => Escape::Csi,
            };
        }
        if self.text.len() > WINDOW_BYTES {
            let mut cut = self.text.len() - WINDOW_BYTES;
            while !self.text.is_char_boundary(cut) {
                cut += 1;
            }
            self.text.drain(..cut);
        }
    }

    /// Forget everything up to `end`.
    fn consume(&mut self, end: usize) {
        self.text.drain(..end);
    }
}

/// What the logon runner wants done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Send a line: `text` to the game, `shown` (the password masked) to `sent.bin`.
    Send { text: String, shown: String },
    /// The realm prompt: logged on.
    Realm,
    /// The logon failed. `signup`: the failure may mean the BBS does not know the user.
    Failed {
        reason: String,
        retry: bool,
        signup: bool,
    },
}

/// Runs a script's logon steps over the text of one connection.
#[derive(Debug)]
pub struct Runner {
    script: Arc<Script>,
    user: String,
    password: String,
    /// The sign-up's `{email}` and `{gender}`; `Some` only for a sign-up, where a question
    /// asked again is a refusal.
    signup: Option<(String, String)>,
    /// The first step not yet sent.
    next: usize,
    window: Window,
    finished: bool,
}

impl Runner {
    pub fn new(script: Arc<Script>, user: &str, password: &str) -> Self {
        Self {
            script,
            user: user.to_owned(),
            password: password.to_owned(),
            signup: None,
            next: 0,
            window: Window::default(),
            finished: false,
        }
    }

    /// Run `script`'s sign-up ([`Script::signup`]) instead of its logon: [`Action::Realm`]
    /// then means it reached `stop_at`. `None` if the script has no sign-up.
    pub fn signup(
        script: &Script,
        user: &str,
        password: &str,
        email: &str,
        gender: &str,
    ) -> Option<Self> {
        let mut runner = Self::new(Arc::clone(script.signup.as_ref()?), user, password);
        runner.signup = Some((email.to_owned(), gender.to_owned()));
        Some(runner)
    }

    /// Feed decoded text; returns what to do, in order. Nothing more once it has reported
    /// [`Action::Realm`] or [`Action::Failed`].
    pub fn feed(&mut self, text: &str) -> Vec<Action> {
        let mut actions = Vec::new();
        if self.finished {
            return actions;
        }
        self.window.push(text);
        loop {
            let hay = self.window.text.as_str();
            if let Some(f) = self
                .script
                .failures
                .iter()
                .find(|f| f.expect.first_end(hay).is_some())
            {
                self.finished = true;
                actions.push(Action::Failed {
                    reason: f.reason.clone(),
                    retry: f.retry,
                    signup: f.signup,
                });
                return actions;
            }
            if self.script.realm.first_end(hay).is_some() {
                self.finished = true;
                actions.push(Action::Realm);
                return actions;
            }
            // The next step, or a later one past optional steps: whichever prompt came first.
            let mut best: Option<(usize, usize)> = None;
            for (i, step) in self.script.steps.iter().enumerate().skip(self.next) {
                if let Some(end) = step.expect.first_end(hay)
                    && best.is_none_or(|(_, e)| end < e)
                {
                    best = Some((i, end));
                }
                if !step.optional {
                    break;
                }
            }
            // A sign-up question already answered, asked again before the next one: the BBS
            // refused the answer, and says why on the line before.
            if self.signup.is_some()
                && let Some(start) = self.script.steps[..self.next]
                    .iter()
                    .filter_map(|s| s.expect.first(hay))
                    .filter(|(_, e)| best.is_none_or(|(_, b)| *e < b))
                    .map(|(start, _)| start)
                    .min()
            {
                self.finished = true;
                actions.push(Action::Failed {
                    reason: refusal(&hay[..start]),
                    retry: false,
                    signup: false,
                });
                return actions;
            }
            let Some((i, end)) = best else {
                return actions;
            };
            self.window.consume(end);
            self.next = i + 1;
            let send = &self.script.steps[i].send;
            actions.push(Action::Send {
                text: self.fill(send, &self.password),
                shown: self.fill(send, crate::session::MASK),
            });
        }
    }

    fn fill(&self, send: &str, password: &str) -> String {
        let (email, gender) = self
            .signup
            .as_ref()
            .map_or(("", ""), |(e, g)| (e.as_str(), g.as_str()));
        send.replace("{user}", &self.user)
            .replace("{password}", password)
            .replace("{email}", email)
            .replace("{gender}", gender)
    }

    /// Every step that must come has been sent; only the realm prompt is awaited.
    pub fn waiting_for_realm(&self) -> bool {
        self.script.steps[self.next..].iter().all(|s| s.optional)
    }

    /// How long to wait for what comes next, from the last step sent (or the connect).
    pub fn timeout(&self) -> Duration {
        match self.script.steps.get(self.next) {
            Some(step) if !self.waiting_for_realm() => step.timeout,
            _ => self.script.realm_timeout,
        }
    }
}

/// What a BBS said before asking a sign-up question again: its last line that is not blank.
fn refusal(before: &str) -> String {
    before
        .lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .map_or_else(|| "the BBS refused the sign-up".to_owned(), str::to_owned)
}

/// Where a session is, from the text: at the realm prompt, or at log-off step `n`'s place (a
/// menu).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Realm,
    Menu(usize),
}

/// Follows the text of a connection to know the latest [`Place`] it showed.
#[derive(Debug)]
pub struct Tracker {
    script: Arc<Script>,
    window: Window,
    place: Option<Place>,
}

impl Tracker {
    pub fn new(script: Arc<Script>) -> Self {
        Self {
            script,
            window: Window::default(),
            place: None,
        }
    }

    pub fn place(&self) -> Option<Place> {
        self.place
    }

    /// Feed decoded text; returns the place when the text showed one (the latest, if several).
    pub fn feed(&mut self, text: &str) -> Option<Place> {
        self.window.push(text);
        let hay = self.window.text.as_str();
        let realm = self
            .script
            .realm
            .last_end(hay)
            .map(|end| (end, Place::Realm));
        let menus = self
            .script
            .logoff
            .iter()
            .enumerate()
            .filter_map(|(i, step)| {
                let end = step.at.as_ref()?.last_end(hay)?;
                Some((end, Place::Menu(i)))
            });
        let (end, place) = realm.into_iter().chain(menus).max_by_key(|(end, _)| *end)?;
        self.window.consume(end);
        self.place = Some(place);
        Some(place)
    }
}

/// What the log-off runner wants done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogoffAction {
    /// The next step's place showed: send its line. Its timeout runs from here.
    Send(String),
    /// The step was answered "not yet" and its place showed again: send it again, retry
    /// `retry` of `of`. The step's timeout keeps running.
    Retry { send: String, retry: u32, of: u32 },
    /// "Not yet" once more with no retries left: the log-off has failed.
    GiveUp,
}

/// Runs a script's log-off steps from where the session is.
#[derive(Debug)]
pub struct Logoff {
    script: Arc<Script>,
    /// The step sent last.
    sent: usize,
    /// Its retries so far.
    retries: u32,
    /// It was answered "not yet": its place's prompt is awaited to send it again.
    again: bool,
    gave_up: bool,
    window: Window,
}

impl Logoff {
    /// Start at `place`: returns the runner and the first line to send, or `None` if the
    /// script has no log-off steps from there.
    pub fn start(script: Arc<Script>, place: Place) -> Option<(Self, String)> {
        let first = match place {
            Place::Realm => 0,
            Place::Menu(i) => i,
        };
        let send = script.logoff.get(first)?.send.clone();
        let logoff = Self {
            script,
            sent: first,
            retries: 0,
            again: false,
            gave_up: false,
            window: Window::default(),
        };
        Some((logoff, send))
    }

    /// Feed decoded text; returns what to do, in order. Nothing more after
    /// [`LogoffAction::GiveUp`].
    pub fn feed(&mut self, text: &str) -> Vec<LogoffAction> {
        let mut actions = Vec::new();
        if self.gave_up {
            return actions;
        }
        self.window.push(text);
        loop {
            let script = Arc::clone(&self.script);
            let step = &script.logoff[self.sent];
            let hay = self.window.text.as_str();
            if self.again {
                // The step's own place: the realm prompt for the first step.
                let place = step.at.as_ref().unwrap_or(&script.realm);
                let Some(end) = place.first_end(hay) else {
                    break;
                };
                self.window.consume(end);
                self.again = false;
                self.retries += 1;
                actions.push(LogoffAction::Retry {
                    send: step.send.clone(),
                    retry: self.retries,
                    of: step.retries,
                });
                continue;
            }
            let next = script.logoff.get(self.sent + 1).map(|next| {
                let at = next
                    .at
                    .as_ref()
                    .expect("Script checks every later step has `at`");
                (at.first_end(hay), next)
            });
            let not_yet = step.retry_on.iter().filter_map(|r| r.first_end(hay)).min();
            match (next, not_yet) {
                // Whichever came first.
                (Some((Some(end), next)), n) if n.is_none_or(|n| end < n) => {
                    self.window.consume(end);
                    self.sent += 1;
                    self.retries = 0;
                    actions.push(LogoffAction::Send(next.send.clone()));
                }
                (_, Some(end)) => {
                    self.window.consume(end);
                    if self.retries >= step.retries {
                        self.gave_up = true;
                        actions.push(LogoffAction::GiveUp);
                        break;
                    }
                    self.again = true;
                }
                _ => break,
            }
        }
        actions
    }

    /// Whether the log-off is still at the realm (its first step), where hanging up drops the
    /// character in the realm.
    pub fn in_realm(&self) -> bool {
        self.script.logoff[self.sent].at.is_none()
    }

    /// How long to wait, from the last line sent, for the next step's place (or the close).
    pub fn timeout(&self) -> Duration {
        self.script.logoff[self.sent].timeout
    }
}

/// The wait before reconnect attempt `attempt` (0 first): 5 s, 10 s, 30 s, then every 60 s.
pub fn backoff(attempt: u32) -> Duration {
    const SCHEDULE: [u64; 3] = [5, 10, 30];
    Duration::from_secs(SCHEDULE.get(attempt as usize).copied().unwrap_or(60))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preset(name: &str) -> Arc<Script> {
        Arc::new(Script::preset(name).unwrap())
    }

    fn sends(actions: &[Action]) -> Vec<&str> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Send { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    /// The MBBSEmu dialogue, as `entry-existing` shows it (ANSI as the server colours it).
    const MBBSEMU: [&str; 5] = [
        "\x1b[?7h banner \r\nEnter Username or enter \"NEW\" to create a new Account\r\nUsername: ",
        "Aldric\r\nPassword: ",
        "*******\r\n\r\nFantasy awaits you\r\n   M ... MajorMUD\r\n\r\nMain Menu\r\n\x1b[1;37mMake your selection (X to exit): ",
        "M\r\n\r\n[E] . Enter the Realm\r\n[X] . Exit Game\r\n\r\n[MAJORMUD]: ",
        "E\r\n\r\nNewhaven, Village Entrance\r\nObvious exits: nJ\x08orth\r\n\x1b[0;37m[HP=35]:",
    ];

    #[test]
    fn presets_parse() {
        for (name, _) in PRESETS {
            let s = Script::preset(name).unwrap();
            assert_eq!(s.steps.len(), 4, "{name}");
            assert_eq!(s.logoff.len(), 4, "{name}");
            assert_eq!(s.logoff[0].timeout, Duration::from_secs(240));
            assert_eq!(s.logoff[0].retries, 20, "{name}");
            assert_eq!(s.logoff[0].retry_on.len(), 1, "{name}");
            assert!(s.logoff[1..].iter().all(|l| l.retry_on.is_empty()));
        }
        assert!(
            Script::preset("nope")
                .unwrap_err()
                .contains("mbbsemu, silvermere")
        );
    }

    #[test]
    fn mbbsemu_logs_on() {
        let mut r = Runner::new(preset("mbbsemu"), "Aldric", "aldric1");
        let mut got = Vec::new();
        for (i, text) in MBBSEMU.iter().enumerate() {
            assert!(!r.waiting_for_realm() || i == 4);
            got.extend(r.feed(text));
        }
        assert_eq!(sends(&got), ["Aldric", "aldric1", "M", "E"]);
        assert_eq!(got.last(), Some(&Action::Realm));
        assert!(r.feed("[HP=35]:").is_empty(), "finished");
        // The password is masked in what is shown.
        assert_eq!(
            got[1],
            Action::Send {
                text: "aldric1".into(),
                shown: "********".into()
            }
        );
    }

    #[test]
    fn prompts_split_anywhere_still_match() {
        let all: String = MBBSEMU.concat();
        // The server waits for each reply, so feed one prompt's text at a time, split at every
        // character.
        for split in 1..40 {
            let mut r = Runner::new(preset("mbbsemu"), "u", "p");
            let mut got = Vec::new();
            for text in MBBSEMU {
                let chars: Vec<char> = text.chars().collect();
                for piece in chars.chunks(split) {
                    got.extend(r.feed(&piece.iter().collect::<String>()));
                }
            }
            assert_eq!(sends(&got), ["u", "p", "M", "E"], "split {split}");
            assert_eq!(got.last(), Some(&Action::Realm));
        }
        assert!(all.contains("[HP=35]"));
    }

    #[test]
    fn silvermere_without_the_bbs_menu() {
        let mut r = Runner::new(preset("silvermere"), "u", "p");
        let mut got = r.feed("\x1b[1m\x1b[37mUsername:\x1b[0m ");
        got.extend(r.feed("\x1b[1m\x1b[37mPassword:\x1b[0m "));
        assert!(!r.waiting_for_realm());
        got.extend(r.feed("menu\r\n[MAJORMUD]: "));
        assert!(r.waiting_for_realm());
        got.extend(r.feed("[HP=12/MA=4]:"));
        assert_eq!(sends(&got), ["u", "p", "E"]);
        assert_eq!(got.last(), Some(&Action::Realm));

        // With it: `M` as on MBBSEmu.
        let mut r = Runner::new(preset("silvermere"), "u", "p");
        let got: Vec<_> = MBBSEMU.iter().flat_map(|t| r.feed(t)).collect();
        assert_eq!(sends(&got), ["u", "p", "M", "E"]);
    }

    #[test]
    fn failures() {
        let cases = [
            (
                "mbbsemu",
                "\r\nInvalid Credentials\r\n",
                "bad credentials",
                false,
            ),
            (
                "mbbsemu",
                "\r\nAldric is already logged in -- only 1 connection allowed per user.\r\n",
                "already logged in",
                true,
            ),
            (
                "silvermere",
                "\r\n\x1b[1m\x1b[31mInvalid Credentials\x1b[0m\r\n",
                "bad credentials",
                false,
            ),
            (
                "silvermere",
                "\r\nThis account is already logged on.\r\n",
                "already logged in",
                true,
            ),
            (
                "silvermere",
                "Too many failed logins: this account is locked",
                "account locked",
                false,
            ),
        ];
        for (name, text, reason, retry) in cases {
            let mut r = Runner::new(preset(name), "u", "p");
            assert_eq!(sends(&r.feed("Username: ")), ["u"]);
            assert_eq!(sends(&r.feed("Password: ")), ["p"]);
            let got = r.feed(text);
            assert_eq!(
                got,
                [Action::Failed {
                    reason: reason.into(),
                    retry,
                    signup: reason == "bad credentials",
                }],
                "{name}: {text:?}"
            );
            // MBBSEmu asks again; the script does not answer.
            assert!(r.feed("Username: ").is_empty());
        }
    }

    #[test]
    fn timeouts() {
        let s = Script::build(
            None,
            Some(&ScriptSpec::parse(
                "realm = 'R>'\nrealm_timeout = 7\n[[step]]\nexpect = 'a'\nsend = '1'\ntimeout = 3\n\
                 [[step]]\nexpect = 'b'\nsend = '2'\noptional = true\n",
            )
            .unwrap()),
        )
        .unwrap()
        .unwrap();
        let mut r = Runner::new(Arc::new(s), "u", "p");
        assert_eq!(r.timeout(), Duration::from_secs(3));
        assert!(!r.waiting_for_realm());
        r.feed("a");
        // Only an optional step is left.
        assert!(r.waiting_for_realm());
        assert_eq!(r.timeout(), Duration::from_secs(7));
        assert_eq!(r.feed("R>"), [Action::Realm]);
    }

    #[test]
    fn inline_scripts_replace_parts_of_a_preset() {
        let inline = ScriptSpec::parse(
            "[[step]]\nexpect = 're:Login\\s*:'\nsend = '{user}'\n\
             [[step]]\nexpect = 'Pass:'\nsend = 'pw {password}'\ntimeout = 5\n",
        )
        .unwrap();
        let s = Script::build(Some("mbbsemu"), Some(&inline))
            .unwrap()
            .unwrap();
        assert_eq!(s.steps.len(), 2);
        assert_eq!(s.logoff.len(), 4, "kept from the preset");
        let mut r = Runner::new(Arc::new(s), "me", "secret");
        let got: Vec<_> = ["Login :", "Pass:", "[HP=1]:"]
            .iter()
            .flat_map(|t| r.feed(t))
            .collect();
        assert_eq!(
            got,
            [
                Action::Send {
                    text: "me".into(),
                    shown: "me".into()
                },
                Action::Send {
                    text: "pw secret".into(),
                    shown: "pw ********".into()
                },
                Action::Realm,
            ]
        );
        assert!(Script::build(None, None).unwrap().is_none());
    }

    #[test]
    fn bad_scripts_are_refused() {
        let cases = [
            ("[[step]]\nexpect = 'a'\nsend = 'b'\n", "no realm"),
            ("realm = 'x'\n", "no steps"),
            (
                "realm = 're:('\n[[step]]\nexpect = 'a'\nsend = 'b'\n",
                "re:(",
            ),
            ("realm = ''\n[[step]]\nexpect = 'a'\nsend = 'b'\n", "empty"),
            (
                "realm = 'x'\n[[step]]\nexpect = 'a'\nsend = 'b'\n[[logoff]]\nat = 'm'\nsend = 'x'\n",
                "no `at`",
            ),
            (
                "realm = 'x'\n[[step]]\nexpect = 'a'\nsend = 'b'\n[[logoff]]\nsend = 'x'\n[[logoff]]\nsend = 'y'\n",
                "step 2 has no `at`",
            ),
        ];
        for (text, want) in cases {
            let spec = ScriptSpec::parse(text).unwrap();
            let err = Script::build(None, Some(&spec)).unwrap_err();
            assert!(err.contains(want), "{text}\n=> {err}");
        }
        assert!(ScriptSpec::parse("colour = 'red'\n").is_err());
    }

    #[test]
    fn the_tracker_follows_the_text() {
        let mut t = Tracker::new(preset("mbbsemu"));
        assert_eq!(t.feed(MBBSEMU[0]), None);
        assert_eq!(t.feed(MBBSEMU[2]), Some(Place::Menu(2)));
        assert_eq!(t.feed(MBBSEMU[3]), Some(Place::Menu(1)));
        assert_eq!(t.feed(MBBSEMU[4]), Some(Place::Realm));
        assert_eq!(t.feed("You say \"hi\"\r\n"), None);
        assert_eq!(t.place(), Some(Place::Realm));
        // Several in one feed: the latest wins; split across feeds still counts.
        assert_eq!(t.feed("[HP=3]:x\r\n.....\r\n[MAJ"), Some(Place::Realm));
        assert_eq!(t.feed("ORMUD]: "), Some(Place::Menu(1)));
        assert_eq!(t.feed("\r\n[HP=3]: [MAJORMUD]: "), Some(Place::Menu(1)));
        assert_eq!(t.feed("[HP=3]:"), Some(Place::Realm));
    }

    fn send(line: &str) -> LogoffAction {
        LogoffAction::Send(line.to_owned())
    }

    /// The game's answer to an attack during the meditation (`exit_interrupted`).
    const INTERRUPTED: &str = "\r\nThe kobold thief stabs you for 3 damage!\r\n...Your meditation has been interrupted - you may not exit now!\r\n";

    #[test]
    fn logoff_from_the_realm_and_from_the_menus() {
        let s = preset("mbbsemu");
        let (mut l, first) = Logoff::start(Arc::clone(&s), Place::Realm).unwrap();
        assert_eq!(first, "x");
        assert!(l.in_realm());
        assert_eq!(l.timeout(), Duration::from_secs(240));
        assert!(
            l.feed("You will exit after a period of silent meditation.\r\n....")
                .is_empty()
        );
        assert_eq!(l.feed("\r\n[MAJORMUD]: "), [send("X")]);
        assert!(!l.in_realm());
        assert_eq!(l.timeout(), Duration::from_secs(20));
        assert_eq!(
            l.feed("Main Menu\r\nMake your selection (X to exit): "),
            [send("X")]
        );
        assert_eq!(
            l.feed("Are you sure (Y/N, or R to re-logon)? "),
            [send("Y")]
        );
        assert_eq!(l.timeout(), Duration::from_secs(10));
        assert!(l.feed("Ok, thanks for calling!").is_empty());

        let (mut l, first) = Logoff::start(Arc::clone(&s), Place::Menu(1)).unwrap();
        assert_eq!(first, "X");
        assert_eq!(
            l.feed("Make your selection (X to exit): Are you sure"),
            [send("X"), send("Y")]
        );
        let (_, first) = Logoff::start(Arc::clone(&s), Place::Menu(3)).unwrap();
        assert_eq!(first, "Y");
    }

    #[test]
    fn an_interrupted_meditation_is_tried_again_at_the_next_prompt() {
        let s = preset("silvermere");
        let (mut l, _) = Logoff::start(Arc::clone(&s), Place::Realm).unwrap();
        // The prompt before the interruption does not count; the one after it does.
        assert!(
            l.feed("You will exit after a period of silent meditation.\r\n.\r\n[HP=30/30]:")
                .is_empty()
        );
        for retry in 1..=3 {
            assert!(l.feed(INTERRUPTED).is_empty(), "waits for the prompt");
            assert_eq!(
                l.feed("[HP=27/30]:"),
                [LogoffAction::Retry {
                    send: "x".into(),
                    retry,
                    of: 20
                }]
            );
        }
        // The interruption and the prompt in one piece of text, split anywhere.
        let both = format!("{INTERRUPTED}[HP=24/30]: ");
        assert_eq!(
            l.feed(&both),
            [LogoffAction::Retry {
                send: "x".into(),
                retry: 4,
                of: 20
            }]
        );
        assert!(l.in_realm());
        // Then it goes: the menus as before, the retries forgotten.
        assert_eq!(
            l.feed("You will exit after a period of silent meditation.\r\n...\r\n[MAJORMUD]: "),
            [send("X")]
        );
        assert!(
            l.feed(INTERRUPTED).is_empty(),
            "only the realm step retries"
        );
    }

    #[test]
    fn a_log_off_interrupted_every_time_gives_up_after_its_retries() {
        let spec = ScriptSpec::parse(
            "realm = 'R>'\n[[step]]\nexpect = 'a'\nsend = '1'\n\
             [[logoff]]\nsend = 'x'\nretry_on = ['not yet', 're:wait \\d']\nretries = 2\n\
             [[logoff]]\nat = 'M>'\nsend = 'X'\n",
        )
        .unwrap();
        let s = Arc::new(Script::build(None, Some(&spec)).unwrap().unwrap());
        assert_eq!(s.logoff[0].retries, 2);
        let (mut l, _) = Logoff::start(s, Place::Realm).unwrap();
        let retry = |retry| LogoffAction::Retry {
            send: "x".into(),
            retry,
            of: 2,
        };
        assert_eq!(l.feed("not yet\r\nR>"), [retry(1)]);
        assert_eq!(l.feed("wait 5\r\nR>"), [retry(2)]);
        assert_eq!(l.feed("not yet\r\n"), [LogoffAction::GiveUp]);
        assert!(l.feed("R> not yet R> M>").is_empty(), "finished");
        // Without `retries` a step has 20; without `retry_on`, none are needed.
        let spec = ScriptSpec::parse(
            "realm = 'R>'\n[[step]]\nexpect = 'a'\nsend = '1'\n\
             [[logoff]]\nsend = 'x'\nretry_on = ['not yet']\n",
        )
        .unwrap();
        let s = Script::build(None, Some(&spec)).unwrap().unwrap();
        assert_eq!(s.logoff[0].retries, 20);
        let bad = ScriptSpec::parse(
            "realm = 'R>'\n[[step]]\nexpect = 'a'\nsend = '1'\n\
             [[logoff]]\nsend = 'x'\nretry_on = ['']\n",
        )
        .unwrap();
        assert!(
            Script::build(None, Some(&bad))
                .unwrap_err()
                .contains("empty")
        );
    }

    /// MBBSEmu's sign-up, as `thornwyn-early` shows it, from `NEW` to the MajorMUD menu
    /// (silvermere's prompts are the same).
    const SIGNUP: [&str; 8] = [
        "\x1b[?7h banner \r\nEnter Username or enter \"NEW\" to create a new Account\r\nUsername: ",
        "NEW\r\n\r\n  Creating an account in MBBSEmu is easy!\r\n  You will only need to provide a unique\r\n  Username, secure Password, Gender and\r\n\r\nPlease enter a unique Username (Max. 29 Characters):\r\n",
        "Thornwyn\r\n\r\nNOTE: Passwords are stored in a local database hashed with SHA-512+unique salt.\r\n\r\nPlease enter a strong Password:\r\n",
        "*********\r\n\r\nPlease re-enter your password to confirm:\r\n",
        "*********\r\n\r\nPlease enter a valid e-Mail Address:\r\n",
        "h@h.com\r\n\r\nPlease enter your gender 'M' or 'F' (can be changed later):\r\n",
        "F\r\n\r\nFantasy awaits you in MajorMUD v1.11p-WG!\r\n\r\nMain Menu\r\nMake your selection (X to exit): ",
        "M\r\n\r\n M A J O R  M U D\r\n[E] . Enter the Realm\r\n\r\n[MAJORMUD]: ",
    ];

    fn signup(name: &str) -> Runner {
        Runner::signup(&preset(name), "Thornwyn", "secret1", "h@h.com", "F").unwrap()
    }

    #[test]
    fn a_sign_up_makes_the_account_and_stops_at_the_menu() {
        for name in ["mbbsemu", "silvermere"] {
            let mut r = signup(name);
            let got: Vec<_> = SIGNUP.iter().flat_map(|t| r.feed(t)).collect();
            assert_eq!(
                sends(&got),
                ["NEW", "Thornwyn", "secret1", "secret1", "h@h.com", "F", "M"],
                "{name}"
            );
            assert_eq!(
                got.last(),
                Some(&Action::Realm),
                "{name}: stops at the menu, no E"
            );
            // The password is masked in what is shown.
            assert_eq!(
                got[2],
                Action::Send {
                    text: "secret1".into(),
                    shown: "********".into()
                }
            );
        }
        // silvermere without its BBS menu shim goes straight to the MajorMUD menu.
        let mut r = signup("silvermere");
        let mut got: Vec<_> = SIGNUP[..6].iter().flat_map(|t| r.feed(t)).collect();
        got.extend(r.feed("F\r\n\r\n[MAJORMUD]: "));
        assert_eq!(sends(&got).len(), 6);
        assert_eq!(got.last(), Some(&Action::Realm));
    }

    #[test]
    fn a_question_asked_again_is_a_refusal() {
        // silvermere's refusals (`silvermere-auth`'s login dialogue): a taken name, then a
        // password and its confirmation that differ.
        let cases = [
            (
                2,
                "kasimir\r\nThat Username is unavailable, please choose another.\r\n\r\nPlease enter a unique Username (Max. 29 Characters):\r\n",
                "That Username is unavailable, please choose another.",
            ),
            (
                4,
                "*****\r\nThe passwords do not match.\r\nPlease enter a strong Password:\r\n",
                "The passwords do not match.",
            ),
            (
                5,
                "no address\r\n\r\nPlease enter a valid e-Mail Address:\r\n",
                "no address",
            ),
        ];
        for (answered, again, why) in cases {
            let mut r = signup("silvermere");
            let mut got: Vec<_> = SIGNUP[..answered].iter().flat_map(|t| r.feed(t)).collect();
            got.extend(r.feed(again));
            assert_eq!(
                got.last(),
                Some(&Action::Failed {
                    reason: why.into(),
                    retry: false,
                    signup: false,
                }),
                "{again:?}"
            );
            assert!(r.feed(SIGNUP[answered]).is_empty(), "finished");
        }
        // silvermere with sign-up closed answers NEW and asks for the user again.
        let mut r = signup("silvermere");
        r.feed(SIGNUP[0]);
        assert_eq!(
            r.feed(
                "NEW\r\n\r\nNew accounts are created by the sysop on this system.\r\nUsername: "
            ),
            [Action::Failed {
                reason: "New accounts are created by the sysop on this system.".into(),
                retry: false,
                signup: false,
            }]
        );
        // An ordinary logon never takes a prompt seen again for a refusal.
        let mut r = Runner::new(preset("mbbsemu"), "u", "p");
        r.feed("Username: ");
        assert!(r.feed("Username: ").is_empty());
    }

    #[test]
    fn only_a_script_with_a_sign_up_can_make_accounts() {
        let spec = ScriptSpec::parse("realm = 'R>'\n[[step]]\nexpect = 'a'\nsend = '1'\n").unwrap();
        let s = Script::build(None, Some(&spec)).unwrap().unwrap();
        assert!(Runner::signup(&s, "u", "p", "e", "F").is_none());
        // A server's own logon keeps the preset's sign-up unless it gives its own.
        let s = Script::build(Some("mbbsemu"), Some(&spec))
            .unwrap()
            .unwrap();
        assert!(s.signup.is_some());
        let bad = ScriptSpec::parse(
            "[signup]\nstop_at = ''\n[[signup.step]]\nexpect = 'a'\nsend = 'b'\n",
        )
        .unwrap();
        assert!(
            Script::build(Some("mbbsemu"), Some(&bad))
                .unwrap_err()
                .contains("[signup]")
        );
    }

    #[test]
    fn a_script_spec_writes_back_the_same() {
        for (_, text) in PRESETS {
            let spec = ScriptSpec::parse(text).unwrap();
            let again = ScriptSpec::parse(&toml::to_string(&spec).unwrap()).unwrap();
            assert_eq!(again, spec);
        }
    }

    #[test]
    fn the_backoff_schedule() {
        let secs: Vec<u64> = (0..6).map(|a| backoff(a).as_secs()).collect();
        assert_eq!(secs, [5, 10, 30, 60, 60, 60]);
    }
}
