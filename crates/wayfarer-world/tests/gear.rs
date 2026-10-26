//! The gear catalogue, read from the fixtures' pack (`kb/economy.md` "Carrying and wearing
//! items"): who may use what, and where it comes from. The cases were checked by hand against
//! the stock data (2026-10-26).

mod common;

use std::time::{Duration, Instant};

use wayfarer_world::gear::{
    Alignment, Catalogue, GearItem, Kind, QuestAt, Reason, Source, Status, Wearer, area_monsters,
    loop_monsters, named_monsters,
};
use wayfarer_world::loops::megamud_loops;
use wayfarer_world::{Monsters, RoomGraph, WorldPack};

const WITCHUNTER: i64 = 2;
const CLERIC: i64 = 4;
const BARD: i64 = 9;
const DRUID: i64 = 13;
const DWARF: i64 = 2;
const GNOME: i64 = 3;
const HALF_ELF: i64 = 6;
const KANG: i64 = 11;

fn pack() -> WorldPack {
    WorldPack::open(common::pack_path()).unwrap()
}

/// The catalogue at a level, Charm 75 and Strength 80, every shop one step away.
fn at(cat: &Catalogue, level: i64) -> Vec<GearItem> {
    let who = Wearer {
        level,
        charm: Some(75),
        strength: Some(80),
    };
    cat.for_level(who, |rooms| rooms.first().map(|&k| (1, k)))
}

fn item<'a>(items: &'a [GearItem], name: &str) -> &'a GearItem {
    items
        .iter()
        .find(|i| i.name == name)
        .unwrap_or_else(|| panic!("{name} not in the catalogue"))
}

fn placed(i: &GearItem) -> Vec<(i64, i64)> {
    i.sources
        .iter()
        .filter_map(|s| match s {
            Source::Placed { room } => Some(*room),
            _ => None,
        })
        .collect()
}

#[test]
fn a_whole_catalogue_builds_in_time() {
    let pack = pack();
    // The limit is the card's: the planner builds one per class and race when it is asked for
    // one, so a build must be quick (about 120 ms on a laptop core; most of it is reading the
    // 26,720 rooms). The fastest of three builds is timed: the other tests running beside this
    // one only ever slow it down.
    let mut fastest = Duration::MAX;
    let mut cat = None;
    for _ in 0..3 {
        let start = Instant::now();
        cat = Some(Catalogue::build(&pack, WITCHUNTER, KANG).unwrap());
        fastest = fastest.min(start.elapsed());
    }
    assert!(fastest < Duration::from_millis(200), "built in {fastest:?}");
    let cat = cat.unwrap();
    let items: Vec<_> = cat.items().collect();
    assert!(items.len() > 800, "{}", items.len());
    assert!(items.iter().any(|i| i.kind == Kind::Weapon));
    assert!(items.iter().any(|i| i.kind == Kind::Armour));
}

#[test]
fn a_kang_witchunter_at_level_10() {
    let pack = pack();
    let cat = Catalogue::build(&pack, WITCHUNTER, KANG).unwrap();
    let items = at(&cat, 10);

    // The class list names the Witchunter; placed in the Large Tomb.
    let cutlass = item(&items, "mithril cutlass");
    assert_eq!(cutlass.usability.status, Status::Now);
    assert_eq!(placed(cutlass), [(1, 2236)]);
    assert_eq!(cutlass.limited, Some(1));

    // From level 15, given by the huge dwarven witchunter's "sword" answer (block 393 → 398 →
    // 399).
    let magebane = item(&items, "Magebane sword");
    assert_eq!(magebane.usability.status, Status::Later);
    assert_eq!(magebane.usability.reasons, [Reason::Level(15)]);
    let quest = magebane
        .sources
        .iter()
        .find_map(|s| match s {
            Source::Quest { block, at } => Some((*block, at.clone())),
            _ => None,
        })
        .expect("a quest source");
    assert_eq!(quest.0, 399);
    assert!(
        quest.1.iter().any(
            |a| matches!(a, QuestAt::Monster { name, .. } if name == "huge dwarven witchunter")
        ),
        "{:?}",
        quest.1
    );
    assert_eq!(
        item(&at(&cat, 15), "Magebane sword").usability.status,
        Status::Now
    );

    // The minotaur champion carries it, 25%: a boss, and on a server where it has never died
    // the first kill drops it.
    let axe = item(&items, "bone-handled headcutter");
    assert_eq!(axe.usability.status, Status::Now);
    let drop = axe
        .sources
        .iter()
        .find(|s| matches!(s, Source::Drop { name, .. } if name == "minotaur champion"))
        .expect("the champion's drop");
    let Source::Drop {
        percent,
        boss,
        first_kill,
        max_hp,
        max_hit,
        regen_hours,
        rooms,
        ..
    } = drop
    else {
        unreachable!()
    };
    assert_eq!((*percent, *boss, *first_kill), (25, true, true));
    assert_eq!((*max_hp, *max_hit, *regen_hours), (200, 70, Some(100)));
    assert!(!rooms.is_empty());

    // Magical, and the Witchunter is Anti-Magic.
    let broadsword = item(&items, "golden broadsword");
    assert_eq!(broadsword.usability.status, Status::Never);
    assert_eq!(broadsword.usability.reasons, [Reason::Magical]);
}

