use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::Instant;

use super::*;

/// A table of `tests/fixtures/world`, record by record.
fn records(table: &str) -> Vec<Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/world")
        .join(format!("{table}.jsonl.gz"));
    let file = std::fs::File::open(&path).unwrap();
    BufReader::new(flate2::read::GzDecoder::new(file))
        .lines()
        .map(|l| serde_json::from_str(&l.unwrap()).unwrap())
        .collect()
}

fn named<'a>(list: &'a [Value], name: &str) -> &'a Value {
    list.iter()
        .find(|v| v["name"] == name)
        .unwrap_or_else(|| panic!("no {name:?} in the fixture"))
}

struct World {
    items: Vec<Value>,
    classes: Vec<Value>,
    races: Vec<Value>,
    monsters: Vec<Value>,
}

impl World {
    fn load() -> World {
        World {
            items: records("WCCITEMS"),
            classes: records("WCCCLASS"),
            races: records("WCCRACE"),
            monsters: records("WCCKNMSR"),
        }
    }

    fn item(&self, name: &str) -> ItemRecord {
        ItemRecord::from_value(named(&self.items, name)).unwrap()
    }

    fn item_number(&self, number: i64) -> ItemRecord {
        let v = self.items.iter().find(|v| v["number"] == number).unwrap();
        ItemRecord::from_value(v).unwrap()
    }

    fn class(&self, name: &str) -> ClassRecord {
        ClassRecord::from_value(named(&self.classes, name)).unwrap()
    }

    fn race(&self, name: &str) -> RaceRecord {
        RaceRecord::from_value(named(&self.races, name)).unwrap()
    }

    fn monster(&self, name: &str) -> MonsterRecord {
        MonsterRecord::from_value(named(&self.monsters, name)).unwrap()
    }
}

fn stats(str: i64, agi: i64) -> Stats {
    Stats {
        str,
        int: 50,
        agi,
        cha: 50,
    }
}

#[allow(clippy::too_many_arguments)]
fn fighter(
    class: &ClassRecord,
    race: &RaceRecord,
    level: i64,
    stats: Stats,
    weapon: Option<&ItemRecord>,
    load: i64,
    buffs: Vec<Buff>,
) -> Fighter {
    Fighter::new(&Character {
        class,
        race,
        level,
        stats,
        weapon,
        worn: Vec::new(),
        encumbrance: load,
        max_energy: STOCK_MAX_ENERGY,
        buffs,
    })
}

// --- Energy and blows ------------------------------------------------------------------

#[test]
fn the_kang_quarterstaff_crits_more_as_it_gets_faster() {
    // `kb/combat.md` "Fast weapons crit more": 238 before the load factor at level 16 and
    // 199 after it; +1 at 16, +9 at 17, +17 at 18, the full +20 from 19.
    let w = World::load();
    let (class, race, staff) = (
        w.class("Witchunter"),
        w.race("Kang"),
        w.item("quarterstaff"),
    );
    assert_eq!((staff.speed, staff.energy), (1200, 0));
    let at = |level| {
        fighter(
            &class,
            &race,
            level,
            stats(77, 42),
            Some(&staff),
            18,
            Vec::new(),
        )
    };
    let l16 = at(16);
    assert_eq!((l16.energy.raw, l16.energy.after_load), (238, 199));
    let bonus: Vec<i64> = (16..=20).map(|l| at(l).energy.fast_crit).collect();
    assert_eq!(bonus, [1, 9, 17, 20, 20]);
    // The crit is the base plus that bonus, and the blow still costs 200.
    assert_eq!(l16.crit, l16.mabase + 1);
    assert_eq!(l16.cost(Move::Attack), 200);
    // At level 1 the attack costs 605 and the bash is capped at a full round (`kb/combat.md`
    // "Attack modes"), with AV 58.
    let l1 = at(1);
    assert_eq!(l1.cost(Move::Attack), 605);
    assert_eq!(l1.cost(Move::Bash), 1000);
    assert_eq!(l1.av, 58);
    assert_eq!((l1.min_damage, l1.max_damage), (2, 14));
}

