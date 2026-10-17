//! The pattern registry: the static table plus the patterns generated from the game's texts,
//! indexed by each pattern's rarest literal word (`patterns.py` `Registry`).

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::OnceLock;

use regex::{Captures, Regex, RegexBuilder, RegexSet, RegexSetBuilder};

use crate::event::{Fields, Value};
use crate::game_texts::GameTexts;
use crate::pyre::{is_py_space, py_escape, py_split, py_strip, regex_tokens, spec, to_rust};
use crate::table::table;

/// How a wrapped display continues on the next line (`patterns.JOIN`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinRule {
    /// `Obvious exits:`: while the text ends with `,` or an exit word (`door`, `gate`, ...).
    Exits,
    /// `You notice ... here.`: until the text ends with ` here.`.
    DotHere,
    /// `Also here: ...`, keys: until the text ends with `.`.
    Dot,
    /// `You are carrying ...`: until a line starting `You have` or `Wealth:`.
    UntilHave,
}

/// One pattern: a static one from the table, or one generated from a game text.
#[derive(Debug)]
pub struct Pattern {
    /// The event name: a static pattern's name, or the generated role (`mon_hits_you`, ...).
    pub name: String,
    pub cat: String,
    /// Lower wins: 50 specific static patterns, 55-59 game texts with a role, 60 other
    /// players, 70 any other game message, 80 built-in fallbacks, 90 generic fallbacks.
    pub prio: i32,
    /// Literal length of the source; within a priority, the more literal pattern wins.
    pub spec: usize,
    /// The regex as Python source, as `patterns.py` builds it.
    pub source: String,
    /// The game text a generated pattern was built from.
    pub template: Option<String>,
    /// A line the pattern is meant to match (for a generated pattern, its game text).
    pub example: String,
    /// How the static pattern is known: `C` confirmed in the source or transcripts, `F` from
    /// a forum capture only, `I` inferred.
    pub seen: String,
    /// The WCCMSG record a generated pattern came from.
    pub msgnum: Option<i32>,
    /// The index key, or `None` for the patterns tried on every line.
    pub key: Option<String>,
    /// For table rows: the most rows the table takes.
    pub max_rows: u32,
    rx: OnceLock<Option<Regex>>,
}

impl Pattern {
    fn new(name: &str, cat: &str, prio: i32, source: String) -> Pattern {
        Pattern {
            name: name.to_owned(),
            cat: cat.to_owned(),
            prio,
            spec: spec(&source),
            source,
            template: None,
            example: String::new(),
            seen: "C".to_owned(),
            msgnum: None,
            key: None,
            max_rows: 999,
            rx: OnceLock::new(),
        }
    }

    /// The compiled regex (anchored at both ends, as `fullmatch`), built on first use: most of
    /// the generated patterns never meet a line in a session. `None` if it does not compile.
    pub fn regex(&self) -> Option<&Regex> {
        self.rx
            .get_or_init(|| {
                let src = to_rust(&self.source).ok()?;
                RegexBuilder::new(&format!("^(?:{src})$"))
                    .size_limit(1 << 26)
                    .build()
                    .ok()
            })
            .as_ref()
    }

    pub fn fullmatch<'h>(&self, line: &'h str) -> Option<Captures<'h>> {
        let rx = self.regex()?;
        if rx.is_match(line) {
            rx.captures(line)
        } else {
            None
        }
    }
}

/// The role, category and priority of each kind of generated text (`Registry.load_world`).
const MONSTER: &str = "movement";

