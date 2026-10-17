//! The prompt and its statline modes (`patterns.py` `compile_statline`, `PromptParser`).
//!
//! `prf_prompt()` (LJNPRMPT.C:615) prints the prompt with no newline. Statline modes
//! (`set statline`): OFF `>`; ON and FULL `[HP=..]:`, `[HP=../MA=..]:`, `[HP=../KAI=..]:`;
//! CUSTOM and FULL CUSTOM the player's template. Without mana the resting mark goes inside the
//! brackets, with mana after `]:`.

use std::sync::OnceLock;

use regex::Regex;

use crate::event::{Fields, Value};
use crate::pyre::{py_escape, to_rust};
use crate::table::table;

/// The generic form's Python source. Its lookahead (the only one in the prototype) is done by
/// hand: match the bracket body, then check it contains `hp=`, `HP=` or `Hp=`.
const GENERIC_PY: &str = r"\[(?P<body>(?=[^\]\n]*(?:hp|HP|Hp)=)[^\]\n]{1,80})\]:(?P<state> \((?:Resting|Meditating)\) )?";
const GENERIC_RUST: &str = r"^\[(?P<body>[^\]\n]{1,80})\]:(?P<state> \((?:Resting|Meditating)\) )?";

/// Which prompt forms to recognise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptMode {
    /// Statline ON or FULL: `[HP=..]:` (the default; new characters start FULL).
    Std,
    /// Statline OFF: `>`.
    Off,
    /// CUSTOM or FULL CUSTOM, with the template given to `set statline custom`.
    Custom,
    /// The standard form, and a custom template once one is set; without one, any bracketed
    /// `[...hp=...]:` line start.
    Auto,
}

/// What peeling the start of a line found.
#[derive(Clone, Debug, PartialEq)]
pub enum Peeled {
    /// A whole prompt: hp, mana, state and whatever else the form carries.
    Prompt(Fields),
    /// One line of a multi-line custom prompt (`%n`); the fields come with the last line.
    Partial,
}

/// Where a multi-line custom prompt has got to: the next segment and the fields so far.
pub type PromptProgress = Option<(usize, Fields)>;

struct Segment {
    rx: Regex,
    empty: bool,
}

/// Recognises the prompt at the start of a line. Cheap to clone.
#[derive(Clone)]
pub struct PromptParser {
    mode: PromptMode,
    template: Option<String>,
    custom: Option<std::sync::Arc<Vec<Segment>>>,
}

impl std::fmt::Debug for PromptParser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PromptParser")
            .field("mode", &self.mode)
            .field("template", &self.template)
            .finish()
    }
}

fn std_rx() -> &'static Regex {
    static RX: OnceLock<Regex> = OnceLock::new();
    RX.get_or_init(|| {
        let src = to_rust(&table().prompt.std).expect("standard prompt translates");
        Regex::new(&format!("^(?:{src})")).expect("standard prompt compiles")
    })
}

fn generic_rx() -> &'static Regex {
    static RX: OnceLock<Regex> = OnceLock::new();
    RX.get_or_init(|| {
        assert_eq!(
            table().prompt.generic,
            GENERIC_PY,
            "the generic prompt changed in patterns.json"
        );
        Regex::new(GENERIC_RUST).expect("generic prompt compiles")
    })
}

fn kv_rx() -> &'static Regex {
    static RX: OnceLock<Regex> = OnceLock::new();
    RX.get_or_init(|| Regex::new(r"([A-Za-z]+)=(-?\w+)(?:/(\w+))?").expect("compiles"))
}