#[test]
fn the_witchunter_magebane_and_headcutter_at_levels_15_and_20() {
    let w = World::load();
    let (class, race) = (w.class("Witchunter"), w.race("Human"));
    let (magebane, headcutter) = (w.item("Magebane sword"), w.item("bone-handled headcutter"));
    assert_eq!((magebane.speed, magebane.strreq), (1600, 60));
    assert_eq!((headcutter.speed, headcutter.strreq), (2200, 80));

    let mb = fighter(
        &class,
        &race,
        15,
        stats(95, 56),
        Some(&magebane),
        55,
        Vec::new(),
    );
    assert_eq!(mb.cost(Move::Attack), 316);
    assert_eq!(mb.cost(Move::Bash), 632);
    // 1, 2, 1, 2, ... as the card has it, but the remainder drifts: 1312 left for round 7
    // pays for two.
    let blows = blows_per_round(mb.cost(Move::Bash), 1000, 8);
    assert_eq!(blows, [1, 2, 1, 2, 1, 2, 2, 1]);

    let hc = fighter(
        &class,
        &race,
        15,
        stats(95, 56),
        Some(&headcutter),
        51,
        Vec::new(),
    );
    assert_eq!(hc.cost(Move::Attack), 427);
    assert_eq!(hc.cost(Move::Bash), 854);
    assert_eq!(blows_per_round(854, 1000, 6), [1, 1, 1, 1, 1, 2]);

    let mb20 = fighter(
        &class,
        &race,
        20,
        stats(97, 62),
        Some(&magebane),
        54,
        Vec::new(),
    );
    assert_eq!(mb20.cost(Move::Bash), 496);
    assert_eq!(blows_per_round(496, 1000, 8), [2; 8]);
    let hc20 = fighter(
        &class,
        &race,
        20,
        stats(97, 62),
        Some(&headcutter),
        50,
        Vec::new(),
    );
    assert_eq!(hc20.cost(Move::Bash), 672);

    // The fight reports the same blows.
    let monster = Defender::new(&w.monster("giant rat"), &[]);
    let fight = Fight::new(&mb, &monster, Move::Bash);
    assert_eq!(fight.blows_per_round, [1, 2, 1, 2, 1, 2, 2, 1]);
    assert_eq!(fight.blow.cost, 632);
}

#[test]
fn a_gnome_druid_swings_a_maul_once_a_round_whatever_the_move() {
    let w = World::load();
    let maul = w.item("maul");
    assert_eq!((maul.speed, maul.strreq), (2600, 90));
    let f = fighter(
        &w.class("Druid"),
        &w.race("Gnome"),
        10,
        stats(60, 45),
        Some(&maul),
        42,
        Vec::new(),
    );
    assert!(f.energy.too_weak);
    assert_eq!(f.energy.after_load, 1309);
    assert_eq!((f.cost(Move::Attack), f.cost(Move::Bash)), (1000, 1000));
    assert_eq!(blows_per_round(f.cost(Move::Attack), 1000, 8), [1; 8]);
    // Too weak: no fast-blow bonus, ever.
    assert_eq!(f.energy.fast_crit, 0);
}

#[test]
fn a_blow_costing_200_strikes_five_a_round_and_never_more_than_six() {
    assert_eq!(blows_per_round(200, 1000, 3), [5, 5, 5]);
    // A cost below the floor never reaches here, but the cap holds on its own.
    assert_eq!(blows_per_round(100, 1000, 2), [6, 6]);
}

// --- Hit, dodge, crit ------------------------------------------------------------------

