//! The incremental parser: terminal text in, events out (`patterns.py` `iter_events`).

use std::collections::VecDeque;
use std::sync::Arc;

use crate::event::{Event, Fields, Value};
use crate::prompt::{Peeled, PromptParser, PromptProgress};
use crate::pyre::{is_py_space, py_strip};
use crate::registry::{JoinRule, Registry, groups};
use crate::text::LineCutter;

/// Commands remembered for echo detection (`patterns.py` keeps four).
const ECHO_DEPTH: usize = 4;
/// How the prototype marks a sent command in a line list (`patterns.SENT`).
const SENT: &str = "\0>";
/// A wrapped display joins at most this many following lines.
const JOIN_MAX: usize = 12;

#[derive(Debug)]
enum Item {
    Line {
        text: String,
        no: u64,
        seq: u64,
    },
    /// A command the client sent, as the prototype marks it in a line list: `"\0>" + cmd`.
    Sent(String),
}

/// What the parser carries from one line to the next.
#[derive(Clone, Debug, Default)]
struct State {
    progress: PromptProgress,
    echo: VecDeque<String>,
    /// The table whose rows are being read, and how many so far.
    table: Option<usize>,
    table_rows: u32,
    /// After a room title, unrecognised lines are the room's long description.
    after_title: bool,
}

/// Prompts already reported for the line in progress, at the end of an earlier chunk.
#[derive(Debug)]
struct Early {
    line_no: u64,
    prompts: Vec<(usize, Fields)>,
}

/// Turns the text of one session into events, incrementally. Feeding a session whole or in
/// any split gives the same events, with one exception: a prompt whose chunk ends after its
/// `]:` but before the end of a resting mark or debug tail that follows it is reported at the
/// end of that chunk without them, then again in full (the game writes a prompt in one piece).
pub struct Parser {
    reg: Arc<Registry>,
    prompt: PromptParser,
    cutter: LineCutter,
    items: VecDeque<Item>,
    state: State,
    lines: u64,
    next_seq: u64,
    early: Option<Early>,
}

impl std::fmt::Debug for Parser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Parser")
            .field("prompt", &self.prompt)
            .field("lines", &self.lines)
            .finish()
    }
}

fn push_echo(echo: &mut VecDeque<String>, marker: &str) {
    echo.push_back(py_strip(&marker[SENT.len()..]).to_owned());
    while echo.len() > ECHO_DEPTH {
        echo.pop_front();
    }
}

fn take_echo(echo: &mut VecDeque<String>, s: &str) -> bool {
    match echo.iter().position(|c| c == s) {
        Some(k) => {
            echo.drain(..=k);
            true
        }
        None => false,
    }
}

/// Could the rest of a line still being received turn out to be the echo of a command?
fn could_be_echo(echo: &VecDeque<String>, rest: &str) -> bool {
    let l = rest.trim_start_matches(is_py_space);
    !l.is_empty() && echo.iter().any(|c| c.starts_with(l) || c == py_strip(rest))
}

fn event(name: &str, cat: &str, fields: Fields, line: &str, seq: u64, line_no: u64) -> Event {
    Event {
        name: name.to_owned(),
        cat: cat.to_owned(),
        fields,
        line: line.to_owned(),
        seq,
        line_no,
    }
}

struct Ctx<'a> {
    reg: &'a Registry,
    prompt: &'a PromptParser,
    items: &'a VecDeque<Item>,
    /// The line in progress, when the lookahead runs out of complete lines.
    partial: Option<&'a str>,
    last: bool,
    early: Option<&'a Early>,
}

