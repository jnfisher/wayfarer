//! The gear plan (docs/brain.md "Gear plan", card C-059): every weapon scored by attack, bash
//! and smash against a monster set, from what-if values and from the character's own. The
//! fixture cases were worked out with John on 2026-10-26.
//!
//! Each test can take over a second: its process loads the world (`game::world`, about half a
//! second) and builds the gear catalogue of each class and race it asks for (about 0.1 s each).

mod game;

use std::time::{Duration, Instant};

use game::arena::*;
use game::*;
use wayfarer_brain::gear_plan::{Ask, Gear, Plan, Subject};
use wayfarer_proto::{
    GearDetail, GearDetailRequest, GearFilter, GearMonsters, GearMove, GearMoveScore, GearPlan,
    GearPlanRequest, GearRow, GearStatus, GearWhatIf,
};

/// The card's monster set.
const SET: [&str; 9] = [
    "kobold warrior",
    "bandit leader",
    "smuggler",
    "skeletal warrior",
    "dark goblin",
    "grey wolf",
    "minotaur",
    "minotaur champion",
    "guardian golem",
];

const HEADCUTTER: &str = "bone-handled headcutter";
const MAGEBANE: &str = "Magebane sword";

fn gear() -> Arc<Gear> {
    world().gear.unwrap()
}

fn subject(class: &str, race: &str) -> Subject {
    Subject {
        class: Some(class.into()),
        race: Some(race.into()),
        ..Subject::default()
    }
}

/// Level, Str, Int, Wil, Agi, Hea, Cha and the load, per cent.
fn what_if(v: [i64; 8]) -> GearWhatIf {
    let [level, str, int, wil, agi, hea, cha, load] = v;
    GearWhatIf {
        level: Some(level),
        str: Some(str),
        int: Some(int),
        wil: Some(wil),
        agi: Some(agi),
        hea: Some(hea),
        cha: Some(cha),
        load_percent: Some(load),
    }
}

/// The card's Kang Witchunter at level 15, and at 20.
const WITCHUNTER_15: [i64; 8] = [15, 95, 30, 45, 56, 91, 41, 51];
const WITCHUNTER_20: [i64; 8] = [20, 97, 30, 45, 62, 91, 41, 54];
/// The card's Gnome Druid (Faye) at level 10.
const DRUID_10: [i64; 8] = [10, 60, 50, 70, 45, 75, 35, 42];

fn ask(v: [i64; 8]) -> Ask {
    Ask {
        what_if: what_if(v),
        ..Ask::default()
    }
}

fn plan(who: &Subject, ask: &Ask) -> Plan {
    let g = gear();
    let names: Vec<String> = SET.iter().map(|s| s.to_string()).collect();
    let (set, unknown) = g.named_monsters(&names);
    assert!(unknown.is_empty(), "{unknown:?}");
    g.plan(who, ask, &set, |_| None).unwrap()
}

fn row<'a>(p: &'a Plan, name: &str) -> &'a GearRow {
    p.rows
        .iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("no row {name}"))
}

fn score(r: &GearRow, m: GearMove) -> &GearMoveScore {
    r.moves
        .iter()
        .find(|s| s.mv == m)
        .unwrap_or_else(|| panic!("{} has no {m:?}", r.name))
}

#[test]
fn a_kang_witchunter_bashes_and_the_magebane_overtakes_the_headcutter_at_20() {
    let who = subject("Witchunter", "Kang");
    let at_15 = plan(&who, &ask(WITCHUNTER_15));
    assert!(at_15.missing.is_empty(), "{:?}", at_15.missing);
    for name in [HEADCUTTER, MAGEBANE] {
        let r = row(&at_15, name);
        assert_eq!(r.status, GearStatus::Now, "{name}");
        assert_eq!(r.best_move, Some(GearMove::Bash), "{name}");
        assert!(
            score(r, GearMove::Bash).rounds < score(r, GearMove::Attack).rounds,
            "{name}: {:?}",
            r.moves
        );
        // Below level 20 a Witchunter's smash is not scored.
        assert_eq!(r.moves.len(), 2);
    }

    let at_20 = plan(&who, &ask(WITCHUNTER_20));
    let magebane = score(row(&at_20, MAGEBANE), GearMove::Bash);
    let headcutter = score(row(&at_20, HEADCUTTER), GearMove::Bash);
    // Two bashes every round against one or two.
    assert_eq!(magebane.blows, [2, 2, 2]);
    assert_eq!(headcutter.blows, [1, 1, 2]);
    assert!(
        magebane.rounds < headcutter.rounds,
        "{magebane:?} {headcutter:?}"
    );
    // The inputs are the what-if's.
    assert_eq!(at_20.inputs.level, Some(20));
    assert_eq!(at_20.inputs.load_percent, Some(54));
    assert!(at_20.inputs.what_if.contains(&"str".to_owned()));
    // One row is the best, and it is scored.
    let best: Vec<_> = at_20.rows.iter().filter(|r| r.best).collect();
    assert_eq!(best.len(), 1);
    assert!(!best[0].moves.is_empty());
}