#[test]
fn hit_and_dodge_chances_from_the_kb() {
    // `kb/combat.md`: AV 58 against DV 40 hits 34; Dodge 15 against AV 58 dodges 21.
    assert_eq!(hit_chance(58, 0, 40), 34);
    assert_eq!(dodge_chance(15, 58), 21);
    assert_eq!(dodge_chance(67, 58), 95);
    assert_eq!(dodge_chance(50, 8), 0);
    // No attack power: 5, clamped up to 10. Never above 99.
    assert_eq!(hit_chance(3, 0, 0), 10);
    assert_eq!(hit_chance(200, 0, 0), 99);
    assert_eq!(crit_after_rule(40), 40);
    assert_eq!(crit_after_rule(70), 50);
}

#[test]
fn the_roll_includes_its_top_value() {
    // These numbers (and the by-hand ones below) are for `ROLL_TOP_INCLUDED`; flipping it
    // fails them on purpose.
    assert_eq!(roll_count(1, 100), 100);
    assert_eq!(roll_count(0, 100), 101);
    // A hit chance of 34 lands on 1..=33 of 1..=100.
    assert!((p_below(1, 100, 34) - 0.33).abs() < 1e-12);
    // A crit of 10 on 0..=9 of 0..=100.
    assert!((p_below(0, 100, 10) - 10.0 / 101.0).abs() < 1e-12);
}

#[test]
fn a_blow_reports_its_ranges_and_chances() {
    let f = Fighter {
        level: 10,
        str: 50,
        agi: 50,
        max_energy: 1000,
        energy: Energy {
            raw: 500,
            before_load: 500,
            after_load: 500,
            too_weak: false,
            fast_crit: 0,
        },
        min_damage: 4,
        max_damage: 10,
        av: 58,
        mabase: 1,
        crit: 70,
    };
    let d = Defender {
        level: 1,
        magicres: 0,
        maxhp: 50,
        dv: 40,
        dr: 20,
        dodge: 15,
        defmod: 0,
    };
    let b = Blow::new(&f, &d, Move::Attack, false);
    assert_eq!((b.hit, b.crit, b.dodge), (34, 50, 21));
    // 4..=11 less DR 2; crits 20..=41 less 2.
    assert_eq!(b.damage, (2, 9));
    assert_eq!(b.crit_damage, Some((18, 39)));
    assert!((b.p_hit - 0.33).abs() < 1e-12);
    assert!((b.p_crit - 0.33 * 50.0 / 101.0).abs() < 1e-12);
    assert!((b.p_dodge - 0.33 * 21.0 / 101.0).abs() < 1e-12);

    // A bash: -15 to hit, +10% on both ends, ×3, never a crit.
    let bash = Blow::new(&f, &d, Move::Bash, false);
    assert_eq!(bash.hit, hit_chance(58, -15, 40));
    assert_eq!(bash.crit, 0);
    assert_eq!(bash.crit_damage, None);
    // 4*110/100 = 4, 10*110/100 = 11: 4..=12, less 2, ×3.
    assert_eq!(bash.damage, (6, 30));
    assert_eq!(bash.cost, 1000);
}

#[test]
fn a_defence_modifier_adds_to_the_attackers_hit_modifier() {
    let mut d = Defender {
        level: 1,
        magicres: 0,
        maxhp: 10,
        dv: 40,
        dr: 0,
        dodge: 0,
        defmod: 10,
    };
    let f = plain_fighter(4, 6, 0);
    assert_eq!(
        Blow::new(&f, &d, Move::Attack, false).hit,
        hit_chance(f.av, 10, 40)
    );
    d.defmod = 0;
    assert_eq!(
        Blow::new(&f, &d, Move::Attack, false).hit,
        hit_chance(f.av, 0, 40)
    );
}

// --- Records and abilities -------------------------------------------------------------