enum Next<'a> {
    Line(&'a str),
    Partial(&'a str),
    End,
    Wait,
}

impl Ctx<'_> {
    /// The next line from item `i` on, and its item index. A sent command is not a line of
    /// the display (the prototype joined its marker, `"\0>" + cmd`, into the text).
    fn next(&self, mut i: usize) -> (Next<'_>, usize) {
        loop {
            return match self.items.get(i) {
                Some(Item::Line { text, .. }) => (Next::Line(text), i),
                Some(Item::Sent(_)) => {
                    i += 1;
                    continue;
                }
                None if self.last => (Next::End, i),
                None => (self.partial.map_or(Next::Wait, Next::Partial), i),
            };
        }
    }

    fn seq_of(&self, i: usize) -> u64 {
        match self.items.get(i) {
            Some(Item::Line { seq, .. }) => *seq,
            _ => 0,
        }
    }

    /// `_join`: join a wrapped display's continuation lines; how many items that took.
    /// `None`: more text is needed.
    fn join(&self, s: &mut String, rule: JoinRule) -> Option<usize> {
        let mut used = 0;
        for _ in 0..JOIN_MAX {
            let done_already = match rule {
                JoinRule::Exits => {
                    let last = s.rsplit(' ').next().unwrap_or_default();
                    !(s.ends_with(',') || self.reg.exit_tail.iter().any(|t| t == last))
                }
                JoinRule::DotHere => s.ends_with(" here."),
                JoinRule::Dot => s.ends_with('.'),
                JoinRule::UntilHave => false,
            };
            let stops = |nxt: &str| {
                nxt.is_empty()
                    || nxt.starts_with('[')
                    || (rule == JoinRule::UntilHave
                        && (nxt.starts_with("You have") || nxt.starts_with("Wealth:")))
            };
            let (next, at) = self.next(1 + used);
            match next {
                Next::End => break,
                Next::Line(_) | Next::Partial(_) if done_already => break,
                Next::Line(t) => {
                    let nxt = py_strip(t);
                    if stops(nxt) {
                        break;
                    }
                    s.push(' ');
                    s.push_str(nxt);
                    used = at;
                }
                Next::Partial(p) => {
                    // only a stop can be decided before the line is complete
                    let p = p.trim_start_matches(is_py_space);
                    if !p.is_empty() && stops(p) {
                        break;
                    }
                    return None;
                }
                Next::Wait => return if done_already { Some(used) } else { None },
            }
        }
        Some(used)
    }

    /// One `iter_events` step for the line at the front: its events, and how many items it
    /// took (more than one when a wrapped display is joined). `None`: more text is needed.
    fn line(&self, st: &mut State, out: &mut Vec<Event>) -> Option<usize> {
        let Some(Item::Line { text, no, seq }) = self.items.front() else {
            return Some(1);
        };
        let (no, seq) = (*no, *seq);
        let mut line: &str = text;
        let mut offset = 0;
        let mut early = self
            .early
            .filter(|e| e.line_no == no)
            .map(|e| e.prompts.as_slice());
        let mut k = 0;
        // prompts at the start of the line (several can pile up)
        while let Some((peeled, n)) = self.prompt.peel(&mut st.progress, line) {
            let partial = peeled == Peeled::Partial;
            if let Peeled::Prompt(f) = peeled {
                let end = offset + n;
                let seen = early
                    .and_then(|e| e.get(k))
                    .is_some_and(|(e_end, e_f)| *e_end == end && *e_f == f);
                if !seen {
                    early = None;
                    out.push(event("prompt", "prompt", f, &line[..n], seq, no));
                }
                k += 1;
            }
            st.after_title = false;
            st.table = None;
            line = &line[n..];
            offset += n;
            let rest = py_strip(line);
            if !rest.is_empty() && take_echo(&mut st.echo, rest) {
                let mut f = Fields::new();
                f.insert("cmd".into(), rest.into());
                out.push(event("echo", "echo", f, line, seq, no));
                return Some(1);
            }
            // The prototype peels again here even after the first line of a multi-line custom
            // prompt has taken the whole line; that fails on the empty rest and forgets the
            // prompt in progress, so such a prompt was never recognised.
            if n == 0 || (partial && rest.is_empty()) {
                break;
            }
        }
        let s = py_strip(line);
        if s.is_empty() {
            return Some(1);
        }
        if take_echo(&mut st.echo, s) {
            let mut f = Fields::new();
            f.insert("cmd".into(), s.into());
            out.push(event("echo", "echo", f, line, seq, no));
            return Some(1);
        }
        let mut s = s.to_owned();
        let mut used = 0;
        if let Some((_, rule)) = self
            .reg
            .join
            .iter()
            .find(|(prefix, _)| s.starts_with(prefix.as_str()))
        {
            used = self.join(&mut s, *rule)?;
        }
        let seq = if used > 0 { self.seq_of(used) } else { seq };
        let taken = 1 + used;

        if let Some(t) = st.table {
            let tp = self.reg.table(t);
            if st.table_rows < tp.max_rows
                && let Some(c) = tp.fullmatch(&s)
            {
                st.table_rows += 1;
                out.push(event(&tp.name, &tp.cat, groups(tp, &c), &s, seq, no));
                return Some(taken);
            }
            st.table = None;
        }
        if self.reg.is_room_name(&s) {
            st.after_title = true;
            let mut f = Fields::new();
            f.insert("name".into(), Value::Text(s.clone()));
            out.push(event("room_title", "room", f, &s, seq, no));
            return Some(taken);
        }
        if let Some((p, c)) = self.reg.match_line(&s) {
            if let Some(t) = self.reg.table_for(&p.name) {
                st.table = Some(t);
                st.table_rows = 0;
            }
            if p.cat == "room" && p.name.starts_with("room_") {
                st.after_title = false;
            }
            out.push(event(&p.name, &p.cat, self.reg.fields(p, &c), &s, seq, no));
            return Some(taken);
        }
        if st.after_title {
            out.push(event("room_desc", "room", Fields::new(), &s, seq, no));
        } else {
            out.push(event("unknown", "unclassified", Fields::new(), &s, seq, no));
        }
        Some(taken)
    }
}

impl Parser {
    /// A parser for one session, with the standard prompt.
    pub fn new(reg: Arc<Registry>) -> Parser {
        Self::with_prompt(reg, PromptParser::default())
    }

    pub fn with_prompt(reg: Arc<Registry>, prompt: PromptParser) -> Parser {
        Parser {
            reg,
            prompt,
            cutter: LineCutter::new(),
            items: VecDeque::new(),
            state: State::default(),
            lines: 0,
            next_seq: 0,
            early: None,
        }
    }

