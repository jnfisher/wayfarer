//! Monsters by name, what a room spawns, and which monsters are worth hunting
//! (`kb/progression.md` "Choosing where to grind", `kb/combat.md`).

use std::collections::{HashMap, HashSet};

use crate::Error;
use crate::pack::{Monster, Room, WorldPack};

/// `montype` of an NPC: shopkeepers, the ferryman, healers.
pub const MONTYPE_NPC: i64 = 2;

/// The room types the spawn pass visits (`kb/progression.md`, `LJNBKGND.C:2466-2620`): Normal,
/// Arena and Lair. Shops get only the start-up fill; a Colliseum, hotel or jail nothing.
const SPAWN_ROOM_TYPES: [i64; 3] = [0, 2, 3];

/// Every known monster, in memory.
pub struct Monsters {
    list: Vec<Monster>,
    by_number: HashMap<i64, usize>,
    /// Lower-cased name to positions in `list`, in number order.
    by_name: HashMap<String, Vec<usize>>,
    /// The numbers of the bosses: monsters that spawn one at a time.
    bosses: HashSet<i64>,
}

/// The four evil alignments of `ALIGNMENTS` (Good, Evil, Chaotic Evil, Neutral, Lawful Good,
/// Neutral Evil, Lawful Evil): Evil, Chaotic Evil, Neutral Evil and Lawful Evil.
pub fn is_evil(alignment: i64) -> bool {
    matches!(alignment, 1 | 2 | 5 | 6)
}

/// A boss: a unique monster, one alive at a time (`maxalive` 1) that comes back `regenhrs`
/// hours after its kill (`kb/progression.md`): the Minotaur Chieftain, the champion. Not an
/// NPC and worth experience: healers, shopkeepers and quest figures are unique too.
fn boss(m: &Monster) -> bool {
    if m.mon_type == MONTYPE_NPC || m.exp <= 0 {
        return false;
    }
    let Ok(raw) = serde_json::from_str::<serde_json::Value>(&m.raw) else {
        return false;
    };
    let field = |f: &str| raw.get(f).and_then(serde_json::Value::as_i64).unwrap_or(0);
    field("maxalive") == 1 && field("regenhrs") > 0
}

/// Worth attacking: evil, not an NPC and worth experience (`kb/combat.md`).
pub fn huntable(m: &Monster) -> bool {
    is_evil(m.alignment) && m.mon_type != MONTYPE_NPC && m.exp > 0
}

impl Monsters {
    pub fn load(pack: &WorldPack) -> Result<Monsters, Error> {
        Ok(Monsters::new(pack.all_monsters()?))
    }