#[test]
fn records_read_from_the_fixture() {
    let w = World::load();
    let magebane = w.item("Magebane sword");
    // Five slots in use; the junk values of the empty ones are dropped.
    assert_eq!(
        magebane.abilities,
        [(116, -10), (86, 2), (114, 10), (43, 353), (135, 15)]
    );
    assert_eq!(
        (magebane.mindmg, magebane.maxdmg, magebane.accuracy),
        (9, 22, 5)
    );
    let kang = w.race("Kang");
    assert_eq!(kang.abilities, [(21, 100), (7, 10), (2, 5)]);
    assert_eq!(w.class("Witchunter").combat, 7);

    let raw = serde_json::to_string(named(&w.monsters, "dark monk")).unwrap();
    let monk = MonsterRecord::from_raw(&raw).unwrap();
    let d = Defender::new(&monk, &[]);
    assert_eq!((d.dv, d.dr, d.dodge, d.maxhp), (30, 20, 15, 125));

    assert!(ItemRecord::from_raw("{}").is_err());
}

#[test]
fn a_monsters_weapon_and_worn_item_count_toward_its_abilities() {
    // The gnoll scout's longbow carries Dodge 5 (LJNABIL.C:681-810).
    let w = World::load();
    let scout = w.monster("gnoll scout");
    assert!(scout.weapon != 0);
    let bow = w.item_number(scout.weapon);
    assert_eq!(bow.name, "longbow");
    let bare = Defender::new(&scout, &[]);
    let armed = Defender::new(&scout, &[&bow]);
    assert_eq!(armed.dodge, bare.dodge + 5);
}

#[test]
fn bonuses_sum_but_attack_value_takes_the_best_and_energy_use_averages() {
    let w = World::load();
    let (class, race) = (w.class("Warrior"), w.race("Human"));
    let base = fighter(&class, &race, 10, stats(60, 60), None, 10, Vec::new());
    let buff = |b: Buff| fighter(&class, &race, 10, stats(60, 60), None, 10, vec![b]);

    let blessed = buff(Buff {
        av: 10,
        ..Buff::default()
    });
    assert_eq!(blessed.av, base.av + 10);
    // Two AV sources: the better one counts, not the sum.
    let twice = fighter(
        &class,
        &race,
        10,
        stats(60, 60),
        None,
        10,
        vec![
            Buff {
                av: 10,
                ..Buff::default()
            },
            Buff {
                av: 4,
                ..Buff::default()
            },
        ],
    );
    assert_eq!(twice.av, base.av + 10);

    let mut dark_elf = race.clone();
    dark_elf.abilities = vec![(ability::AV_THIRD, 3), (ability::CRITICAL, 1)];
    let elf = fighter(
        &class,
        &dark_elf,
        10,
        stats(60, 60),
        None,
        10,
        vec![Buff {
            av: 10,
            crit: 5,
            ..Buff::default()
        }],
    );
    assert_eq!(elf.av, base.av + 10);
    assert_eq!(elf.crit, base.crit + 6);

    let dmg = buff(Buff {
        max_damage: 3,
        ..Buff::default()
    });
    assert_eq!(
        (dmg.min_damage, dmg.max_damage),
        (base.min_damage, base.max_damage + 3)
    );

    // Strength and agility buffs change the stats before everything else.
    let strong = buff(Buff {
        str: 40,
        agi: 12,
        ..Buff::default()
    });
    assert_eq!(strong.str, 100);
    assert_eq!(strong.max_damage, base.max_damage + 4);
    // (100-50)/3 - (60-50)/3 + (72-50)/6 - (60-50)/6
    assert_eq!(strong.av, base.av + 16 - 3 + 3 - 1);

    // Energy use: the buff (a spell) comes first, then the race, then the class, then the
    // items; each later source is averaged with what came before.
    let mut fast_race = race.clone();
    fast_race.abilities = vec![(ability::ENERGY_USE, 50)];
    let pct = |buffs: Vec<Buff>, race: &RaceRecord| {
        fighter(&class, race, 10, stats(60, 60), None, 10, buffs)
            .energy
            .before_load
    };
    let raw = base.energy.raw;
    assert_eq!(pct(Vec::new(), &fast_race), raw * 50 / 100);
    let spell = Buff {
        energy_pct: 90,
        ..Buff::default()
    };
    assert_eq!(pct(vec![spell], &fast_race), raw * 70 / 100);
    assert_eq!(pct(vec![spell], &race), raw * 90 / 100);
}