#[test]
fn a_dwarf_cleric_at_level_10() {
    let pack = pack();
    let cat = Catalogue::build(&pack, CLERIC, DWARF).unwrap();
    let items = at(&cat, 10);

    let flail = item(&items, "black flail");
    assert_eq!(flail.usability.status, Status::Now);
    assert_eq!(placed(flail), [(1, 2258)]);

    // Two-handed blunt: fine for an any-blunt class; Thrag carries it.
    let club = item(&items, "huge darkwood club");
    assert_eq!(club.usability.status, Status::Now);
    assert_eq!(club.str_short, Some(20));
    assert!(
        club.sources
            .iter()
            .any(|s| matches!(s, Source::Drop { name, percent: 5, .. } if name == "Thrag"))
    );

    let katana = item(&items, "katana");
    assert_eq!(katana.usability.status, Status::Never);
    assert_eq!(katana.usability.reasons, [Reason::WeaponType]);
}

#[test]
fn a_half_elf_bard_at_level_10() {
    let pack = pack();
    let cat = Catalogue::build(&pack, BARD, HALF_ELF).unwrap();
    let items = at(&cat, 10);

    // Off-hand, and the Bard fights one-handed, but the class list names the Bard.
    let gauche = item(&items, "jeweled main-gauche");
    assert_eq!(gauche.slot, 12);
    assert_eq!(gauche.usability.status, Status::Now);

    let dagger = item(&items, "curved bone dagger");
    assert_eq!(dagger.usability.status, Status::Later);
    assert_eq!(
        dagger.usability.reasons,
        [Reason::Alignment(Alignment::Evil)]
    );
}

#[test]
fn a_gnome_druid_waits_for_the_woodland_robes() {
    let pack = pack();
    let cat = Catalogue::build(&pack, DRUID, GNOME).unwrap();
    let items = at(&cat, 10);
    let robes = item(&items, "woodland robes");
    assert_eq!(robes.usability.status, Status::Later);
    assert_eq!(robes.usability.reasons, [Reason::Level(20)]);
    assert_eq!(placed(robes), [(3, 214)]);
}

#[test]
fn shops_buy_only_what_is_on_their_list_and_sell_with_markup() {
    let pack = pack();
    let cat = Catalogue::build(&pack, WITCHUNTER, KANG).unwrap();
    let items = at(&cat, 10);
    let sales = |name: &str| {
        let mut v: Vec<_> = item(&items, name)
            .sales
            .iter()
            .map(|s| (s.rooms.clone(), s.price))
            .collect();
        v.sort();
        v
    };
    assert_eq!(
        sales("gilded robes"),
        [(vec![(1, 3303)], 11_160), (vec![(6, 1342)], 11_160)]
    );
    // The Recycler only resells (maxqty 0), but it buys.
    assert_eq!(sales("gold ring"), [(vec![(1, 168), (1, 2337)], 62_000)]);
    let ring = item(&items, "gold ring");
    assert!(
        !ring
            .sources
            .iter()
            .any(|s| matches!(s, Source::Shop { .. }))
    );

    let tunic = item(&items, "scalemail tunic");
    let skalis = tunic
        .sources
        .iter()
        .find_map(|s| match s {
            Source::Shop {
                rooms,
                price,
                room,
                steps,
                ..
            } if rooms.contains(&(1, 306)) => Some((*price, *room, *steps)),
            _ => None,
        })
        .expect("sold at Skali's");
    assert_eq!(skalis, (4750, Some((1, 306)), Some(1)));
}

#[test]
fn monster_sets_for_a_loop_an_area_and_names() {
    let pack = pack();
    let graph = RoomGraph::load(&pack).unwrap();
    let monsters = Monsters::load(&pack).unwrap();
    let loops = megamud_loops(&pack, &graph, &monsters).unwrap();
    let arena_loop = loops.iter().find(|l| l.path.file == "NARNLOO2.mp").unwrap();
    let set = loop_monsters(&pack, &monsters, &arena_loop.stops()).unwrap();
    let arena = pack.room(1, 2150).unwrap().unwrap();
    for m in monsters.spawn_pool(&arena) {
        assert!(set.iter().any(|s| s.number == m.number), "{}", m.name);
    }
    let mut numbers: Vec<_> = set.iter().map(|s| s.number).collect();
    numbers.sort_unstable();
    numbers.dedup();
    assert_eq!(numbers.len(), set.len(), "a monster twice: {set:?}");

    let slums = area_monsters(&pack, &monsters, "Slum Street").unwrap();
    for name in ["half-ogre bodyguard", "orc lieutenant"] {
        assert!(slums.iter().any(|s| s.name == name), "{name}: {slums:?}");
    }

    let (set, unknown) = named_monsters(&monsters, &["minotaur champion", "no such beast"]);
    assert_eq!(set.len(), 1);
    assert!(set[0].boss);
    assert_eq!(unknown, ["no such beast"]);
}