#[test]
fn a_gnome_druid_bashes_with_the_maul_but_the_darkwood_staff_beats_it() {
    let p = plan(&subject("Druid", "Gnome"), &ask(DRUID_10));
    let maul = row(&p, "maul");
    let staff = row(&p, "darkwood staff");
    assert_eq!(maul.best_move, Some(GearMove::Bash));
    assert_eq!(staff.best_move, Some(GearMove::Attack));
    // Too weak for the maul: 30 Str short.
    assert_eq!(maul.str_short, Some(30));
    assert!(
        score(staff, GearMove::Attack).rounds < score(maul, GearMove::Bash).rounds,
        "{:?} {:?}",
        staff.moves,
        maul.moves
    );
}

#[test]
fn a_ticked_buff_raises_the_hit_chance() {
    let who = subject("Witchunter", "Kang");
    let plain = plan(&who, &ask(WITCHUNTER_15));
    let valour = plain
        .buffs_offered
        .iter()
        .find(|b| b.name == "song of valour")
        .expect("song of valour offered");
    assert_eq!(valour.effect, "song of valour: +5 AV");
    assert_eq!(valour.classes, ["Bard"]);
    // A slot of value 0 takes the spell's strength: bless is +3 AV.
    let bless = plain.buffs_offered.iter().find(|b| b.name == "bless");
    assert_eq!(bless.map(|b| b.effect.as_str()), Some("bless: +3 AV"));
    // Spells that lower the target are not buffs.
    assert!(!plain.buffs_offered.iter().any(|b| b.name == "curse"));

    let buffed = plan(
        &who,
        &Ask {
            buffs: vec!["Song of Valour".into()],
            ..ask(WITCHUNTER_15)
        },
    );
    assert_eq!(buffed.inputs.buffs, ["song of valour"]);
    let (before, after) = (row(&plain, MAGEBANE), row(&buffed, MAGEBANE));
    for m in [GearMove::Attack, GearMove::Bash] {
        assert!(
            score(after, m).hit > score(before, m).hit,
            "{m:?}: {:?} then {:?}",
            before.moves,
            after.moves
        );
    }
    // The Witchunter resists friendly buffs some of the time: its rows say so.
    assert!(before.note.is_none());
    assert!(
        after
            .note
            .as_deref()
            .is_some_and(|n| n.contains("Anti-Magic"))
    );
    // A name the plan does not offer is said.
    let unknown = plan(
        &who,
        &Ask {
            buffs: vec!["no such song".into()],
            ..ask(WITCHUNTER_15)
        },
    );
    assert!(unknown.inputs.buffs.is_empty());
    assert_eq!(unknown.notes, ["no buff \"no such song\""]);
}

#[test]
fn smash_only_for_a_class_that_keeps_it_from_its_level_and_when_asked() {
    let with_smash = |v| Ask {
        smash: true,
        ..ask(v)
    };
    let has_smash = |p: &Plan| {
        p.rows
            .iter()
            .any(|r| r.moves.iter().any(|s| s.mv == GearMove::Smash))
    };
    // A Bard never keeps it.
    let bard = plan(&subject("Bard", "Human"), &with_smash(WITCHUNTER_20));
    assert!(!has_smash(&bard));
    assert!(!bard.inputs.smash);
    let witchunter = subject("Witchunter", "Kang");
    // A Witchunter from level 20, and only when asked.
    assert!(!has_smash(&plan(&witchunter, &with_smash(WITCHUNTER_15))));
    assert!(!has_smash(&plan(&witchunter, &ask(WITCHUNTER_20))));
    let at_20 = plan(&witchunter, &with_smash(WITCHUNTER_20));
    assert!(at_20.inputs.smash);
    let r = row(&at_20, MAGEBANE);
    let smash = score(r, GearMove::Smash);
    // A smash costs all the energy: one a round.
    assert_eq!((smash.energy, &smash.blows[..]), (1000, &[1, 1, 1][..]));
}