#[test]
fn strength_moves_the_damage_range() {
    let w = World::load();
    let (class, race, staff) = (w.class("Warrior"), w.race("Human"), w.item("quarterstaff"));
    let at = |str| {
        let f = fighter(
            &class,
            &race,
            10,
            stats(str, 50),
            Some(&staff),
            10,
            Vec::new(),
        );
        (f.min_damage, f.max_damage)
    };
    assert_eq!(at(50), (2, 12));
    assert_eq!(at(45), (2, 12)); // -5/10 is 0 in C
    assert_eq!(at(30), (2, 10));
    assert_eq!(at(120), (6, 19));
    assert_eq!(at(0), (2, 7));
}

#[test]
fn light_load_adds_accuracy_and_worn_armour_counts() {
    let w = World::load();
    let (class, race) = (w.class("Warrior"), w.race("Human"));
    let light = fighter(&class, &race, 4, stats(50, 50), None, 10, Vec::new());
    let heavy = fighter(&class, &race, 4, stats(50, 50), None, 40, Vec::new());
    // accuracy 1 + 14 against 1: skill differs by 14/2 - 1/2 = 7, AV by 14.
    assert_eq!(light.av - heavy.av, 14);

    let mut helm = ItemRecord {
        item_type: ITEM_ARMOUR,
        accuracy: 6,
        ..ItemRecord::default()
    };
    let with = |helm: &ItemRecord| {
        Fighter::new(&Character {
            class: &class,
            race: &race,
            level: 4,
            stats: stats(50, 50),
            weapon: None,
            worn: vec![helm],
            encumbrance: 40,
            max_energy: 1000,
            buffs: Vec::new(),
        })
    };
    assert_eq!(with(&helm).av - heavy.av, 6 / 2 * 2);
    // Only armour adds accuracy.
    helm.item_type = 1;
    assert_eq!(with(&helm).av, heavy.av);
}

// --- Kill probabilities against a brute force ------------------------------------------

/// A fighter with a fixed damage range, one blow a round (it costs the whole round), AV 200.
fn plain_fighter(min: i64, max: i64, crit: i64) -> Fighter {
    Fighter {
        level: 20,
        str: 50,
        agi: 50,
        max_energy: 1000,
        energy: Energy {
            raw: 2000,
            before_load: 2000,
            after_load: 2000,
            too_weak: false,
            fast_crit: 0,
        },
        min_damage: min,
        max_damage: max,
        av: 200,
        mabase: 1,
        crit,
    }
}

/// Every value `genrdn(lo, hi)` can return.
fn support(lo: i64, hi: i64) -> std::ops::Range<i64> {
    lo..lo + roll_count(lo, hi)
}