    pub fn new(mut list: Vec<Monster>) -> Monsters {
        list.sort_by_key(|m| m.number);
        let by_number = list
            .iter()
            .enumerate()
            .map(|(i, m)| (m.number, i))
            .collect();
        let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, m) in list.iter().enumerate() {
            by_name.entry(m.name.to_lowercase()).or_default().push(i);
        }
        let bosses = list.iter().filter(|m| boss(m)).map(|m| m.number).collect();
        Monsters {
            list,
            by_number,
            by_name,
            bosses,
        }
    }

    /// Every monster, by number.
    pub fn iter(&self) -> impl Iterator<Item = &Monster> {
        self.list.iter()
    }

    pub fn get(&self, number: i64) -> Option<&Monster> {
        self.by_number.get(&number).map(|&i| &self.list[i])
    }

    /// The monsters a name seen in the game refers to: every record with that name (any case),
    /// else with leading words stripped one at a time until a name matches ("thin giant rat"
    /// finds the giant rats). Several records may share a name.
    pub fn by_name(&self, name: &str) -> Vec<&Monster> {
        let name = name.trim().to_lowercase();
        let mut rest = name.as_str();
        loop {
            if let Some(v) = self.by_name.get(rest) {
                return v.iter().map(|&i| &self.list[i]).collect();
            }
            match rest.split_once(' ') {
                Some((_, tail)) => rest = tail.trim_start(),
                None => return Vec::new(),
            }
        }
    }

    /// What the spawn pass makes in a room (`generate_monster`, `LJNMONST.C:458-620`): nothing
    /// unless the pass visits the room's type and `maxmonsters` > 0; then the room's `genmonster` when it has one, else every
    /// monster with `montype` = `mongroup` and `genindex` within `minlevel` (0 = any) to
    /// `maxlevel` (0 = nothing).
    pub fn generated(&self, room: &Room) -> Vec<&Monster> {
        if room.max_monsters <= 0 || !SPAWN_ROOM_TYPES.contains(&room.room_type) {
            return Vec::new();
        }
        if room.gen_monster != 0 {
            return self.get(room.gen_monster).into_iter().collect();
        }
        if room.max_level == 0 {
            return Vec::new();
        }
        self.list
            .iter()
            .filter(|m| {
                m.mon_type == room.mon_group
                    && (room.min_level == 0 || m.gen_index >= room.min_level)
                    && m.gen_index <= room.max_level
            })
            .collect()
    }

    /// Every monster a room can hold: its own specific monster (`specmon`, made again whenever
    /// a player enters while it is dead) and what the spawn pass makes.
    pub fn spawn_pool(&self, room: &Room) -> Vec<&Monster> {
        let mut pool: Vec<&Monster> = self.get(room.spec_mon).into_iter().collect();
        for m in self.generated(room) {
            if !pool.iter().any(|p| p.number == m.number) {
                pool.push(m);
            }
        }
        pool
    }

    /// A boss: one alive at a time, back hours after its kill.
    pub fn is_boss(&self, number: i64) -> bool {
        self.bosses.contains(&number)
    }

    /// The bosses a room's pool holds, if any: the room is their lair. Only bosses count, not
    /// every Lair room (John, 2026-10-20); a MegaMud waypoint flagged boss is the other way a
    /// room becomes one ([`crate::Finder::boss_lair`]).
    pub fn boss_lair(&self, room: &Room) -> Option<Vec<&Monster>> {
        let bosses: Vec<&Monster> = self
            .spawn_pool(room)
            .into_iter()
            .filter(|m| self.is_boss(m.number))
            .collect();
        (!bosses.is_empty()).then_some(bosses)
    }

    /// A room whose pool holds a monster worth hunting.
    pub fn hunt_room(&self, room: &Room) -> bool {
        self.spawn_pool(room).into_iter().any(huntable)
    }
}

/// What the bot does about a monster, by name (docs/plan/04-settings.md "Monsters and
/// players"; MegaMud's relationships, `kb/megamud-manual.md` 9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MonsterRule {
    /// Never attack it; fight back if it attacks.
    NeverFight,
    /// Leave the room as soon as it is seen, whatever the HP; fight only if there is no way
    /// out and it attacks.
    FleeFrom,
    /// Do not walk through the room it was last seen in when there is another way.
    Avoid,
}

/// The rules every character starts with, each with why. Settings (M4) layer over them.
/// Full names only: a pattern matches any name that holds it, so `ooze` would also match the
/// glittering ooze, which is fightable.
pub const DEFAULT_RULES: &[(&str, MonsterRule, &str)] = &[(
    "gigantic black ooze",
    MonsterRule::FleeFrom,
    // Monster 63: 2,500 HP, regenerates 500 a round, damage reduction 40. Every swing of a
    // mid-level character glances off its armour: a fight that never ends (the M3 soak,
    // 2026-10-20). MegaMud's manual uses it as the example of its Flee class.
    "unfightable: its armour takes every blow and it regenerates 500 HP a round",
)];

/// Rules by monster name, matched as MegaMud does: a pattern matches a name that holds it, in
/// any case, and the longest matching pattern wins.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MonsterRules {
    /// Lower-cased patterns, longest first; one rule per pattern.
    list: Vec<(String, MonsterRule)>,
}

