//! Spells for sale: the scrolls shops stock, which a class can learn, and what they cost
//! (`kb/spells.md` "Learning", `kb/economy.md` "Prices" and "Shops").
//!
//! A scroll is an item of type 9 with ability 42, "Link to Spell", whose value is the spell's
//! number. A shop sells it when the slot's `maxqty` is above 0 (a 0 slot only resells what
//! players sold). A character can read it when the spell's magery is the class's, its magery
//! level is at most the class's, and its level is at most the character's (alignment is not
//! checked here).

use std::collections::{BTreeMap, HashMap, HashSet};

use serde_json::Value;

use crate::Error;
use crate::nav::Key;
use crate::pack::WorldPack;

/// Item type of a scroll, and the ability that links it to a spell.
const ITEM_SCROLL: i64 = 9;
const ABILITY_LINK_SPELL: i64 = 42;
/// Room type of a shop.
const ROOM_SHOP: i64 = 1;

/// One scroll a shop sells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScrollOffer {
    pub spell: i64,
    pub spell_name: String,
    /// The character level the spell needs.
    pub spell_level: i64,
    pub magery: i64,
    pub magery_level: i64,
    pub scroll: String,
    /// The item's cost in its currency (0 copper, 1 silver, 2 gold, 3 platinum, 4 runic).
    pub cost: i64,
    pub currency: i64,
    pub shop: i64,
    pub shop_name: String,
    /// The shop's markup, per cent.
    pub markup: i64,
    /// The rooms that are the shop.
    pub rooms: Vec<Key>,
}

impl ScrollOffer {
    /// What buying it costs a character of `charm`, in copper: the item's cost in copper, plus
    /// the markup, then the Charm discount (`kb/economy.md` "Prices"), each step truncating. With
    /// the Charm not known, a Charm of 50 (no discount, no surcharge) is assumed.
    pub fn price(&self, charm: Option<i64>) -> u64 {
        let copper = u64::try_from(self.cost.max(0)).unwrap_or(0) * unit(self.currency);
        let marked = copper * u64::try_from(100 + self.markup.max(0)).unwrap_or(100) / 100;
        let factor = u64::try_from(110 - charm.unwrap_or(50) / 5).unwrap_or(100);
        marked * factor / 100
    }
}

/// Copper in one coin of a currency.
pub fn unit(currency: i64) -> u64 {
    match currency {
        1 => 10,
        2 => 100,
        3 => 10_000,
        4 => 1_000_000,
        _ => 1,
    }
}

/// Every scroll any shop sells, for the spells the pack knows. Built once from the pack.
pub fn load(pack: &WorldPack) -> Result<Vec<ScrollOffer>, Error> {
    let spells: HashMap<i64, (String, i64, i64, i64)> = pack
        .spells()?
        .into_iter()
        .map(|s| {
            let v: Value = serde_json::from_str(&s.raw).unwrap_or(Value::Null);
            let get = |k: &str| v.get(k).and_then(Value::as_i64).unwrap_or(0);
            (
                s.number,
                (s.name.clone(), s.level, get("magery"), get("magelvl")),
            )
        })
        .collect();
    let mut shop_rooms: BTreeMap<i64, Vec<Key>> = BTreeMap::new();
    for r in pack.all_rooms()? {
        if r.room_type == ROOM_SHOP && r.shop_num > 0 {
            shop_rooms
                .entry(r.shop_num)
                .or_default()
                .push((r.map, r.room));
        }
    }
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for (number, _) in pack.shop_types()? {
        let Some(shop) = pack.shop(number)? else {
            continue;
        };
        for (item, max_qty) in &shop.stock {
            if *max_qty <= 0 {
                continue;
            }
            let Some(item) = pack.item(*item)?.filter(|i| i.item_type == ITEM_SCROLL) else {
                continue;
            };
            let Some(spell) = linked_spell(&item.raw) else {
                continue;
            };
            let Some((name, level, magery, magery_level)) = spells.get(&spell) else {
                continue;
            };
            if !seen.insert((shop.number, spell)) {
                continue;
            }
            out.push(ScrollOffer {
                spell,
                spell_name: name.clone(),
                spell_level: *level,
                magery: *magery,
                magery_level: *magery_level,
                scroll: item.name,
                cost: item.cost,
                currency: item.currency,
                shop: shop.number,
                shop_name: shop.name.clone(),
                markup: shop.markup,
                rooms: shop_rooms.get(&shop.number).cloned().unwrap_or_default(),
            });
        }
    }
    Ok(out)
}

/// The spell an item's record links to (ability 42's value).
fn linked_spell(raw: &str) -> Option<i64> {
    let v: Value = serde_json::from_str(raw).ok()?;
    let at = v
        .get("abilities")?
        .as_array()?
        .iter()
        .position(|a| a.as_i64() == Some(ABILITY_LINK_SPELL))?;
    v.get("abilvals")?.as_array()?.get(at)?.as_i64()
}

/// Who is buying.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Learner {
    pub magery: i64,
    pub magery_level: i64,
    pub level: i64,
    pub charm: Option<i64>,
}

/// A spell the character does not know, and the best place to buy it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanItem {
    pub spell: String,
    pub spell_level: i64,
    pub scroll: String,
    pub shop: String,
    /// The shop's room nearest the character.
    pub room: Key,
    /// In copper.
    pub price: u64,
    /// Steps to the shop; `None` when there is no way.
    pub steps: Option<u32>,
    /// The character's level is high enough to read it now.
    pub usable: bool,
    /// The cash on hand covers it, cheapest first, with those before it.
    pub affordable: bool,
}