/// `template_regex`: a game text to a Python regex source. `%s` becomes a lazy group, `%d` a
/// number, `%%` a percent sign; groups are named after the role's arguments (`s<N>`/`n<N>`
/// otherwise) and a repeated name becomes a non-capturing group; the first letter matches either
/// case (the game capitalises monster lines); a trailing run of `%s` becomes an optional tail.
pub fn template_regex(text: &str, argnames: &[String]) -> String {
    let mut t = text;
    let mut opt_tail = "";
    // re.search(r"(?:\s*%s)+$", t): walk back over `\s*%s` groups.
    let mut k = t.len();
    let mut groups = 0;
    while t[..k].ends_with("%s") {
        k -= 2;
        groups += 1;
        while let Some(c) = t[..k].chars().next_back() {
            if !is_py_space(c) {
                break;
            }
            k -= c.len_utf8();
        }
    }
    if groups > 0 && k > 0 {
        t = &t[..k];
        opt_tail = r"(?:\s.*)?";
    }
    // re.split(r"(%[sd]|%%)", t)
    let mut parts: Vec<&str> = Vec::new();
    let mut lit = 0;
    let b = t.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 1 < b.len() && matches!(b[i + 1], b's' | b'd' | b'%') {
            parts.push(&t[lit..i]);
            parts.push(&t[i..i + 2]);
            i += 2;
            lit = i;
        } else {
            i += 1;
        }
    }
    parts.push(&t[lit..]);

    let has_d = t.contains("%d") && argnames.iter().any(|a| a == "dmg");
    let snames: Vec<&String> = argnames
        .iter()
        .filter(|a| !(has_d && *a == "dmg"))
        .collect();
    let mut out = String::with_capacity(t.len() * 2);
    let mut used: Vec<String> = Vec::new();
    let mut si = 0;
    for (n, p) in parts.iter().enumerate() {
        match *p {
            "%s" | "%d" => {
                let nm = if *p == "%d" && has_d {
                    "dmg".to_owned()
                } else {
                    let nm = match snames.get(si) {
                        Some(a) => (*a).clone(),
                        None => format!("{}{}", if *p == "%s" { 's' } else { 'n' }, si + 1),
                    };
                    si += 1;
                    nm
                };
                let rx = if *p == "%s" { ".*?" } else { r"-?\d+" };
                if nm.is_empty() || used.contains(&nm) {
                    out.push_str(&format!("(?:{rx})"));
                } else {
                    out.push_str(&format!("(?P<{nm}>{rx})"));
                    used.push(nm);
                }
            }
            "%%" => out.push('%'),
            "" => {}
            lit => {
                let mut cs = lit.chars();
                let c0 = cs.next().unwrap_or_default();
                if n == 0 && c0.is_alphabetic() {
                    out.push('[');
                    out.extend(c0.to_uppercase());
                    out.extend(c0.to_lowercase());
                    out.push(']');
                    out.push_str(&py_escape(cs.as_str()));
                } else {
                    out.push_str(&py_escape(lit));
                }
            }
        }
    }
    out.push_str(opt_tail);
    out
}

/// `_template_tokens`: a game text's literal words.
fn template_tokens(t: &str) -> Vec<&str> {
    t.split(' ')
        .filter(|w| !w.is_empty() && !w.contains('%'))
        .collect()
}

/// `split_monster`: `fierce kobold thief` to (`fierce`, `kobold thief`) with the known names.
pub fn split_monster(name: &str, known: &HashSet<String>) -> (String, String) {
    let words: Vec<&str> = py_split(name).collect();
    for i in 0..words.len() {
        let base = words[i..].join(" ").to_lowercase();
        if known.contains(&base) {
            return (words[..i].join(" "), base);
        }
    }
    (String::new(), name.to_lowercase())
}

#[derive(Default)]
struct Verbs {
    hit: BTreeSet<String>,
    hit3: BTreeSet<String>,
    miss: BTreeSet<String>,
    miss3: BTreeSet<String>,
}

fn alternation(verbs: &BTreeSet<String>, fallback: &str) -> String {
    if verbs.is_empty() {
        return fallback.to_owned();
    }
    let mut v: Vec<String> = verbs.iter().map(|s| py_escape(s)).collect();
    v.sort_by_key(|s| std::cmp::Reverse(s.chars().count()));
    v.join("|")
}

/// The patterns, their index and the game facts the fields need.
pub struct Registry {
    patterns: Vec<Pattern>,
    /// Pattern indices in rank order: by priority, then specificity, then indexed before
    /// always-tried, then list order.
    by_rank: Vec<u32>,
    index: HashMap<String, Vec<u32>>,
    always: Vec<u32>,
    /// The always-tried patterns as one set, in `always` order: one pass over a line tells
    /// which of them can match, instead of a hundred.
    always_set: OnceLock<Option<RegexSet>>,
    tables: Vec<Pattern>,
    table_by_opener: HashMap<String, usize>,
    room_names: HashSet<String>,
    death_texts: HashMap<String, Vec<String>>,
    monster_names: HashSet<String>,
    pub(crate) join: Vec<(String, JoinRule)>,
    pub(crate) exit_tail: Vec<String>,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("patterns", &self.patterns.len())
            .field("keys", &self.index.len())
            .field("always", &self.always.len())
            .field("room_names", &self.room_names.len())
            .finish()
    }
}

