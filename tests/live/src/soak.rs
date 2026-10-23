//! The soak scenario (C-033): a character runs a loop for N minutes and the numbers come out.
//! The arithmetic of the summary is here, with its unit tests; the run itself is live only.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::gateway::Gateway;
use crate::{Ctx, bot_field, flee_from_args, placed_brain, stop_brain, walk_out_to};

/// What `--scenario soak` was asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoakArgs {
    pub minutes: u64,
    /// `MAP:ROOM`; the believed room when absent.
    pub anchor: Option<(i64, i64)>,
    pub radius: u32,
    /// Stops the loop passes by (`--skip MAP:ROOM`, repeatable).
    pub skip: Vec<(i64, i64)>,
    /// A generated loop's id or a MegaMud loop's file (`Mwodloop.mp`); the first generated
    /// loop listed when absent.
    pub loop_id: Option<String>,
    /// Save the loop as a custom loop of this name first (`--save-as`), with the stops of
    /// `add_stops` added, and run that.
    pub save_as: Option<String>,
    /// `--add-stop MAP:ROOM[@N]`: a room added after the loop's stop N (counted in the loop as
    /// listed; 0 puts it first); at the end when there is no `@N`.
    pub add_stops: Vec<((i64, i64), Option<usize>)>,
}

impl Default for SoakArgs {
    fn default() -> Self {
        Self {
            minutes: 0,
            anchor: None,
            radius: 8,
            skip: Vec::new(),
            loop_id: None,
            save_as: None,
            add_stops: Vec::new(),
        }
    }
}

/// A pool character may not soak for longer than this: the pool is for the short scenarios.
pub const POOL_MAX_MINUTES: u64 = 10;

/// `MAP:ROOM`.
pub fn parse_room(text: &str) -> Option<(i64, i64)> {
    let (map, room) = text.split_once(':')?;
    Some((map.trim().parse().ok()?, room.trim().parse().ok()?))
}

/// `MAP:ROOM` or `MAP:ROOM@N`: a room to add after the loop's stop N (1-based, as listed; 0
/// puts it first).
pub fn parse_add_stop(text: &str) -> Option<((i64, i64), Option<usize>)> {
    match text.split_once('@') {
        Some((room, after)) => Some((parse_room(room)?, Some(after.trim().parse().ok()?))),
        None => Some((parse_room(text)?, None)),
    }
}

/// `base` with the rooms of `adds` put in: each after the stop it names (counted in `base`, so
/// the numbers do not shift as others are added), several after the same stop in the order
/// given, a number past the end or none at the end.
pub fn insert_stops(base: &[(i64, i64)], adds: &[((i64, i64), Option<usize>)]) -> Vec<(i64, i64)> {
    let after = |n: usize| {
        adds.iter()
            .filter(move |(_, a)| *a == Some(n))
            .map(|&(k, _)| k)
    };
    let mut out: Vec<(i64, i64)> = after(0).collect();
    for (i, &stop) in base.iter().enumerate() {
        out.push(stop);
        out.extend(after(i + 1));
    }
    out.extend(
        adds.iter()
            .filter(|(_, a)| a.is_none_or(|n| n > base.len()))
            .map(|&(k, _)| k),
    );
    out
}

/// How long the session has gone without a prompt, the longest stretch so far.
#[derive(Debug, Clone, Copy)]
pub struct PromptGaps {
    last: Instant,
    longest: Duration,
}

impl PromptGaps {
    pub fn new(now: Instant) -> Self {
        Self {
            last: now,
            longest: Duration::ZERO,
        }
    }

    /// A prompt came at `now`.
    pub fn prompt(&mut self, now: Instant) {
        self.longest = self.longest.max(now.saturating_duration_since(self.last));
        self.last = now;
    }

    /// The longest gap up to `now`, the one still open included.
    pub fn longest(&self, now: Instant) -> Duration {
        self.longest.max(now.saturating_duration_since(self.last))
    }

    /// Forget the gaps before `now` (the run starts).
    pub fn restart(&mut self, now: Instant) {
        *self = Self::new(now);
    }
}