/// A custom statline template to one Python regex source per output line (`%n` splits),
/// following `substitute_string()` (LJNPRMPT.C:214): `%%` prints two percent signs, unknown
/// codes print as typed, `%f`/`%b` + digit and `%d %B %N %U %L %R` are colours (nothing in
/// plain text). The template is cut to `statline_len` (60) characters.
pub fn compile_statline(template: &str) -> Vec<String> {
    let forms = &table().prompt;
    let t: Vec<char> = template.chars().take(forms.statline_len).collect();
    let mut segs = Vec::new();
    let mut out = String::new();
    let mut used: Vec<String> = Vec::new();
    let mut i = 0;
    while i < t.len() {
        let c = t[i];
        if c != '%' || i + 1 >= t.len() {
            out.push_str(&py_escape(&c.to_string()));
            i += 1;
            continue;
        }
        let k = t[i + 1];
        if (k == 'f' || k == 'b') && i + 2 < t.len() && ('0'..='7').contains(&t[i + 2]) {
            i += 3;
        } else if "fbdBNULR".contains(k) {
            i += 2;
        } else if k == '%' {
            out.push_str("%%");
            i += 2;
        } else if k == 'n' {
            segs.push(std::mem::take(&mut out));
            i += 2;
        } else if let Some(code) = forms.statline_codes.get(&k.to_string()) {
            match &code.field {
                Some(name) if !used.contains(name) => {
                    used.push(name.clone());
                    out.push_str(&format!("(?P<{name}>{})", code.rx));
                }
                _ => out.push_str(&format!("(?:{})", code.rx)),
            }
            i += 2;
        } else if k.is_ascii_digit() {
            out.push_str(&py_escape(&k.to_string()));
            i += 2;
        } else {
            out.push_str(&py_escape(&format!("%{k}")));
            i += 2;
        }
    }
    segs.push(out);
    segs
}

fn int_or_text(s: &str) -> Value {
    s.parse::<i64>()
        .map(Value::Int)
        .unwrap_or_else(|_| Value::Text(s.to_owned()))
}

fn state_value(s: &str) -> Value {
    let st = s.trim_matches(|c| c == ' ' || c == '(' || c == ')');
    if st.is_empty() {
        Value::Null
    } else {
        Value::Text(st.to_owned())
    }
}

/// `str.isdigit()` for the generic form's values.
fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

impl Default for PromptParser {
    fn default() -> Self {
        Self::new(PromptMode::Std)
    }
}

impl PromptParser {
    pub fn new(mode: PromptMode) -> Self {
        PromptParser {
            mode,
            template: None,
            custom: None,
        }
    }

    /// A parser for `set statline [full] custom <template>`.
    pub fn custom(template: &str) -> Result<Self, String> {
        let mut p = Self::new(PromptMode::Custom);
        p.set_custom(template)?;
        Ok(p)
    }

    pub fn mode(&self) -> PromptMode {
        self.mode
    }

    pub fn template(&self) -> Option<&str> {
        self.template.as_deref()
    }