struct Builder {
    patterns: Vec<Option<Pattern>>,
    seen: HashMap<String, usize>,
    death_texts: HashMap<String, Vec<String>>,
}

impl Builder {
    #[allow(clippy::too_many_arguments)]
    fn add_text(
        &mut self,
        text: &str,
        role: &str,
        cat: &str,
        prio: i32,
        num: i32,
        mon: Option<&str>,
    ) {
        let text = text.trim_end_matches(is_py_space);
        if text.chars().count() < 4 {
            return;
        }
        if role == "monster_death"
            && let Some(mon) = mon.filter(|m| !m.is_empty())
        {
            self.death_texts
                .entry(text.to_owned())
                .or_default()
                .push(mon.to_owned());
        }
        if !template_tokens(text).iter().any(|t| t.chars().count() >= 3) {
            return;
        }
        let old = self.seen.get(text).copied();
        if let Some(o) = old
            && self.patterns[o].as_ref().is_some_and(|p| p.prio <= prio)
        {
            return;
        }
        let args = table()
            .roles
            .args
            .get(role)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let mut p = Pattern::new(role, cat, prio, template_regex(text, args));
        p.template = Some(text.to_owned());
        p.example = text.to_owned();
        p.msgnum = Some(num);
        if let Some(o) = old {
            self.patterns[o] = None;
        }
        self.seen.insert(text.to_owned(), self.patterns.len());
        self.patterns.push(Some(p));
    }
}

impl Registry {
    /// The static table only, with no game texts (no generated patterns, no room titles, and
    /// the weapon verbs left to the table's fallbacks).
    pub fn static_only() -> Registry {
        Self::build(None)
    }

    /// The static table plus the patterns generated from `texts`.
    pub fn new(texts: &GameTexts) -> Registry {
        Self::build(Some(texts))
    }