/// Exp per hour over `secs`, the brain's one-minute floor applied.
pub fn exp_per_hour(exp: i64, secs: u64) -> i64 {
    exp * 3600 / secs.max(60) as i64
}

/// Unclassified lines as a percentage of lines.
pub fn unclassified_percent(unclassified: u64, lines: u64) -> f64 {
    if lines == 0 {
        0.0
    } else {
        unclassified as f64 * 100.0 / lines as f64
    }
}

/// Kills by monster from `monster_death` events' `monsters` fields. One death text can belong
/// to several monsters (the line does not say which), so they count under their joined names.
pub fn kills_by_monster<'a>(
    deaths: impl IntoIterator<Item = &'a serde_json::Map<String, serde_json::Value>>,
) -> BTreeMap<String, u64> {
    let mut kills = BTreeMap::new();
    for fields in deaths {
        let name = match fields.get("monsters") {
            Some(serde_json::Value::Array(list)) => list
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
                .join(" / "),
            Some(serde_json::Value::String(s)) => s.clone(),
            _ => String::new(),
        };
        let name = if name.is_empty() {
            "(unnamed)".to_owned()
        } else {
            name
        };
        *kills.entry(name).or_insert(0) += 1;
    }
    kills
}

/// How many of the brain's notices were "No exit `...`: looking to relocate".
pub fn relocations<'a>(notices: impl IntoIterator<Item = &'a str>) -> usize {
    notices
        .into_iter()
        .filter(|n| n.starts_with("No exit `"))
        .count()
}

/// The numbers a soak prints.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tally {
    /// The loop run, with its kind and id: "Wooden Passages (MegaMud, Mwodloop.mp)".
    pub loop_name: String,
    pub minutes: u64,
    pub exp_gained: i64,
    pub exp_per_hour: i64,
    pub kills: u64,
    pub kills_by_monster: BTreeMap<String, u64>,
    pub deaths: u64,
    pub flees: u64,
    pub rests: u64,
    pub relocations: usize,
    pub unclassified: u64,
    pub lines: u64,
    pub longest_gap: Duration,
    pub brain_restarts: u32,
    /// The run as the store holds it (its id, or its start when the id can't be read).
    pub run: String,
    /// Why the run ended before its time, if it did.
    pub ended_early: Option<String>,
}