    /// Use a custom template (e.g. after reading `Statusline:` from the SET screen). In `Std`
    /// mode this switches to `Custom`, as in the prototype.
    pub fn set_custom(&mut self, template: &str) -> Result<(), String> {
        let segs = compile_statline(template)
            .into_iter()
            .map(|py| {
                let rx =
                    Regex::new(&format!("^(?:{})", to_rust(&py)?)).map_err(|e| e.to_string())?;
                Ok(Segment {
                    rx,
                    empty: py.is_empty(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        self.template = Some(template.to_owned());
        self.custom = Some(std::sync::Arc::new(segs));
        if self.mode == PromptMode::Std {
            self.mode = PromptMode::Custom;
        }
        Ok(())
    }

    /// Peel a prompt off the start of `line`: what was found and how many bytes it took.
    /// `progress` carries a multi-line custom prompt from one line to the next.
    pub fn peel(&self, progress: &mut PromptProgress, line: &str) -> Option<(Peeled, usize)> {
        if matches!(self.mode, PromptMode::Std | PromptMode::Auto)
            && let Some(c) = std_rx().captures(line)
        {
            let mut d = Fields::new();
            let mut state = Value::Null;
            for name in ["hp", "mtype", "mana", "energy", "dbg"] {
                if let Some(m) = c.name(name) {
                    let v = if name == "mtype" || name == "dbg" {
                        Value::from(m.as_str())
                    } else {
                        int_or_text(m.as_str())
                    };
                    d.insert(name.to_owned(), v);
                }
            }
            if let Some(m) = c.name("state1").or_else(|| c.name("state2")) {
                state = state_value(m.as_str());
            }
            d.insert("state".into(), state);
            d.insert("form".into(), "std".into());
            return Some((Peeled::Prompt(d), c.get(0).map_or(0, |m| m.end())));
        }
        if matches!(self.mode, PromptMode::Custom | PromptMode::Auto)
            && let Some(segs) = &self.custom
        {
            let k = progress.as_ref().map_or(0, |p| p.0);
            if let Some(c) = segs[k].rx.captures(line) {
                let end = c.get(0).map_or(0, |m| m.end());
                if end > 0 || segs[k].empty {
                    let mut d = progress.take().map(|p| p.1).unwrap_or_default();
                    for name in segs[k].rx.capture_names().flatten() {
                        if let Some(m) = c.name(name) {
                            d.insert(name.to_owned(), Value::from(m.as_str()));
                        }
                    }
                    if k + 1 < segs.len() {
                        *progress = Some((k + 1, d));
                        return Some((Peeled::Partial, end));
                    }
                    for f in [
                        "hp", "maxhp", "mana", "maxmana", "exp", "exp_left", "wealth",
                    ] {
                        if let Some(Value::Text(s)) = d.get(f) {
                            let v = int_or_text(s);
                            d.insert(f.to_owned(), v);
                        }
                    }
                    if let Some(Value::Text(s)) = d.get("state") {
                        let v = state_value(s);
                        d.insert("state".into(), v);
                    }
                    d.insert("form".into(), "custom".into());
                    return Some((Peeled::Prompt(d), end));
                }
            }
            *progress = None;
        }
        if self.mode == PromptMode::Auto
            && self.custom.is_none()
            && let Some(c) = generic_rx().captures(line)
        {
            let body = c.name("body").map_or("", |m| m.as_str());
            if body.contains("hp=") || body.contains("HP=") || body.contains("Hp=") {
                let mut d = Fields::new();
                d.insert("form".into(), "generic".into());
                d.insert(
                    "state".into(),
                    c.name("state")
                        .map_or(Value::Null, |m| state_value(m.as_str())),
                );
                for kv in kv_rx().captures_iter(body) {
                    let key = kv[1].to_lowercase();
                    let key = match key.as_str() {
                        "ma" | "kai" => "mana".to_owned(),
                        "cash" => "wealth".to_owned(),
                        _ => key,
                    };
                    let v = &kv[2];
                    if is_digits(v.trim_start_matches('-'))
                        && let Ok(n) = v.parse::<i64>()
                    {
                        d.insert(key.clone(), Value::Int(n));
                    }
                    if let Some(mx) = kv.get(3).map(|m| m.as_str())
                        && is_digits(mx)
                        && let Ok(n) = mx.parse::<i64>()
                    {
                        d.insert(format!("max{key}"), Value::Int(n));
                    }
                }
                return Some((Peeled::Prompt(d), c.get(0).map_or(0, |m| m.end())));
            }
        }
        if self.mode == PromptMode::Off && line.starts_with('>') {
            let mut d = Fields::new();
            d.insert("form".into(), "off".into());
            return Some((Peeled::Prompt(d), 1));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peel_one(p: &PromptParser, line: &str) -> Option<(Peeled, usize)> {
        p.peel(&mut None, line)
    }

    fn fields(pairs: &[(&str, Value)]) -> Peeled {
        Peeled::Prompt(
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        )
    }

    #[test]
    fn standard_forms() {
        let p = PromptParser::default();
        assert_eq!(
            peel_one(&p, "[HP=48]:Slum Street"),
            Some((
                fields(&[
                    ("hp", Value::Int(48)),
                    ("state", Value::Null),
                    ("form", "std".into())
                ]),
                8
            ))
        );
        let (got, n) = peel_one(&p, "[HP=12/MA=3]: (Resting) ").unwrap();
        assert_eq!(n, 24);
        assert_eq!(
            got,
            fields(&[
                ("hp", Value::Int(12)),
                ("mtype", "MA".into()),
                ("mana", Value::Int(3)),
                ("state", "Resting".into()),
                ("form", "std".into())
            ])
        );
        let (got, _) = peel_one(&p, "[HP=-4 (Meditating) ]:").unwrap();
        assert_eq!(
            got,
            fields(&[
                ("hp", Value::Int(-4)),
                ("state", "Meditating".into()),
                ("form", "std".into())
            ])
        );
        let (got, _) = peel_one(&p, "[HP=40/KAI=2]:{Energy=900 dbg x}:").unwrap();
        assert!(
            matches!(got, Peeled::Prompt(ref d) if d["energy"] == Value::Int(900) && d["dbg"] == "dbg x".into())
        );
        assert_eq!(peel_one(&p, "Slum Street"), None);
    }

    #[test]
    fn off_form() {
        let p = PromptParser::new(PromptMode::Off);
        assert_eq!(
            peel_one(&p, ">look"),
            Some((fields(&[("form", "off".into())]), 1))
        );
    }

    #[test]
    fn statline_codes() {
        assert_eq!(
            compile_statline("[%h/%H %m/%M]%r:"),
            [
                r"\[(?P<hp>-?\d+)/(?P<maxhp>-?\d+)\ (?P<mana>-?\d+)/(?P<maxmana>-?\d+)\](?P<state>(?: \((?:Resting|Meditating)\) )?):"
            ]
        );
        // colours vanish, %% prints two percent signs, %n splits, unknown codes print as typed
        assert_eq!(
            compile_statline("%f3%h%%%n%q%5%fh"),
            [r"(?P<hp>-?\d+)%%", r"%q5h"]
        );
        assert_eq!(compile_statline(&"x".repeat(70)).concat().len(), 60);
    }

    #[test]
    fn custom_forms() {
        let p = PromptParser::custom("[%h/%H %m/%M]%r:").unwrap();
        let (got, n) = peel_one(&p, "[30/56 7/9] (Resting) :n").unwrap();
        assert_eq!(n, 23);
        assert_eq!(
            got,
            fields(&[
                ("hp", Value::Int(30)),
                ("maxhp", Value::Int(56)),
                ("mana", Value::Int(7)),
                ("maxmana", Value::Int(9)),
                ("state", "Resting".into()),
                ("form", "custom".into())
            ])
        );
        let (got, _) = peel_one(&p, "[30/56 7/9]:").unwrap();
        assert!(matches!(got, Peeled::Prompt(ref d) if d["state"] == Value::Null));
        assert_eq!(
            peel_one(&p, "[HP=30]:"),
            None,
            "custom mode does not take the standard form"
        );

        // a two-line prompt reports on its last line
        let p = PromptParser::custom("%i %c%nHP %h:").unwrap();
        let mut progress = None;
        assert_eq!(
            p.peel(&mut progress, "Kasimir 1200"),
            Some((Peeled::Partial, 12))
        );
        let (got, _) = p.peel(&mut progress, "HP 30:").unwrap();
        assert_eq!(
            got,
            fields(&[
                ("name", "Kasimir".into()),
                ("wealth", Value::Int(1200)),
                ("hp", Value::Int(30)),
                ("form", "custom".into())
            ])
        );
        assert_eq!(progress, None);
    }

    #[test]
    fn generic_form() {
        let p = PromptParser::new(PromptMode::Auto);
        let (got, _) = peel_one(&p, "[hp=1/2,MA=8/42,CASH=180750,WARNS=Off]:").unwrap();
        assert_eq!(
            got,
            fields(&[
                ("form", "generic".into()),
                ("state", Value::Null),
                ("hp", Value::Int(1)),
                ("maxhp", Value::Int(2)),
                ("mana", Value::Int(8)),
                ("maxmana", Value::Int(42)),
                ("wealth", Value::Int(180750))
            ])
        );
        // the standard form wins where both fit
        let (got, _) = peel_one(&p, "[HP=395/MA=392]:").unwrap();
        assert!(matches!(got, Peeled::Prompt(ref d) if d["form"] == "std".into()));
        let (got, _) = peel_one(&p, "[HP=395/395,MA=392/433]: (Resting) ").unwrap();
        assert!(
            matches!(got, Peeled::Prompt(ref d) if d["form"] == "generic".into() && d["state"] == "Resting".into())
        );
        // the lookahead: the body must carry hp= (in one of three spellings)
        assert_eq!(peel_one(&p, "[MA=3]:"), None);
        assert_eq!(peel_one(&p, "[hP=3]:"), None);
        assert!(peel_one(&p, "[x Hp=3]:").is_some());
        // once a template is set, auto mode stops taking the generic form
        let mut p = PromptParser::new(PromptMode::Auto);
        p.set_custom("<%h>").unwrap();
        assert_eq!(p.mode(), PromptMode::Auto);
        assert!(peel_one(&p, "[hp=1/2]:").is_none());
        assert!(peel_one(&p, "<5>").is_some());
        assert!(peel_one(&p, "[HP=5]:").is_some());
    }
}