#[test]
fn the_filter_keeps_now_or_later_and_never_rows_are_listed_but_not_scored() {
    let who = subject("Witchunter", "Kang");
    let all = plan(&who, &ask([10, 95, 30, 45, 56, 91, 41, 51]));
    let never: Vec<_> = all
        .rows
        .iter()
        .filter(|r| r.status == GearStatus::Never)
        .collect();
    assert!(!never.is_empty());
    assert!(
        never
            .iter()
            .all(|r| r.moves.is_empty() && !r.reasons.is_empty())
    );
    let broadsword = row(&all, "golden broadsword");
    assert_eq!(broadsword.reasons, ["magical"]);
    // The Magebane is for level 15: later at 10, and scored.
    let magebane = row(&all, MAGEBANE);
    assert_eq!(magebane.status, GearStatus::Later);
    assert_eq!(magebane.reasons, ["level 15"]);
    assert_eq!(magebane.min_level, Some(15));
    assert!(!magebane.moves.is_empty());
    // Its quest, and the champion that drops the headcutter.
    assert!(
        magebane
            .sources
            .iter()
            .any(|s| s.name == "huge dwarven witchunter")
    );
    let headcutter = row(&all, HEADCUTTER);
    assert!(headcutter.two_handed);
    assert!(
        headcutter
            .sources
            .iter()
            .any(|s| s.name == "minotaur champion" && s.boss && s.percent == Some(25))
    );
    // Usable now first, then later, then never.
    let order: Vec<_> = all.rows.iter().map(|r| r.status as u8).collect();
    assert!(order.is_sorted(), "{order:?}");

    for (filter, status) in [
        (GearFilter::Now, GearStatus::Now),
        (GearFilter::Later, GearStatus::Later),
    ] {
        let p = plan(
            &who,
            &Ask {
                filter,
                ..ask([10, 95, 30, 45, 56, 91, 41, 51])
            },
        );
        assert!(!p.rows.is_empty());
        assert!(p.rows.iter().all(|r| r.status == status), "{filter:?}");
    }
}

#[test]
fn a_whole_witchunter_plan_answers_in_time() {
    // The card's limit: the planner asks again on every change of an input, so a plan for
    // the weapons a class can use or will (about 180 here) × 3 moves × a 10-monster set must
    // answer within 100 ms. The catalogue is built once per class and race and kept (the
    // first request builds it), so it is warmed first; the fastest of three is timed, since
    // other tests running beside this one only slow it down (nextest runs it alone,
    // `.config/nextest.toml`). 40 to 50 ms on 4 cores in a test build.
    let who = subject("Witchunter", "Kang");
    let a = Ask {
        smash: true,
        ..ask(WITCHUNTER_20)
    };
    let p = plan(&who, &a);
    let scored = p.rows.iter().filter(|r| r.moves.len() == 3).count();
    assert!(scored > 150, "{scored} rows scored");
    assert!(p.monsters.len() >= 9);
    let mut fastest = Duration::MAX;
    for _ in 0..3 {
        let start = Instant::now();
        let p = plan(&who, &a);
        fastest = fastest.min(start.elapsed());
        assert!(!p.rows.is_empty());
    }
    assert!(
        fastest < Duration::from_millis(100),
        "a plan took {fastest:?}"
    );
}

// Through the brain: a `gear_plan_request` answered from the character it knows.

fn request(monsters: Option<GearMonsters>, what_if: Option<GearWhatIf>) -> ToClient {
    ToClient::GearPlanRequest(GearPlanRequest {
        session: ID.into(),
        monsters,
        what_if,
        buffs: None,
        smash: None,
        filter: None,
    })
}

async fn answer(g: &mut Game, req: ToClient) -> GearPlan {
    g.send(req).await;
    g.until(|m| match m {
        ToGateway::GearPlan(p) => Some(p.clone()),
        ToGateway::Warning { text, .. } if text.starts_with("gear") => panic!("{text}"),
        _ => None,
    })
    .await
}

fn names() -> Option<GearMonsters> {
    Some(GearMonsters::Names(
        SET.iter().map(|s| s.to_string()).collect(),
    ))
}