    pub fn registry(&self) -> &Arc<Registry> {
        &self.reg
    }

    /// Change the prompt forms (after `set statline`).
    pub fn set_prompt(&mut self, prompt: PromptParser) {
        self.prompt = prompt;
        self.state.progress = None;
    }

    /// The client sent `cmd`: when the game echoes it, the echo is reported as `echo` instead
    /// of being read as game text. Call it when the command goes out.
    pub fn sent(&mut self, cmd: &str) {
        self.items.push_back(Item::Sent(format!("{SENT}{cmd}")));
    }

    /// Feed the next chunk of the session's text (decoded, ANSI included); `seq` is the next
    /// after the last one given.
    pub fn feed(&mut self, text: &str) -> Vec<Event> {
        self.feed_at(self.next_seq, text)
    }

    /// Feed a chunk with its sequence number (the gateway's output log numbers its chunks).
    /// Lines are classified as they complete; a prompt is reported at the end of the chunk.
    pub fn feed_at(&mut self, seq: u64, text: &str) -> Vec<Event> {
        self.next_seq = seq + 1;
        let mut lines = Vec::new();
        self.cutter.feed(text, &mut lines);
        self.push_lines(lines, seq);
        let mut out = Vec::new();
        self.drain(false, &mut out);
        self.prompt_at_end(seq, &mut out);
        out
    }

    /// End of the session's text: the line in progress is complete.
    pub fn finish(&mut self) -> Vec<Event> {
        let seq = self.next_seq.saturating_sub(1);
        let mut lines = Vec::new();
        self.cutter.finish(&mut lines);
        self.push_lines(lines, seq);
        let mut out = Vec::new();
        self.drain(true, &mut out);
        out
    }

    fn push_lines(&mut self, lines: Vec<String>, seq: u64) {
        for text in lines {
            self.items.push_back(Item::Line {
                text,
                no: self.lines,
                seq,
            });
            self.lines += 1;
        }
    }

    fn drain(&mut self, last: bool, out: &mut Vec<Event>) {
        let partial = if last { None } else { self.cutter.partial() };
        loop {
            match self.items.front() {
                None => break,
                Some(Item::Sent(marker)) => {
                    push_echo(&mut self.state.echo, marker);
                    self.items.pop_front();
                }
                Some(Item::Line { text, no, .. }) => {
                    let no = *no;
                    let may_join = self.reg.join.iter().any(|(p, _)| text.contains(p.as_str()));
                    let ctx = Ctx {
                        reg: &self.reg,
                        prompt: &self.prompt,
                        items: &self.items,
                        partial: partial.as_deref(),
                        last,
                        early: self.early.as_ref(),
                    };
                    let taken = if may_join {
                        // a wrapped display may need lines not here yet: try on a copy
                        let mut st = self.state.clone();
                        let mut evs = Vec::new();
                        let Some(n) = ctx.line(&mut st, &mut evs) else {
                            break;
                        };
                        self.state = st;
                        out.extend(evs);
                        n
                    } else {
                        ctx.line(&mut self.state, out).unwrap_or(1)
                    };
                    let mut joined = 0;
                    for item in self.items.drain(..taken) {
                        match item {
                            Item::Line { .. } => joined += 1,
                            // sent while the display was arriving: its echo comes after
                            Item::Sent(marker) => push_echo(&mut self.state.echo, &marker),
                        }
                    }
                    if self
                        .early
                        .as_ref()
                        .is_some_and(|e| e.line_no >= no && e.line_no < no + joined as u64)
                    {
                        self.early = None;
                    }
                }
            }
        }
    }

    /// Report a prompt at the end of the text received so far: the prompt has no newline, and
    /// the game waits after it.
    fn prompt_at_end(&mut self, seq: u64, out: &mut Vec<Event>) {
        if !self.items.is_empty() {
            return;
        }
        let Some(p) = self.cutter.partial() else {
            return;
        };
        let line_no = self.lines;
        let mut done = match self.early.take() {
            Some(e) if e.line_no == line_no => e.prompts,
            _ => Vec::new(),
        };
        let mut progress = self.state.progress.clone();
        let mut line = p.as_str();
        let mut offset = 0;
        let mut k = 0;
        while let Some((peeled, n)) = self.prompt.peel(&mut progress, line) {
            let partial = peeled == Peeled::Partial;
            if let Peeled::Prompt(f) = peeled {
                let end = offset + n;
                if done
                    .get(k)
                    .is_none_or(|(e_end, e_f)| *e_end != end || *e_f != f)
                {
                    done.truncate(k);
                    out.push(event(
                        "prompt",
                        "prompt",
                        f.clone(),
                        &line[..n],
                        seq,
                        line_no,
                    ));
                    done.push((end, f));
                }
                k += 1;
            }
            line = &line[n..];
            offset += n;
            if n == 0
                || (partial && py_strip(line).is_empty())
                || could_be_echo(&self.state.echo, line)
            {
                break;
            }
        }
        if !done.is_empty() {
            self.early = Some(Early {
                line_no,
                prompts: done,
            });
        }
    }
}
