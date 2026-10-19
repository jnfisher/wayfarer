//! Monsters by name, what a room spawns, and which monsters are worth hunting
//! (`kb/progression.md` "Choosing where to grind", `kb/combat.md`).

use std::collections::HashMap;

use crate::Error;
use crate::pack::{Monster, Room, WorldPack};

/// `montype` of an NPC: shopkeepers, the ferryman, healers.
pub const MONTYPE_NPC: i64 = 2;

/// Every known monster, in memory.
pub struct Monsters {
    list: Vec<Monster>,
    by_number: HashMap<i64, usize>,
    /// Lower-cased name to positions in `list`, in number order.
    by_name: HashMap<String, Vec<usize>>,
}

/// The four evil alignments of `ALIGNMENTS` (Good, Evil, Chaotic Evil, Neutral, Lawful Good,
/// Neutral Evil, Lawful Evil): Evil, Chaotic Evil, Neutral Evil and Lawful Evil.
pub fn is_evil(alignment: i64) -> bool {
    matches!(alignment, 1 | 2 | 5 | 6)
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
        Monsters {
            list,
            by_number,
            by_name,
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
    /// unless `maxmonsters` > 0; then the room's `genmonster` when it has one, else every
    /// monster with `montype` = `mongroup` and `genindex` within `minlevel` (0 = any) to
    /// `maxlevel` (0 = nothing).
    pub fn generated(&self, room: &Room) -> Vec<&Monster> {
        if room.max_monsters <= 0 {
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

    /// A room whose pool holds a monster worth hunting.
    pub fn hunt_room(&self, room: &Room) -> bool {
        self.spawn_pool(room).into_iter().any(huntable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut shop = room(0, 1, 2, (0, 0));
        shop.spec_mon = 3;
        assert_eq!(ms.spawn_pool(&shop).len(), 1);
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