#[tokio::test(start_paused = true)]
async fn without_the_status_screen_the_plan_says_what_is_missing() {
    let (mut g, run) = in_arena(&[]).await;
    let only_load = GearWhatIf {
        load_percent: Some(40),
        ..GearWhatIf::default()
    };
    let p = answer(&mut g, request(names(), Some(only_load))).await;
    assert_eq!(p.missing, Some(vec!["stats".to_owned()]));
    assert!(p.rows.is_empty());
    // The buffs are offered all the same, and the areas for the planner's suggestions.
    assert!(!p.buffs_offered.is_empty());
    assert!(!p.areas.is_empty());
    g.close(run).await;
}

#[test]
fn the_areas_offered_are_sorted_names_whose_rooms_spawn() {
    let g = gear();
    let p = g
        .plan(&Subject::default(), &Ask::default(), &[], |_| None)
        .unwrap();
    assert!(p.areas.len() > 10, "{:?}", p.areas);
    assert!(p.areas.windows(2).all(|w| w[0] < w[1]), "sorted, each once");
    for area in &p.areas {
        assert!(!area.contains(',') && area.trim() == area, "{area:?}");
    }
    // They name sets with monsters in them.
    for area in [&p.areas[0], &p.areas[p.areas.len() / 2]] {
        assert!(!g.area_monsters(area).unwrap().is_empty(), "{area:?}");
    }
}

#[tokio::test(start_paused = true)]
async fn a_plan_from_the_characters_own_values_swaps_the_readied_weapon() {
    let (mut g, run) = in_arena(&[]).await;
    g.output(&status(35)).await;
    g.output(&line(
        "You are carrying padded vest (Torso), quarterstaff (Two handed)\r\nYou have no keys.\r\n\
         Wealth: 0 copper farthings\r\nEncumbrance: 400/1000 - Light [40%]",
        35,
    ))
    .await;
    // No set asked for and no loop running: the area around the Arena.
    let p = answer(&mut g, request(None, None)).await;
    assert_eq!(p.missing, None, "{:?}", p.notes);
    assert!(!p.monsters.is_empty());
    assert!(p.inputs.what_if.is_empty());
    assert!(p.inputs.class.is_some() && p.inputs.str.is_some());
    assert_eq!(p.inputs.load_percent, Some(40));
    assert_eq!(p.inputs.readied.as_deref(), Some("quarterstaff"));
    // The quarterstaff (100) swapped for the maul (300): 600/1000.
    let maul = p
        .rows
        .iter()
        .find(|r| r.name == "maul")
        .expect("a maul row");
    if maul.status != GearStatus::Never {
        assert_eq!(maul.load_percent, Some(60));
    }
    let staff = p
        .rows
        .iter()
        .find(|r| r.name == "quarterstaff")
        .expect("a quarterstaff row");
    assert_eq!(staff.load_percent, Some(40));
    assert!(p.rows.iter().any(|r| !r.moves.is_empty()));

    // The same with a what-if: its values are used and named.
    let p = answer(&mut g, request(names(), Some(what_if(WITCHUNTER_15)))).await;
    assert_eq!(p.inputs.level, Some(15));
    assert_eq!(p.inputs.load_percent, Some(51));
    assert_eq!(p.inputs.readied, None);
    assert_eq!(p.inputs.what_if.len(), 8);
    assert!(p.rows.iter().any(|r| !r.moves.is_empty()));

    // Names that match no monster are said.
    let p = answer(
        &mut g,
        request(
            Some(GearMonsters::Names(vec![
                "kobold warrior".into(),
                "nobody".into(),
            ])),
            None,
        ),
    )
    .await;
    assert_eq!(p.notes, ["no monster \"nobody\""]);
    g.close(run).await;
}

#[tokio::test(start_paused = true)]
async fn a_detail_gives_every_monster_and_eight_rounds_of_blows() {
    let (mut g, run) = in_arena(&[]).await;
    g.output(&status(35)).await;
    g.send(ToClient::GearDetailRequest(GearDetailRequest {
        session: ID.into(),
        item: 100,
        monsters: names(),
        what_if: Some(what_if(WITCHUNTER_15)),
        buffs: None,
        smash: None,
    }))
    .await;
    let d: GearDetail = g
        .until(|m| match m {
            ToGateway::GearDetail(d) => Some(d.clone()),
            _ => None,
        })
        .await;
    assert_eq!(d.name, "quarterstaff");
    assert_eq!(d.moves.len(), 2);
    for m in &d.moves {
        assert_eq!(m.blows_per_round.len(), 8);
        assert_eq!(m.monsters.len(), d.monsters.len());
        assert!(m.monsters.iter().all(|s| s.kill_by_round.len() == 8));
    }
    g.close(run).await;
}