/// `calculate_attack` as the C has it, run for every value of every roll: the blow's damage
/// (capped by nothing) with its chance. LJNCOMBT.C:1226-1427.
fn brute_blow(f: &Fighter, d: &Defender, mv: Move, down: bool) -> Vec<(i64, f64)> {
    let (mut hitmod, dmgbonus, mut crit) = match mv {
        Move::Attack => (0, 0, f.crit),
        Move::Bash => (-15, 10, 0),
        Move::Smash => (-25, 20, 0),
    };
    let dv = if down {
        if d.dv < 35 { 0 } else { d.dv - 35 }
    } else {
        d.dv
    };
    hitmod += d.defmod;
    let mut min0 = f.min_damage;
    let mut max0 = f.max_damage;
    if dmgbonus != 0 {
        min0 = min0 * (dmgbonus + 100) / 100;
        max0 = max0 * (dmgbonus + 100) / 100;
    }
    if crit > 40 {
        crit = (crit - 40) / 3 + 40;
    }
    // Indexed by damage: a map would make the ~10^7 leaves slow.
    let mut out: Vec<f64> = Vec::new();
    let mut add = |damage: i64, w: f64| {
        let i = damage as usize;
        if i >= out.len() {
            out.resize(i + 1, 0.0);
        }
        out[i] += w;
    };
    let p = |lo, hi| 1.0 / roll_count(lo, hi) as f64;
    for roll in support(1, 100) {
        // The off-guard roll comes only for a negative dodge.
        let guards: Vec<(bool, f64)> = if d.dodge < 0 {
            support(0, 100)
                .map(|g| (g > d.dodge + 100, p(0, 100)))
                .collect()
        } else {
            vec![(false, 1.0)]
        };
        for (off_guard, pg) in guards {
            let w = p(1, 100) * pg;
            let hitchance = if off_guard {
                99
            } else {
                let power = (f.av + hitmod) * (f.av + hitmod) / 14 / 10;
                let h = if power != 0 { 100 - dv * dv / power } else { 5 };
                h.clamp(10, 99)
            };
            if roll >= hitchance {
                add(0, w);
                continue;
            }
            for crit_roll in support(0, 100) {
                let w = w * p(0, 100);
                let (mut min, mut max) = (min0, max0);
                if crit_roll < crit && mv == Move::Attack {
                    min = max * 2;
                    max *= 4;
                }
                if min > max {
                    max = min;
                }
                let n = max - min + 1;
                for dmg_roll in support(0, n) {
                    let w = w * p(0, n);
                    let mut damage = dmg_roll + min - d.dr / 10;
                    let dodgechance = if f.av > 8 {
                        d.dodge * 10 / (f.av / 8)
                    } else {
                        0
                    }
                    .min(95);
                    damage *= match mv {
                        Move::Attack => 1,
                        Move::Bash => 3,
                        Move::Smash => 5,
                    };
                    let dodges: Vec<(bool, f64)> = if d.dodge > 0 {
                        support(0, 100)
                            .map(|r| (r < dodgechance, p(0, 100)))
                            .collect()
                    } else {
                        vec![(false, 1.0)]
                    };
                    for (dodged, pd) in dodges {
                        let damage = if dodged { 0 } else { damage.max(0) };
                        add(damage, w * pd);
                    }
                }
            }
        }
    }
    drop(add);
    out.iter()
        .enumerate()
        .filter(|(_, p)| **p > 0.0)
        .map(|(x, p)| (x as i64, *p))
        .collect()
}

/// The chance the monster is dead after `blows` blows, every outcome enumerated.
fn brute_kill(f: &Fighter, d: &Defender, mv: Move, blows: usize) -> f64 {
    let knocks_down = mv == Move::Smash && f.str + f.level > d.level + d.magicres + 30;
    let up = brute_blow(f, d, mv, false);
    let down = if knocks_down {
        brute_blow(f, d, mv, true)
    } else {
        Vec::new()
    };
    fn go(
        dealt: i64,
        is_down: bool,
        left: usize,
        hp: i64,
        up: &[(i64, f64)],
        down: &[(i64, f64)],
        kd: bool,
    ) -> f64 {
        if dealt >= hp {
            return 1.0;
        }
        if left == 0 {
            return 0.0;
        }
        let outcomes = if is_down { down } else { up };
        outcomes
            .iter()
            .map(|&(dmg, p)| {
                p * go(
                    dealt + dmg,
                    is_down || (kd && dmg > 0),
                    left - 1,
                    hp,
                    up,
                    down,
                    kd,
                )
            })
            .sum()
    }
    go(0, false, blows, d.maxhp, &up, &down, knocks_down)
}