/// The spells the class could learn that the character lacks, each at its best shop: usable now
/// first (cheapest first, those the cash covers marked), then those for a higher level. A spell
/// for sale nowhere is not listed. `known` holds spell names, any case; `reach` gives the steps
/// to the nearest of a shop's rooms and which it is.
pub fn plan(
    offers: &[ScrollOffer],
    who: Learner,
    known: &[String],
    cash: u64,
    reach: impl Fn(&[Key]) -> Option<(u32, Key)>,
) -> Vec<PlanItem> {
    let known: HashSet<String> = known.iter().map(|k| k.to_lowercase()).collect();
    // The best offer of each spell: the cheapest, then the nearest.
    let mut best: BTreeMap<i64, PlanItem> = BTreeMap::new();
    for o in offers {
        if o.magery != who.magery
            || o.magery_level > who.magery_level
            || known.contains(&o.spell_name.to_lowercase())
        {
            continue;
        }
        let (steps, room) = match reach(&o.rooms) {
            Some((n, k)) => (Some(n), k),
            None => match o.rooms.first() {
                Some(&k) => (None, k),
                None => continue,
            },
        };
        let item = PlanItem {
            spell: o.spell_name.clone(),
            spell_level: o.spell_level,
            scroll: o.scroll.clone(),
            shop: o.shop_name.clone(),
            room,
            price: o.price(who.charm),
            steps,
            usable: o.spell_level <= who.level,
            affordable: false,
        };
        let better = |new: &PlanItem, old: &PlanItem| {
            let key = |p: &PlanItem| (p.steps.is_none(), p.price, p.steps);
            key(new) < key(old)
        };
        match best.get(&o.spell) {
            Some(old) if !better(&item, old) => {}
            _ => {
                best.insert(o.spell, item);
            }
        }
    }
    let mut items: Vec<PlanItem> = best.into_values().collect();
    items.sort_by(|a, b| {
        (!a.usable, a.price, &a.spell, a.spell_level).cmp(&(
            !b.usable,
            b.price,
            &b.spell,
            b.spell_level,
        ))
    });
    let mut left = cash;
    for item in items.iter_mut().filter(|i| i.usable) {
        if item.price <= left {
            left -= item.price;
            item.affordable = true;
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offer(
        spell: i64,
        name: &str,
        level: i64,
        cost: i64,
        currency: i64,
        shop: &str,
    ) -> ScrollOffer {
        ScrollOffer {
            spell,
            spell_name: name.into(),
            spell_level: level,
            magery: 1,
            magery_level: 1,
            scroll: format!("scroll of {name}"),
            cost,
            currency,
            shop: 1,
            shop_name: shop.into(),
            markup: 0,
            rooms: vec![(1, spell)],
        }
    }

    const MAGE: Learner = Learner {
        magery: 1,
        magery_level: 2,
        level: 5,
        charm: None,
    };

    fn near(rooms: &[Key]) -> Option<(u32, Key)> {
        rooms.first().map(|&k| (u32::try_from(k.1).unwrap_or(0), k))
    }

    #[test]
    fn a_price_is_the_cost_in_copper_with_markup_then_charm() {
        let mut o = offer(1, "mend", 3, 8, 2, "Shop");
        // 8 gold = 800 copper; no markup; Charm 50 is 100%.
        assert_eq!(o.price(Some(50)), 800);
        // Charm 30 is 104%.
        assert_eq!(o.price(Some(30)), 832);
        // A markup of 200 per cent triples it first.
        o.markup = 200;
        assert_eq!(o.price(Some(50)), 2400);
        // Free stays free; copper is copper.
        assert_eq!(offer(1, "x", 1, 0, 0, "S").price(None), 0);
        assert_eq!(offer(1, "x", 1, 7, 0, "S").price(Some(50)), 7);
    }

    #[test]
    fn the_plan_lists_what_the_class_lacks_cheapest_first_and_marks_what_cash_covers() {
        let mut other_class = offer(9, "smite", 2, 1, 2, "Mage Shop");
        other_class.magery = 2;
        let offers = [
            offer(1, "mend", 3, 8, 2, "Far Shop"),
            offer(1, "mend", 3, 8, 2, "Near Shop"),
            offer(2, "magic missile", 1, 0, 0, "Near Shop"),
            offer(3, "illuminate", 2, 4, 2, "Near Shop"),
            offer(4, "firebolt", 9, 50, 2, "Near Shop"),
            offer(5, "blur", 1, 1, 2, "Near Shop"),
            other_class,
        ];
        // The first mend shop is farther (a higher room number = more steps in `near`).
        let mut offers = offers.to_vec();
        offers[0].rooms = vec![(1, 90)];
        let known = ["Blur".to_owned()];
        // 5 gold: magic missile (free) and illuminate (4 gold); mend (8 gold) is out of reach.
        let plan = plan(&offers, MAGE, &known, 500, near);
        let rows: Vec<_> = plan
            .iter()
            .map(|p| {
                (
                    p.spell.as_str(),
                    p.shop.as_str(),
                    p.price,
                    p.usable,
                    p.affordable,
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                ("magic missile", "Near Shop", 0, true, true),
                ("illuminate", "Near Shop", 400, true, true),
                ("mend", "Near Shop", 800, true, false),
                ("firebolt", "Near Shop", 5000, false, false),
            ]
        );
    }

    #[test]
    fn a_spell_nobody_reaches_or_sells_is_still_listed_when_a_shop_has_it() {
        let offers = [offer(1, "mend", 3, 8, 2, "Hidden Shop")];
        let plan = plan(&offers, MAGE, &[], 0, |_| None);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].steps, None);
        assert!(!plan[0].affordable);
    }
}