    fn build(texts: Option<&GameTexts>) -> Registry {
        let tbl = table();
        let mut b = Builder {
            patterns: Vec::new(),
            seen: HashMap::new(),
            death_texts: HashMap::new(),
        };
        let mut verbs = Verbs::default();
        let mut monster_names = HashSet::new();
        let mut room_names = HashSet::new();

        if let Some(w) = texts {
            room_names = w.room_names.iter().cloned().collect();
            // a dict: a repeated number keeps its first place and its last texts
            let mut msgs: Vec<(i32, &[String; 3])> = Vec::with_capacity(w.messages.len());
            let mut pos: HashMap<i32, usize> = HashMap::with_capacity(w.messages.len());
            for m in &w.messages {
                match pos.get(&m.number) {
                    Some(&i) => msgs[i].1 = &m.texts,
                    None => {
                        pos.insert(m.number, msgs.len());
                        msgs.push((m.number, &m.texts));
                    }
                }
            }
            let msg = |n: i32| pos.get(&n).map(|&i| msgs[i].1);

            for m in &w.monsters {
                monster_names.insert(m.name.to_lowercase());
                if let Some(a) = msg(m.arrive_msg) {
                    b.add_text(&a[0], "monster_arrives", MONSTER, 55, m.arrive_msg, None);
                    b.add_text(&a[1], "monster_leaves", MONSTER, 55, m.arrive_msg, None);
                    b.add_text(&a[2], "monster_follows", MONSTER, 55, m.arrive_msg, None);
                }
                if let Some(d) = msg(m.death_msg) {
                    b.add_text(
                        &d[2],
                        "monster_death",
                        "combat",
                        55,
                        m.death_msg,
                        Some(&m.name),
                    );
                }
                for at in m.attacks.iter().filter(|a| a.kind != 0) {
                    if let Some(h) = msg(at.hit_msg) {
                        b.add_text(&h[0], "mon_hits_you", "combat", 55, at.hit_msg, None);
                        b.add_text(&h[1], "mon_hits_other", "combat", 56, at.hit_msg, None);
                    }
                    if let Some(d) = msg(at.defend_msg) {
                        b.add_text(&d[0], "mon_deflect_you", "combat", 55, at.defend_msg, None);
                        b.add_text(
                            &d[1],
                            "mon_deflect_other",
                            "combat",
                            56,
                            at.defend_msg,
                            None,
                        );
                        b.add_text(&d[2], "mon_dodge_you", "combat", 55, at.defend_msg, None);
                    }
                    if let Some(s) = msg(at.miss_msg) {
                        b.add_text(&s[0], "mon_dodge_other", "combat", 56, at.miss_msg, None);
                        b.add_text(&s[1], "mon_miss_you", "combat", 55, at.miss_msg, None);
                        b.add_text(&s[2], "mon_miss_other", "combat", 56, at.miss_msg, None);
                    }
                }
            }
            // weapon verbs (struct item hitmsg/missmsg; LJNCOMBT.C:525-560)
            let split = |s: &str, into: &mut BTreeSet<String>| {
                into.extend(
                    s.split('|')
                        .map(py_strip)
                        .filter(|v| !v.is_empty())
                        .map(str::to_owned),
                );
            };
            for it in &w.items {
                if let Some(h) = msg(it.hit_msg) {
                    split(&h[0], &mut verbs.hit);
                    split(&h[2], &mut verbs.hit3);
                }
                if let Some(m) = msg(it.miss_msg) {
                    split(&m[0], &mut verbs.miss);
                    split(&m[2], &mut verbs.miss3);
                }
                if let Some(d) = msg(it.destroy_msg) {
                    b.add_text(
                        &d[0],
                        "item_destroyed",
                        "equipment",
                        58,
                        it.destroy_msg,
                        None,
                    );
                    b.add_text(
                        &d[1],
                        "item_destroyed_other",
                        "equipment",
                        59,
                        it.destroy_msg,
                        None,
                    );
                }
            }
            // fists, kicks, the default swing (LJNCOMBT.C:444-618)
            verbs
                .hit
                .extend(["punch", "kick", "jumpkick", "attack"].map(String::from));
            verbs
                .hit3
                .extend(["punches", "kicks", "jumpkicks", "attacks"].map(String::from));
            verbs.miss.insert("swing at".into());
            verbs.miss3.insert("swings at".into());
            // spell cast texts to caster / target / room (LJNSPELL.C:297-420)
            let you = Regex::new(r"\byou(?:r)?\b").expect("compiles");
            for &n in &w.spell_msgs {
                if let Some(m) = msg(n) {
                    for t in m {
                        let role = if t.starts_with("You ") {
                            "spell_cast_you"
                        } else if you.is_match(t) {
                            "spell_cast_on_you"
                        } else {
                            "spell_cast_room"
                        };
                        b.add_text(t, role, "spell", 57, n, None);
                    }
                }
            }
            // every other game message (spell start and end texts, quests, room actions, ...)
            for (n, m) in &msgs {
                for t in m.iter() {
                    b.add_text(t, "wccmsg", "message", 70, *n, None);
                }
            }
        }

        let hit = alternation(&verbs.hit, r"[a-z]+(?: [a-z]+)?");
        let miss3 = alternation(&verbs.miss3, r"[a-z]+ at");
        let hit3 = alternation(&verbs.hit3, r"[a-z]+(?: [a-z]+)?");
        let miss = alternation(&verbs.miss, r"[a-z]+ at");
        let mut patterns: Vec<Pattern> = b.patterns.into_iter().flatten().collect();
        for s in &tbl.statics {
            let rx =
                s.rx.replace("{HITV}", &hit)
                    .replace("{HITV3}", &hit3)
                    .replace("{MISSV3}", &miss3)
                    .replace("{MISSV}", &miss);
            let mut p = Pattern::new(&s.name, &s.cat, s.prio, rx);
            p.example = s.example.clone();
            p.seen = s.seen.clone();
            patterns.push(p);
        }
        let mut tables = Vec::new();
        let mut table_by_opener = HashMap::new();
        for t in &tbl.tables {
            let mut p = Pattern::new(&t.row_name, &t.cat, 50, t.rx.clone());
            p.max_rows = t.max_rows.unwrap_or(999);
            p.example = t.example.clone();
            table_by_opener.insert(t.opener.clone(), tables.len());
            tables.push(p);
        }

        // keys: each pattern's rarest always-present word (`Registry.build`)
        let words = |p: &Pattern| -> Vec<String> {
            match &p.template {
                Some(t) => {
                    let mut ws = template_tokens(t);
                    if let Some(w0) = ws.first()
                        && t.starts_with(w0)
                        && w0.chars().next().is_some_and(char::is_alphabetic)
                        && ws.len() > 1
                    {
                        ws.remove(0);
                    }
                    ws.into_iter().map(str::to_owned).collect()
                }
                None => regex_tokens(&p.source),
            }
        };
        let pat_words: Vec<Vec<String>> = patterns.iter().map(words).collect();
        let table_words: Vec<Vec<String>> = tables.iter().map(words).collect();
        let mut freq: HashMap<String, u32> = HashMap::new();
        for ws in pat_words.iter().chain(table_words.iter()) {
            let uniq: HashSet<&String> = ws.iter().collect();
            for w in uniq {
                *freq.entry(w.clone()).or_default() += 1;
            }
        }
        for (p, ws) in patterns.iter_mut().zip(&pat_words) {
            p.key = ws
                .iter()
                .filter(|w| w.chars().count() >= 2)
                .min_by_key(|w| (freq[w.as_str()], std::cmp::Reverse(w.chars().count())))
                .cloned();
        }

        let mut order: Vec<u32> = (0..patterns.len() as u32).collect();
        order.sort_by_key(|&i| {
            let p = &patterns[i as usize];
            (p.prio, std::cmp::Reverse(p.spec), p.key.is_none(), i)
        });
        let mut rank = vec![0u32; patterns.len()];
        for (r, &i) in order.iter().enumerate() {
            rank[i as usize] = r as u32;
        }
        let mut index: HashMap<String, Vec<u32>> = HashMap::new();
        let mut always = Vec::new();
        for (i, p) in patterns.iter().enumerate() {
            match &p.key {
                Some(k) => index.entry(k.clone()).or_default().push(rank[i]),
                None => always.push(rank[i]),
            }
        }
        for v in index.values_mut() {
            v.sort_unstable();
        }
        always.sort_unstable();

        Registry {
            patterns,
            by_rank: order,
            index,
            always,
            always_set: OnceLock::new(),
            tables,
            table_by_opener,
            room_names,
            death_texts: b.death_texts,
            monster_names,
            join: tbl
                .join
                .iter()
                .map(|j| {
                    let rule = match j.rule.as_str() {
                        "exits" => JoinRule::Exits,
                        "dot_here" => JoinRule::DotHere,
                        "dot" => JoinRule::Dot,
                        "until_have" => JoinRule::UntilHave,
                        other => panic!("unknown join rule {other}"),
                    };
                    (j.prefix.clone(), rule)
                })
                .collect(),
            exit_tail: tbl.exit_tail.clone(),
        }
    }