#[test]
fn two_blows_of_4_to_6_against_10_hp_agree_with_the_brute_force() {
    let f = plain_fighter(4, 6, 0);
    let d = Defender {
        level: 1,
        magicres: 0,
        maxhp: 10,
        dv: 0,
        dr: 0,
        dodge: 0,
        defmod: 0,
    };
    let fight = Fight::new(&f, &d, Move::Attack);
    assert_eq!(fight.blows_per_round[..3], [1, 1, 1]);
    assert_eq!(fight.blow.hit, 99);
    for k in 1..=4 {
        let brute = brute_kill(&f, &d, Move::Attack, k);
        assert!(
            (fight.p_kill_by_round[k - 1] - brute).abs() < 1e-9,
            "after {k}: {} vs {brute}",
            fight.p_kill_by_round[k - 1]
        );
    }
    // By hand: each blow lands on 98 of 100 rolls for 4, 5, 6 or 7 (the top included). Two
    // landed blows reach 10 unless they are (4,4) or (4,5) or (5,4): 13 of 16.
    let two = 0.98 * 0.98 * 13.0 / 16.0;
    assert!((fight.p_kill_by_round[1] - two).abs() < 1e-12);
    assert!(fight.p_kill_by_round[0].abs() < 1e-12);
}

#[test]
fn crits_dodges_and_armour_agree_with_the_brute_force() {
    // Commented: the brute force enumerates ~10^6 roll combinations per blow.
    let f = Fighter {
        av: 60,
        crit: 55,
        ..plain_fighter(2, 6, 0)
    };
    let d = Defender {
        level: 1,
        magicres: 0,
        maxhp: 24,
        dv: 30,
        dr: 25,
        dodge: 12,
        defmod: -3,
    };
    for mv in [Move::Attack, Move::Bash] {
        let fight = Fight::new(&f, &d, mv);
        for k in 1..=3 {
            let brute = brute_kill(&f, &d, mv, k);
            let got = fight.p_kill_by_round[k - 1];
            assert!(
                (got - brute).abs() < 1e-9,
                "{mv:?} after {k}: {got} vs {brute}"
            );
        }
        let mean: f64 = brute_blow(&f, &d, mv, false)
            .iter()
            .map(|(x, p)| *x as f64 * p)
            .sum();
        assert!((fight.expected_damage_per_blow - mean).abs() < 1e-9);
    }
}

#[test]
fn a_negative_dodge_is_caught_off_guard_like_the_brute_force() {
    let f = Fighter {
        av: 40,
        ..plain_fighter(3, 5, 1)
    };
    let d = Defender {
        level: 1,
        magicres: 0,
        maxhp: 9,
        dv: 45,
        dr: 0,
        dodge: -20,
        defmod: 0,
    };
    let fight = Fight::new(&f, &d, Move::Attack);
    for k in 1..=3 {
        let brute = brute_kill(&f, &d, Move::Attack, k);
        assert!((fight.p_kill_by_round[k - 1] - brute).abs() < 1e-9);
    }
}

#[test]
fn a_smash_knocks_down_and_the_next_blows_hit_more_often() {
    let f = Fighter {
        str: 97,
        av: 70,
        ..plain_fighter(3, 8, 0)
    };
    let d = Defender {
        level: 10,
        magicres: 40,
        maxhp: 60,
        dv: 45,
        dr: 10,
        dodge: 0,
        defmod: 0,
    };
    // 97 + 20 > 10 + 40 + 30.
    let fight = Fight::new(&f, &d, Move::Smash);
    let down = fight.down_blow.expect("knocks down");
    assert!(down.hit > fight.blow.hit);
    assert_eq!(down.hit, hit_chance(70, -25, 10));
    assert_eq!(fight.blows_per_round, [1; 8]);
    for k in 1..=4 {
        let brute = brute_kill(&f, &d, Move::Smash, k);
        assert!(
            (fight.p_kill_by_round[k - 1] - brute).abs() < 1e-9,
            "after {k}"
        );
    }
    // Round 2's expected damage mixes the two by the chance round 1 knocked it down.
    let p = fight.blow.p_damage();
    let want = p * down.expected_damage + (1.0 - p) * fight.blow.expected_damage;
    assert!((fight.expected_damage_round2 - want).abs() < 1e-12);
    assert!(fight.expected_damage_round2 > fight.expected_damage_round1);

    // A monster too strong to knock down: no down blow.
    let tough = Defender { magicres: 80, ..d };
    assert_eq!(Fight::new(&f, &tough, Move::Smash).down_blow, None);
    assert_eq!(Fight::new(&f, &d, Move::Bash).down_blow, None);
}