impl MonsterRules {
    /// Rules from `(pattern, rule)` pairs; a pattern given again (any case) replaces the rule
    /// before it, so later layers win. Empty patterns are left out.
    pub fn new<S: AsRef<str>>(pairs: impl IntoIterator<Item = (S, MonsterRule)>) -> Self {
        let mut list: Vec<(String, MonsterRule)> = Vec::new();
        for (pattern, rule) in pairs {
            let pattern = pattern.as_ref().trim().to_lowercase();
            if pattern.is_empty() {
                continue;
            }
            match list.iter_mut().find(|(p, _)| *p == pattern) {
                Some(entry) => entry.1 = rule,
                None => list.push((pattern, rule)),
            }
        }
        // Longest first; equal lengths in the order given.
        list.sort_by_key(|(p, _)| std::cmp::Reverse(p.chars().count()));
        MonsterRules { list }
    }

    /// The default list ([`DEFAULT_RULES`]).
    pub fn defaults() -> Self {
        MonsterRules::new(DEFAULT_RULES.iter().map(|&(p, r, _)| (p, r)))
    }

    /// The rule for a monster `name` as the game shows it (`large giant rat`), if one matches.
    pub fn rule_for(&self, name: &str) -> Option<MonsterRule> {
        let name = name.to_lowercase();
        self.list
            .iter()
            .find(|(p, _)| name.contains(p.as_str()))
            .map(|&(_, r)| r)
    }