    /// Every pattern, in the prototype's list order (generated, then static).
    pub fn patterns(&self) -> &[Pattern] {
        &self.patterns
    }

    /// The table-row patterns.
    pub fn table_rows(&self) -> &[Pattern] {
        &self.tables
    }

    pub(crate) fn table_for(&self, opener: &str) -> Option<usize> {
        self.table_by_opener.get(opener).copied()
    }

    pub(crate) fn table(&self, i: usize) -> &Pattern {
        &self.tables[i]
    }

    pub fn is_room_name(&self, s: &str) -> bool {
        self.room_names.contains(s)
    }

    pub fn room_name_count(&self) -> usize {
        self.room_names.len()
    }

    pub fn key_count(&self) -> usize {
        self.index.len()
    }

    pub fn always_count(&self) -> usize {
        self.always.len()
    }

    pub fn monster_names(&self) -> &HashSet<String> {
        &self.monster_names
    }

    /// The pattern a stripped line matches, and the match (`match_line`): the patterns keyed
    /// by one of the line's words plus the always-tried ones, best first.
    pub fn match_line<'h>(&self, line: &'h str) -> Option<(&Pattern, Captures<'h>)> {
        let mut cands: Vec<u32> = Vec::with_capacity(16);
        for w in line.split(' ') {
            if let Some(v) = self.index.get(w) {
                cands.extend_from_slice(v);
            }
        }
        cands.sort_unstable();
        cands.dedup();
        let always_hits = self.always_set().map(|set| set.matches(line));
        let (mut a, mut b) = (0, 0);
        loop {
            let r = match (cands.get(a), self.always.get(b)) {
                (Some(&x), Some(&y)) if x < y => {
                    a += 1;
                    x
                }
                (Some(&x), None) => {
                    a += 1;
                    x
                }
                (_, Some(&y)) => {
                    b += 1;
                    if always_hits.as_ref().is_some_and(|m| !m.matched(b - 1)) {
                        continue;
                    }
                    y
                }
                (None, None) => return None,
            };
            let p = &self.patterns[self.by_rank[r as usize] as usize];
            if let Some(c) = p.fullmatch(line) {
                return Some((p, c));
            }
        }
    }

    fn always_set(&self) -> Option<&RegexSet> {
        self.always_set
            .get_or_init(|| {
                let pats: Option<Vec<String>> = self
                    .always
                    .iter()
                    .map(|&r| {
                        let p = &self.patterns[self.by_rank[r as usize] as usize];
                        to_rust(&p.source).ok().map(|s| format!("^(?:{s})$"))
                    })
                    .collect();
                RegexSetBuilder::new(pats?).size_limit(1 << 26).build().ok()
            })
            .as_ref()
    }

    /// The fields of a match (`Registry.fields`): the named groups, the monster names a death
    /// text belongs to, the monster name re-split on the known names, and the WCCMSG number.
    pub fn fields(&self, p: &Pattern, c: &Captures) -> Fields {
        let mut d = groups(p, c);
        if p.name == "monster_death" {
            let names: Vec<String> = p
                .template
                .as_ref()
                .and_then(|t| self.death_texts.get(t))
                .map(|v| {
                    v.iter()
                        .cloned()
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect()
                })
                .unwrap_or_default();
            d.insert("monsters".into(), Value::List(names));
        }
        if d.contains_key("mon") && !self.monster_names.is_empty() {
            self.fix_monster(&mut d);
        }
        if let Some(n) = p.msgnum {
            d.insert("msgnum".into(), Value::Int(n.into()));
        }
        d
    }

    /// `_fix_monster`: a lazy `%s` leaves the split between a monster's name and a variable
    /// verb to chance (`big` / `lashworm lunges at`). Re-split on the known names, one leading
    /// adjective allowed: the longest prefix that names a monster wins.
    fn fix_monster(&self, d: &mut Fields) {
        let mon = py_strip(d["mon"].as_str().unwrap_or_default()).to_owned();
        let verb = d
            .get("verb")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let joined = if verb.is_empty() {
            mon.clone()
        } else {
            format!("{mon} {verb}")
        };
        let words: Vec<&str> = py_split(&joined).collect();
        let mut best = None;
        for k in 1..=words.len() {
            let (_, base) = split_monster(&words[..k].join(" "), &self.monster_names);
            if self.monster_names.contains(&base) {
                best = Some(k);
            }
        }
        if let Some(k) = best {
            let m = words[..k].join(" ");
            if !verb.is_empty() {
                d.insert("verb".into(), Value::Text(words[k..].join(" ")));
            }
            let (adj, base) = split_monster(&m, &self.monster_names);
            d.insert("mon".into(), Value::Text(m));
            d.insert("base".into(), Value::Text(base));
            d.insert("adj".into(), Value::Text(adj));
        }
    }
}

