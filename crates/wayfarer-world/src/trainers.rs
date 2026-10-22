//! Trainers: the shops of type Training where `train` takes a character up a level
//! (`kb/progression.md` "Training").
//!
//! A trainer takes a character when its `trainclass` is 0 (any) or the character's class, and
//! the character's *next* level is within its `minlevel..=maxlevel`. The fee is `level × 5`
//! silver times `(markup + 100) / 100`, in whole silver, Charm playing no part.

use std::collections::BTreeMap;

use crate::Error;
use crate::nav::Key;
use crate::pack::WorldPack;

/// Shop type of a trainer, and of the room a shop is.
const SHOP_TRAINING: i64 = 8;
const ROOM_SHOP: i64 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trainer {
    pub shop: i64,
    pub name: String,
    pub min_level: i64,
    pub max_level: i64,
    /// Per cent over the base fee.
    pub markup: i64,
    /// The class number it trains; 0 is any.
    pub class: i64,
    pub rooms: Vec<Key>,
}

impl Trainer {
    /// What `train` costs a character of `level` (going to `level + 1`), in copper.
    pub fn fee(&self, level: i64) -> u64 {
        let silver = u64::try_from(level.max(1) * 5).unwrap_or(5)
            * u64::try_from(self.markup + 100).unwrap_or(100)
            / 100;
        silver * 10
    }

    /// Whether it trains a character of `class` and `level` to the next level.
    pub fn takes(&self, class: Option<i64>, level: i64) -> bool {
        let next = level + 1;
        (self.class == 0 || Some(self.class) == class)
            && self.min_level <= next
            && next <= self.max_level
    }
}

/// Every trainer that has a room to walk to, the sysop's apart.
pub fn load(pack: &WorldPack) -> Result<Vec<Trainer>, Error> {
    let mut rooms: BTreeMap<i64, Vec<Key>> = BTreeMap::new();
    for r in pack.all_rooms()? {
        if r.room_type == ROOM_SHOP && r.shop_num > 0 {
            rooms.entry(r.shop_num).or_default().push((r.map, r.room));
        }
    }
    let mut out = Vec::new();
    for (number, kind) in pack.shop_types()? {
        if kind != SHOP_TRAINING {
            continue;
        }
        let Some(shop) = pack.shop(number)? else {
            continue;
        };
        let rooms = rooms.get(&number).cloned().unwrap_or_default();
        if rooms.is_empty() || shop.name.to_lowercase().contains("sysop") {
            continue;
        }
        out.push(Trainer {
            shop: number,
            name: shop.name,
            min_level: shop.min_level,
            max_level: shop.max_level,
            markup: shop.markup,
            class: shop.train_class,
            rooms,
        });
    }
    Ok(out)
}

/// The trainer to go to: of those that take the character, the cheapest, then the nearest;
/// `reach` gives the steps to the nearest of a trainer's rooms and which it is. `None` when no
/// trainer takes the character or none can be reached.
pub fn choose(
    trainers: &[Trainer],
    class: Option<i64>,
    level: i64,
    reach: impl Fn(&[Key]) -> Option<(u32, Key)>,
) -> Option<(&Trainer, Key, u32)> {
    trainers
        .iter()
        .filter(|t| t.takes(class, level))
        .filter_map(|t| reach(&t.rooms).map(|(steps, room)| (t, room, steps)))
        .min_by_key(|(t, _, steps)| (t.markup, *steps, t.shop))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trainer(shop: i64, class: i64, lo: i64, hi: i64, markup: i64, room: i64) -> Trainer {
        Trainer {
            shop,
            name: format!("T{shop}"),
            min_level: lo,
            max_level: hi,
            markup,
            class,
            rooms: vec![(1, room)],
        }
    }

    fn near(rooms: &[Key]) -> Option<(u32, Key)> {
        rooms.first().map(|&k| (u32::try_from(k.1).unwrap_or(0), k))
    }

    #[test]
    fn the_fee_is_five_silver_a_level_with_the_markup() {
        let universal = trainer(1, 0, 1, 10, 0, 5);
        let class = trainer(2, 4, 1, 10, 300, 5);
        // Level 1: 5 silver at the universal trainer, 20 at a class trainer.
        assert_eq!((universal.fee(1), class.fee(1)), (50, 200));
        assert_eq!(universal.fee(6), 300);
    }

    #[test]
    fn a_trainer_takes_the_next_level_of_its_class_or_any() {
        let t = trainer(2, 13, 1, 10, 300, 5);
        // A level 10 character trains to 11: outside 1..=10.
        assert!(t.takes(Some(13), 9));
        assert!(!t.takes(Some(13), 10));
        // Another class's trainer.
        assert!(!t.takes(Some(12), 5));
        // Any class.
        assert!(trainer(1, 0, 1, 10, 0, 5).takes(Some(12), 5));
        assert!(!trainer(1, 0, 12, 20, 0, 5).takes(Some(12), 5));
    }

    #[test]
    fn the_cheapest_trainer_that_takes_it_then_the_nearest() {
        let ts = [
            trainer(1, 13, 1, 10, 300, 3),
            trainer(2, 0, 1, 10, 0, 50),
            trainer(3, 0, 1, 10, 0, 20),
            trainer(4, 0, 12, 20, 0, 1),
        ];
        let (t, room, steps) = choose(&ts, Some(13), 6, near).unwrap();
        // The two universal trainers cost nothing extra: the nearer (20), not the class one.
        assert_eq!((t.shop, room, steps), (3, (1, 20), 20));
        // None takes a level 30 character; none is reachable.
        assert!(choose(&ts, Some(13), 30, near).is_none());
        assert!(choose(&ts, Some(13), 6, |_| None).is_none());
    }
}