    /// The patterns and their rules, longest first.
    pub fn iter(&self) -> impl Iterator<Item = (&str, MonsterRule)> {
        self.list.iter().map(|(p, r)| (p.as_str(), *r))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ooze_is_flee_from_by_default_and_only_by_its_full_name() {
        let rules = MonsterRules::defaults();
        assert_eq!(
            rules.rule_for("gigantic black ooze"),
            Some(MonsterRule::FleeFrom)
        );
        assert_eq!(
            rules.rule_for("Gigantic Black Ooze"),
            Some(MonsterRule::FleeFrom)
        );
        assert_eq!(
            rules.rule_for("large gigantic black ooze"),
            Some(MonsterRule::FleeFrom)
        );
        assert_eq!(rules.rule_for("ooze"), None);
        assert_eq!(rules.rule_for("glittering ooze"), None);
    }

    #[test]
    fn the_longest_matching_pattern_wins_in_any_case() {
        let rules = MonsterRules::new([
            ("rat", MonsterRule::Avoid),
            ("Giant Rat", MonsterRule::NeverFight),
            ("orc", MonsterRule::FleeFrom),
        ]);
        assert_eq!(
            rules.rule_for("fat GIANT rat"),
            Some(MonsterRule::NeverFight)
        );
        assert_eq!(rules.rule_for("rat"), Some(MonsterRule::Avoid));
        assert_eq!(rules.rule_for("pirate"), Some(MonsterRule::Avoid));
        assert_eq!(rules.rule_for("kobold"), None);
        let lengths: Vec<usize> = rules.iter().map(|(p, _)| p.len()).collect();
        assert_eq!(lengths, [9, 3, 3]);
    }

    #[test]
    fn a_later_rule_for_the_same_pattern_replaces_the_earlier() {
        let rules = MonsterRules::new([
            ("gigantic black ooze", MonsterRule::FleeFrom),
            ("GIGANTIC black ooze ", MonsterRule::NeverFight),
            ("", MonsterRule::Avoid),
        ]);
        assert_eq!(rules.iter().count(), 1);
        assert_eq!(
            rules.rule_for("gigantic black ooze"),
            Some(MonsterRule::NeverFight)
        );
        assert_eq!(rules.rule_for("anything"), None);
    }

    fn monster(number: i64, name: &str, mon_type: i64, gen_index: i64) -> Monster {
        Monster {
            number,
            name: name.into(),
            level: 1,
            exp: 9,
            max_hp: 5,
            mon_type,
            gen_index,
            alignment: 2,
            raw: String::new(),
        }
    }

    fn room(gen_monster: i64, max_monsters: i64, group: i64, levels: (i64, i64)) -> Room {
        Room {
            map: 1,
            room: 1,
            name: "R".into(),
            light: 0,
            room_type: 0,
            shop_num: 0,
            min_level: levels.0,
            max_level: levels.1,
            gen_monster,
            max_monsters,
            mon_group: group,
            regen_delay: 0,
            spec_mon: 0,
            flags: 0,
            mm_id: 0,
        }
    }

    #[test]
    fn genmonster_replaces_the_group_and_maxlevel_zero_makes_nothing() {
        let ms = Monsters::new(vec![
            monster(1, "giant rat", 6, 1),
            monster(2, "orc", 6, 4),
            monster(3, "troll", 7, 1),
        ]);
        let nums = |r: &Room| ms.generated(r).iter().map(|m| m.number).collect::<Vec<_>>();
        assert_eq!(nums(&room(0, 3, 6, (1, 3))), [1]);
        assert_eq!(nums(&room(0, 3, 6, (0, 9))), [1, 2]);
        assert_eq!(nums(&room(3, 3, 6, (1, 3))), [3]);
        assert!(nums(&room(0, 3, 6, (0, 0))).is_empty());
        assert!(nums(&room(0, 0, 6, (1, 3))).is_empty());
        // Normal, Arena and Lair rooms only: not a shop or a Colliseum.
        for (room_type, spawns) in [(0, true), (1, false), (2, true), (3, true), (5, false)] {
            let mut r = room(0, 3, 6, (1, 3));
            r.room_type = room_type;
            assert_eq!(
                !ms.generated(&r).is_empty(),
                spawns,
                "room type {room_type}"
            );
        }
        let mut shop = room(0, 1, 2, (0, 0));
        shop.spec_mon = 3;
        assert_eq!(ms.spawn_pool(&shop).len(), 1);
    }

    #[test]
    fn a_boss_spawns_one_at_a_time_and_comes_back_hours_later() {
        let mut chief = monster(114, "minotaur chieftain", 11, 0);
        chief.raw = r#"{"maxalive":1,"regenhrs":24}"#.into();
        let mut once = monster(465, "Champion Gudruk", 11, 0);
        once.raw = r#"{"maxalive":1,"regenhrs":0}"#.into();
        let mut healer = monster(160, "healer", MONTYPE_NPC, 0);
        healer.raw = r#"{"maxalive":1,"regenhrs":1}"#.into();
        let mut merchant = monster(161, "dwarven barkeep", 24, 0);
        merchant.raw = healer.raw.clone();
        merchant.exp = 0;
        let ms = Monsters::new(vec![
            chief,
            once,
            healer,
            merchant,
            monster(111, "minotaur", 11, 4),
        ]);
        assert!(ms.is_boss(114));
        for n in [465, 160, 161, 111] {
            assert!(!ms.is_boss(n), "{n}");
        }
        let lair = room(114, 1, 11, (4, 4));
        assert_eq!(ms.boss_lair(&lair).unwrap()[0].name, "minotaur chieftain");
        assert!(ms.boss_lair(&room(0, 5, 11, (4, 4))).is_none());
        let mut spec = room(0, 5, 11, (4, 4));
        spec.spec_mon = 114;
        assert!(ms.boss_lair(&spec).is_some());
    }

    #[test]
    fn names_match_with_leading_words_stripped() {
        let ms = Monsters::new(vec![
            monster(1, "giant rat", 6, 1),
            monster(9, "Giant Rat", 6, 1),
        ]);
        assert_eq!(ms.by_name("thin giant rat").len(), 2);
        assert_eq!(ms.by_name("giant rat").len(), 2);
        assert!(ms.by_name("rat").is_empty());
        assert!(ms.by_name("orc").is_empty());
    }
}