/// The named groups that took part in a match.
pub(crate) fn groups(p: &Pattern, c: &Captures) -> Fields {
    let mut d = Fields::new();
    if let Some(rx) = p.regex() {
        for name in rx.capture_names().flatten() {
            if let Some(m) = c.name(name) {
                d.insert(name.to_owned(), Value::Text(m.as_str().to_owned()));
            }
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn templates_become_regexes() {
        assert_eq!(
            template_regex(
                "The %s %slunges at you for %d damage!",
                &args(&["mon", "dmg"])
            ),
            r"[Tt]he\ (?P<mon>.*?)\ (?P<s2>.*?)lunges\ at\ you\ for\ (?P<dmg>-?\d+)\ damage!"
        );
        // a trailing %s is optional; a leading one leaves the first letter alone
        assert_eq!(
            template_regex("%s attacks %s", &[]),
            r"(?P<s1>.*?)\ attacks(?:\s.*)?"
        );
        assert_eq!(template_regex("%s", &[]), r"(?P<s1>.*?)");
        assert_eq!(
            template_regex("100%% sure %d", &[]),
            r"100%\ sure\ (?P<n1>-?\d+)"
        );
        // a repeated argument name is not captured twice
        assert_eq!(
            template_regex("%s hits %s", &args(&["mon", "mon"])),
            r"(?P<mon>.*?)\ hits(?:\s.*)?"
        );
        assert_eq!(
            template_regex("%s and %s ok", &args(&["mon", "mon"])),
            r"(?P<mon>.*?)\ and\ (?:.*?)\ ok"
        );
    }

    #[test]
    fn monster_split() {
        let known: HashSet<String> = ["kobold thief", "kobold", "giant rat"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            split_monster("angry kobold thief", &known),
            ("angry".into(), "kobold thief".into())
        );
        assert_eq!(
            split_monster("Big Giant Rat", &known),
            ("Big".into(), "giant rat".into())
        );
        assert_eq!(split_monster("ghost", &known), ("".into(), "ghost".into()));
    }
}