/// The summary table, in markdown.
pub fn table(t: &Tally) -> String {
    let by_monster = if t.kills_by_monster.is_empty() {
        "none".to_owned()
    } else {
        t.kills_by_monster
            .iter()
            .map(|(m, n)| format!("{m} {n}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let rows = [
        ("Loop", t.loop_name.clone()),
        ("Minutes", t.minutes.to_string()),
        ("Exp gained", t.exp_gained.to_string()),
        ("Exp per hour", t.exp_per_hour.to_string()),
        ("Kills", format!("{} ({by_monster})", t.kills)),
        ("Deaths", t.deaths.to_string()),
        ("Flees", t.flees.to_string()),
        ("Rests", t.rests.to_string()),
        ("Relocations", t.relocations.to_string()),
        (
            "Unclassified lines",
            format!(
                "{} of {} ({:.2}%)",
                t.unclassified,
                t.lines,
                unclassified_percent(t.unclassified, t.lines)
            ),
        ),
        (
            "Longest gap without a prompt",
            format!("{:.1} s", t.longest_gap.as_secs_f64()),
        ),
        ("Brain restarts", t.brain_restarts.to_string()),
        ("Run in the store", t.run.clone()),
    ];
    let mut out = String::from("| Soak | |\n|---|---|\n");
    for (name, value) in rows {
        out.push_str(&format!("| {name} | {} |\n", value.replace('|', "\\|")));
    }
    if let Some(why) = &t.ended_early {
        out.push_str(&format!("\nThe run ended early: {why}\n"));
    }
    out
}

/// The tick between the progress lines.
const TICK: Duration = Duration::from_secs(60);
/// How long Flee and Recover may take to finish before the bot is stopped.
const SETTLE: Duration = Duration::from_secs(180);

/// The soak, with the gateway in the realm: a brain; the loops around the anchor; `run_loop`
/// for the minutes asked, a line a minute; Stop (after Recover or Flee if one is going), a
/// walk to the anchor, the brain stopped; the numbers in `ctx.soak`. Whether it all went.
pub async fn scenario(ctx: &mut Ctx, gw: &mut Gateway) -> bool {
    let soak = ctx.args.soak.clone();
    let extra = flee_from_args(&ctx.args.flee_from);
    let Some((running, start)) = placed_brain(ctx, gw, "soak", &extra).await else {
        return false;
    };
    let dir = running.dir().to_path_buf();
    let anchor = soak.anchor.unwrap_or(start);

    // The loop: --loop, or the first generated one around the anchor.
    let started = Instant::now();
    let picked = pick_loop(gw, anchor, soak.radius, soak.loop_id.as_deref()).await;
    ctx.record(
        &format!("loops_request around {}/{}", anchor.0, anchor.1),
        started,
        &picked
            .as_ref()
            .map(|(_, _, s)| s.clone())
            .map_err(Clone::clone),
    );
    // `--save-as`: the picked loop with the rooms of `--add-stop` put in, saved under a name,
    // and that is what runs.
    let picked = match (picked, &soak.save_as) {
        (Ok((base, _, note)), Some(name)) => {
            let started = Instant::now();
            let saved = save_custom(gw, &base, name, &soak.add_stops).await;
            ctx.record(
                &format!("save_loop {name:?}"),
                started,
                &saved.as_ref().map(|(_, n)| n.clone()).map_err(Clone::clone),
            );
            saved.map(|(id, n)| (id, name.clone(), format!("{note}; {n}")))
        }
        (picked, _) => picked,
    };
    let mut ok = picked.is_ok();
    let mut tally = Tally::default();
    let mut ended_early = None;
    if let Ok((id, label, _)) = picked {
        // Run it.
        let from = (gw.notices.len(), gw.warnings.len(), gw.events.len());
        let parse_from = parse_counts(&gw.character);
        gw.gaps.restart(Instant::now());
        let started = Instant::now();
        let ran = run(gw, &id, &soak, from.1).await;
        let (result, early) = match ran {
            Ok(note) => (Ok(note), None),
            Err(why) => (Err(why.clone()), Some(why)),
        };
        ctx.record(
            &format!("run_loop for {} minutes", soak.minutes),
            started,
            &result,
        );
        ok &= result.is_ok();
        ended_early = early;
        let dead = ended_early.as_ref().is_some_and(|w| w.starts_with("died"));

        // Let Flee or Recover finish, then Stop.
        if !dead {
            let _ = gw
                .wait_for(SETTLE, "Flee and Recover over", |g| {
                    (!matches!(bot_field(&g.character, "mode"), Some("flee" | "recover")))
                        .then_some(())
                })
                .await;
        }
        let ended = parse_counts(&gw.character);
        let run_secs = started.elapsed().as_secs();
        let longest_gap = gw.gaps.longest(Instant::now());
        let started = Instant::now();
        let stopped = crate::turn_off(gw).await;
        ctx.record("stop the bot", started, &stopped);
        ok &= stopped.is_ok();

        // The numbers: the store's run (ended now) for the counts, the stream for the rest.
        tally = collect(gw, from, (parse_from, ended), (longest_gap, run_secs), &dir).await;
        tally.ended_early = ended_early.clone();
        tally.loop_name = label;
        save_streams(gw, from, &dir);

        // Back to the anchor.
        if !dead {
            ok &= walk_out_to(ctx, gw, anchor).await;
        }
    }
    ctx.soak = Some(table(&tally));
    stop_brain(ctx, running).await && ok && ended_early.is_none()
}

/// `loops_request` around `anchor` and the loop to run: `wanted` by a generated loop's id or
/// a MegaMud loop's file, else the first generated; its id, its name and kind, and a note on
/// what was listed.
async fn pick_loop(
    gw: &mut Gateway,
    anchor: (i64, i64),
    radius: u32,
    wanted: Option<&str>,
) -> Result<(String, String, String), String> {
    gw.loops_request(anchor, radius).await?;
    let warnings = gw.warnings.len();
    gw.wait_for(Duration::from_secs(15), "the loops", |g| {
        if let Some(w) = g.warnings[warnings..].first() {
            return Some(Err(w.clone()));
        }
        let wayfarer_proto::ToClient::Loops {
            generated, megamud, ..
        } = g.loops.as_ref()?
        else {
            return None;
        };
        let listed = generated
            .iter()
            .map(|l| {
                format!(
                    "{} ({}, {} stops, {} steps)",
                    l.name,
                    l.id,
                    l.stops.len(),
                    l.steps
                )
            })
            .chain(megamud.iter().map(|m| {
                let run = m.reason.as_deref().map_or_else(
                    || format!("{} stops", m.stops.len()),
                    |why| format!("not runnable: {why}"),
                );
                format!("{} (MegaMud {}, {run})", m.description, m.file)
            }))
            .collect::<Vec<_>>()
            .join("; ");
        let generated_one = |l: &wayfarer_proto::GeneratedLoop| {
            Ok((l.id.clone(), format!("{} (generated, {})", l.name, l.id)))
        };
        let chosen = match wanted {
            Some(id) => generated
                .iter()
                .find(|l| l.id == id)
                .map(generated_one)
                .or_else(|| {
                    let m = megamud.iter().find(|m| m.file.eq_ignore_ascii_case(id))?;
                    let name = if m.description.trim().is_empty() {
                        &m.file
                    } else {
                        &m.description
                    };
                    Some(match &m.reason {
                        None => Ok((m.file.clone(), format!("{name} (MegaMud, {})", m.file))),
                        Some(why) => Err(format!("MegaMud loop {} is not runnable: {why}", m.file)),
                    })
                }),
            None => generated.first().map(generated_one),
        };
        Some(match chosen {
            Some(Ok((id, label))) => {
                let note = format!("running {label}; listed {listed}");
                Ok((id, label, note))
            }
            Some(Err(why)) => Err(format!("{why}; listed {listed}")),
            None => Err(format!("no such loop ({wanted:?}); listed {listed:?}")),
        })
    })
    .await?
}

/// The stops of loop `id` in the last `loops` message, a generated loop's or a MegaMud loop's.
fn base_stops(gw: &Gateway, id: &str) -> Option<Vec<(i64, i64)>> {
    let wayfarer_proto::ToClient::Loops {
        generated, megamud, ..
    } = gw.loops.as_ref()?
    else {
        return None;
    };
    let stops = generated
        .iter()
        .find(|l| l.id == id)
        .map(|l| &l.stops)
        .or_else(|| {
            megamud
                .iter()
                .find(|m| m.file.eq_ignore_ascii_case(id))
                .map(|m| &m.stops)
        })?;
    Some(stops.iter().map(|s| (s.map, s.room)).collect())
}

/// Save a custom loop `name` from loop `base` with `adds` put in, and the id the brain made
/// for it (the end of its notice: "Saved loop X (N stops): custom:x") with that notice.
async fn save_custom(
    gw: &mut Gateway,
    base: &str,
    name: &str,
    adds: &[((i64, i64), Option<usize>)],
) -> Result<(String, String), String> {
    let stops = insert_stops(
        &base_stops(gw, base).ok_or_else(|| format!("no stops listed for {base}"))?,
        adds,
    );
    let (notices, warnings) = (gw.notices.len(), gw.warnings.len());
    gw.save_loop(name, Some(base), &stops).await?;
    let note = gw
        .wait_for(Duration::from_secs(10), "the loop saved", |g| {
            if let Some(w) = g.warnings[warnings..].first() {
                return Some(Err(w.clone()));
            }
            g.notices[notices..]
                .iter()
                .find(|n| n.starts_with("Saved loop") || n.starts_with("Changed loop"))
                .map(|n| Ok(n.clone()))
        })
        .await??;
    let id = note
        .rsplit(": ")
        .next()
        .filter(|id| id.starts_with("custom:"))
        .ok_or_else(|| format!("no loop id in {note:?}"))?
        .to_owned();
    Ok((id, note))
}

/// `(lines, unclassified)` from the snapshot's parse counts.
fn parse_counts(c: &serde_json::Value) -> (u64, u64) {
    let n = |k: &str| c["parse"][k].as_u64().unwrap_or(0);
    (n("lines"), n("unclassified"))
}

/// `run_loop` and watch for `soak.minutes`, a line a minute. Ok when the time is up with the
/// bot still running; an error saying why when it died or stopped itself first.
async fn run(gw: &mut Gateway, id: &str, soak: &SoakArgs, warned: usize) -> Result<String, String> {
    gw.run_loop(id, &soak.skip).await?;
    gw.wait_for(Duration::from_secs(15), "the bot running", |g| {
        (bot_field(&g.character, "state") == Some("running")).then_some(())
    })
    .await?;
    let begun = Instant::now();
    let end = begun + Duration::from_secs(soak.minutes * 60);
    let mut minute = 0;
    loop {
        let tick = (begun + TICK * (minute + 1)).min(end);
        let waited = Instant::now();
        let over = gw
            .wait_for(tick.saturating_duration_since(waited), "a minute", |g| {
                ended(g, warned)
            })
            .await;
        if let Ok(why) = over {
            println!("{}", progress(gw, begun));
            return Err(why);
        }
        if waited.elapsed() < Duration::from_millis(500) && Instant::now() < tick {
            return Err("the gateway connection ended".to_owned());
        }
        minute += 1;
        println!("{}", progress(gw, begun));
        if Instant::now() >= end {
            return Ok(format!("{} minutes, the bot still running", soak.minutes));
        }
    }
}

/// Why the run is over, if it is: a death, or the bot stopped itself.
fn ended(g: &Gateway, warned: usize) -> Option<String> {
    let run = &g.character["run"];
    if let Some(w) = g.warnings[warned..].iter().find(|w| w.contains("killed")) {
        return Some(format!("died: {w}"));
    }
    if run["deaths"].as_u64().unwrap_or(0) > 0 {
        return Some("died: the run counts a death".to_owned());
    }
    if bot_field(&g.character, "state") == Some("stopped") {
        let why = bot_field(&g.character, "reason")
            .filter(|r| !r.is_empty())
            .or_else(|| bot_field(&g.character, "narration"))
            .unwrap_or("no reason given");
        return Some(format!("the bot stopped itself: {why}"));
    }
    None
}

/// The minute's line: time, room, HP, exp gained, kills, flees, deaths, narration.
fn progress(g: &Gateway, begun: Instant) -> String {
    let run = &g.character["run"];
    let room = g.character["room_title"].as_str().unwrap_or("?");
    let hp = crate::hp(&g.character)
        .map(|(h, m)| format!("{h}/{m}"))
        .unwrap_or_else(|| "?".to_owned());
    format!(
        "[{:>3}:{:02}] {room} | HP {hp} | exp +{} | kills {} | flees {} | deaths {} | {}",
        begun.elapsed().as_secs() / 60,
        begun.elapsed().as_secs() % 60,
        run["exp_gained"],
        run["kills"],
        run["flees"],
        run["deaths"],
        bot_field(&g.character, "narration").unwrap_or(""),
    )
}

/// The run's numbers once the bot is stopped: `stats_request` for the store's counts (the
/// newest run, ended), the event and notice streams since `from` for the rest.
async fn collect(
    gw: &mut Gateway,
    from: (usize, usize, usize),
    parse: ((u64, u64), (u64, u64)),
    (longest_gap, run_secs): (Duration, u64),
    dir: &std::path::Path,
) -> Tally {
    let mut t = Tally {
        longest_gap,
        brain_restarts: gw.brain_drops,
        relocations: relocations(gw.notices[from.0..].iter().map(String::as_str)),
        kills_by_monster: kills_by_monster(
            gw.events[from.2..]
                .iter()
                .filter(|e| e.name == "monster_death")
                .map(|e| &e.fields),
        ),
        lines: parse.1.0.saturating_sub(parse.0.0),
        unclassified: parse.1.1.saturating_sub(parse.0.1),
        ..Tally::default()
    };
    let _ = gw.stats_request().await;
    let stats = gw
        .wait_for(Duration::from_secs(10), "the stats", |g| g.stats.clone())
        .await;
    if let Ok(wayfarer_proto::ToClient::Stats { runs, .. }) = stats
        && let Some(r) = runs.first()
    {
        t.minutes = r.minutes;
        t.exp_gained = r.exp_gained;
        t.exp_per_hour = exp_per_hour(r.exp_gained, run_secs);
        t.kills = r.kills;
        t.deaths = r.deaths;
        t.flees = r.flees;
        t.rests = r.rests;
        t.run = run_id(dir).unwrap_or_else(|| format!("started {}", r.started_at));
    }
    t
}

/// The newest run's id in the brain's store, by the `sqlite3` tool if there is one.
fn run_id(dir: &std::path::Path) -> Option<String> {
    let out = std::process::Command::new("sqlite3")
        .arg(dir.join("store.sqlite"))
        .arg("SELECT id FROM runs ORDER BY id DESC LIMIT 1")
        .output()
        .ok()?;
    let id = String::from_utf8(out.stdout).ok()?;
    let id = id.trim();
    (out.status.success() && !id.is_empty())
        .then(|| format!("{id} ({})", dir.join("store.sqlite").display()))
}

/// The full event and notice streams since `from`, as `events.log` and `notices.log` in `dir`,
/// each line with its seconds into the run.
fn save_streams(gw: &Gateway, from: (usize, usize, usize), dir: &std::path::Path) {
    let begun = gw
        .notice_at
        .get(from.0)
        .copied()
        .unwrap_or_else(Instant::now);
    let secs = |at: Instant| at.saturating_duration_since(begun).as_secs_f64();
    let events: String = gw.events[from.2..]
        .iter()
        .map(|e| format!("{:>8.1} {} {}\n", secs(e.at), e.name, e.line.trim_end()))
        .collect();
    let notices: String = gw.notices[from.0..]
        .iter()
        .zip(&gw.notice_at[from.0..])
        .map(|(n, at)| format!("{:>8.1} {n}\n", secs(*at)))
        .collect();
    let _ = std::fs::write(dir.join("events.log"), events);
    let _ = std::fs::write(dir.join("notices.log"), notices);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fields(v: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn a_room_to_add_names_the_stop_it_goes_after() {
        assert_eq!(parse_add_stop("9:870"), Some(((9, 870), None)));
        assert_eq!(parse_add_stop("9:870@32"), Some(((9, 870), Some(32))));
        assert_eq!(parse_add_stop(" 9:870 @ 0 "), Some(((9, 870), Some(0))));
        assert_eq!(parse_add_stop("9:870@x"), None);
        assert_eq!(parse_add_stop("870@3"), None);
    }

    #[test]
    fn rooms_are_put_in_after_the_stops_they_name() {
        let base = [(1, 1), (1, 2), (1, 3)];
        let x = (9, 870);
        let y = (9, 871);
        assert_eq!(insert_stops(&base, &[]), base);
        assert_eq!(
            insert_stops(&base, &[(x, Some(2))]),
            [(1, 1), (1, 2), x, (1, 3)]
        );
        assert_eq!(
            insert_stops(&base, &[(x, Some(0))]),
            [x, (1, 1), (1, 2), (1, 3)]
        );
        assert_eq!(
            insert_stops(&base, &[(x, None)]),
            [(1, 1), (1, 2), (1, 3), x]
        );
        assert_eq!(
            insert_stops(&base, &[(x, Some(99))]),
            [(1, 1), (1, 2), (1, 3), x]
        );
        // Counted in the loop as listed, so adding one does not shift the other; two after the
        // same stop keep the order given.
        assert_eq!(
            insert_stops(&base, &[(x, Some(1)), (y, Some(1)), ((9, 872), Some(3))]),
            [(1, 1), x, y, (1, 2), (1, 3), (9, 872)]
        );
    }

    #[test]
    fn rooms_parse_as_map_and_room() {
        assert_eq!(parse_room("1:2140"), Some((1, 2140)));
        assert_eq!(parse_room(" 1 : 1865 "), Some((1, 1865)));
        assert_eq!(parse_room("2140"), None);
        assert_eq!(parse_room("a:b"), None);
    }

    #[test]
    fn exp_per_hour_scales_the_run_to_an_hour_with_a_minute_floor() {
        assert_eq!(exp_per_hour(500, 1800), 1000);
        assert_eq!(exp_per_hour(100, 180), 2000);
        assert_eq!(exp_per_hour(10, 6), 600);
        assert_eq!(exp_per_hour(0, 3600), 0);
    }

    #[test]
    fn the_unclassified_rate_is_a_percentage_and_zero_without_lines() {
        assert_eq!(unclassified_percent(0, 0), 0.0);
        assert!((unclassified_percent(2, 1000) - 0.2).abs() < 1e-9);
    }

    #[test]
    fn kills_are_counted_by_the_monsters_a_death_text_names() {
        let a = fields(json!({"monsters": ["large rat"]}));
        let b = fields(json!({"monsters": ["large rat"]}));
        let c = fields(json!({"monsters": ["cave bear", "grizzly bear"]}));
        let d = fields(json!({"monsters": "kobold"}));
        let e = fields(json!({}));
        let kills = kills_by_monster([&a, &b, &c, &d, &e]);
        assert_eq!(kills["large rat"], 2);
        assert_eq!(kills["cave bear / grizzly bear"], 1);
        assert_eq!(kills["kobold"], 1);
        assert_eq!(kills["(unnamed)"], 1);
    }

    #[test]
    fn only_no_exit_notices_count_as_relocations() {
        let said = [
            "Walking to Newhaven, Arena",
            "No exit `n`: looking to relocate",
            "Arrived: Newhaven, Arena",
            "No exit `u`: looking to relocate",
        ];
        assert_eq!(relocations(said), 2);
    }

    #[test]
    fn the_longest_gap_includes_the_one_still_open() {
        let t0 = Instant::now();
        let at = |s| t0 + Duration::from_secs(s);
        let mut g = PromptGaps::new(t0);
        g.prompt(at(2));
        g.prompt(at(10));
        g.prompt(at(12));
        assert_eq!(g.longest(at(13)), Duration::from_secs(8));
        // Nothing since the last prompt for twenty seconds: that gap is the longest now.
        assert_eq!(g.longest(at(32)), Duration::from_secs(20));
        g.restart(at(40));
        assert_eq!(g.longest(at(41)), Duration::from_secs(1));
    }

    #[test]
    fn the_table_gives_every_number_the_card_names() {
        let t = Tally {
            loop_name: "Wooden Passages (MegaMud, Mwodloop.mp)".to_owned(),
            minutes: 3,
            exp_gained: 150,
            exp_per_hour: 3000,
            kills: 3,
            kills_by_monster: BTreeMap::from([("large rat".to_owned(), 3)]),
            unclassified: 1,
            lines: 500,
            longest_gap: Duration::from_millis(4500),
            run: "7".to_owned(),
            ..Tally::default()
        };
        let text = table(&t);
        for want in [
            "| Loop | Wooden Passages (MegaMud, Mwodloop.mp) |",
            "| Minutes | 3 |",
            "| Exp per hour | 3000 |",
            "| Kills | 3 (large rat 3) |",
            "| Deaths | 0 |",
            "| Flees | 0 |",
            "| Rests | 0 |",
            "| Relocations | 0 |",
            "| Unclassified lines | 1 of 500 (0.20%) |",
            "| Longest gap without a prompt | 4.5 s |",
            "| Brain restarts | 0 |",
            "| Run in the store | 7 |",
        ] {
            assert!(text.contains(want), "{want:?} not in\n{text}");
        }
        assert!(!text.contains("ended early"));
        let early = Tally {
            ended_early: Some("death".to_owned()),
            ..t
        };
        assert!(table(&early).contains("The run ended early: death"));
    }
}