#[test]
fn a_monster_eight_rounds_cannot_kill_caps_the_expected_rounds() {
    let f = plain_fighter(4, 6, 0);
    let d = Defender {
        level: 1,
        magicres: 0,
        maxhp: 1000,
        dv: 0,
        dr: 0,
        dodge: 0,
        defmod: 0,
    };
    let fight = Fight::new(&f, &d, Move::Attack);
    assert!(fight.p_kill_by_round.iter().all(|p| p.abs() < 1e-12));
    assert!((fight.expected_rounds - 20.0).abs() < 1e-9);
    assert!(fight.rounds_capped);

    // And one that dies in the first round for sure is 1, not capped.
    let rat = Defender { maxhp: 1, ..d };
    let quick = Fight::new(&f, &rat, Move::Attack);
    assert!((quick.p_kill_by_round[0] - 0.98).abs() < 1e-12);
    assert!(!quick.rounds_capped);
    assert!(quick.expected_rounds < 1.03);
}

#[test]
fn expected_rounds_is_the_sum_of_the_chances_still_alive() {
    let f = plain_fighter(4, 6, 0);
    let d = Defender {
        level: 1,
        magicres: 0,
        maxhp: 10,
        dv: 0,
        dr: 0,
        dodge: 0,
        defmod: 0,
    };
    let fight = Fight::new(&f, &d, Move::Attack);
    // Rounds beyond 8 add a little, 20 at most.
    let first8: f64 = 1.0
        + fight.p_kill_by_round[..7]
            .iter()
            .map(|p| 1.0 - p)
            .sum::<f64>();
    assert!(fight.expected_rounds >= first8 - 1e-12);
    assert!(fight.expected_rounds < first8 + 1e-6);
    assert!(fight.expected_damage_round1 > 0.0);
    assert!((fight.expected_damage_round1 - fight.expected_damage_per_blow).abs() < 1e-12);
}

// --- Performance -----------------------------------------------------------------------

#[test]
fn the_gear_planners_4500_fights_take_well_under_a_second() {
    // Why this test: the gear planner (C-059) runs about 150 weapons × 3 moves × 10 monsters
    // per request, so this many fights must stay fast. Every monster has 300 HP; the slow
    // weapons need many rounds, the worst case for the exact tally.
    let w = World::load();
    let (class, race) = (w.class("Witchunter"), w.race("Human"));
    let weapons: Vec<ItemRecord> = w
        .items
        .iter()
        .filter(|v| v["type"] == 1)
        .take(150)
        .map(|v| ItemRecord::from_value(v).unwrap())
        .collect();
    assert_eq!(weapons.len(), 150);
    let fighters: Vec<Fighter> = weapons
        .iter()
        .map(|wpn| fighter(&class, &race, 20, stats(97, 62), Some(wpn), 40, Vec::new()))
        .collect();
    let monsters: Vec<Defender> = (0..10)
        .map(|i| Defender {
            level: 10 + i,
            magicres: 30,
            maxhp: 300,
            dv: 10 * i,
            dr: 10 * i,
            dodge: i,
            defmod: 0,
        })
        .collect();
    let start = Instant::now();
    let mut kills = 0.0;
    for f in &fighters {
        for d in &monsters {
            for mv in Move::ALL {
                kills += Fight::new(f, d, mv).p_kill_by_round[1];
            }
        }
    }
    let took = start.elapsed();
    assert!(kills >= 0.0);
    assert!(took.as_secs_f64() < 1.0, "4,500 fights took {took:?}");
}
