//! The items Item and Key exits ask for that a shop sells (docs/brain.md "Walking"): a room
//! ticket for the Homely Hearth's stairs, a rope and grapple, a wooden skiff. The walker buys
//! one on the way when the best route goes through such an exit ([`crate::route_buying`]).

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::nav::Key;
use crate::spellshop::unit;
use crate::{Error, WorldPack};

const ROOM_SHOP: i64 = 1;

/// An item an exit asks for, and where it is sold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitItem {
    pub number: i64,
    pub name: String,
    /// Uses before it is gone (`items.raw` `charges`): 1 for a room ticket, which the exit
    /// takes; -1 for unlimited.
    pub charges: i64,
    /// The item's cost in copper, before a shop's markup and the Charm discount.
    pub price: u64,
    /// The shop rooms that stock it, by room key.
    pub shops: Vec<Key>,
}

impl ExitItem {
    /// One pass through its exit uses it up.
    pub fn used_up(&self) -> bool {
        self.charges == 1
    }
}

/// The items of `wanted` (by number) that a shop of the pack stocks (`max_qty` above 0), with
/// the rooms of those shops. Items no shop room sells are left out.
pub fn load(pack: &WorldPack, wanted: &BTreeSet<i64>) -> Result<BTreeMap<i64, ExitItem>, Error> {
    let mut shop_rooms: BTreeMap<i64, Vec<Key>> = BTreeMap::new();
    for r in pack.all_rooms()? {
        if r.room_type == ROOM_SHOP && r.shop_num > 0 {
            shop_rooms
                .entry(r.shop_num)
                .or_default()
                .push((r.map, r.room));
        }
    }
    let mut sold: BTreeMap<i64, BTreeSet<Key>> = BTreeMap::new();
    for (number, _) in pack.shop_types()? {
        let Some(rooms) = shop_rooms.get(&number) else {
            continue;
        };
        let Some(shop) = pack.shop(number)? else {
            continue;
        };
        for (item, max_qty) in &shop.stock {
            if *max_qty > 0 && wanted.contains(item) {
                sold.entry(*item).or_default().extend(rooms);
            }
        }
    }
    let mut out = BTreeMap::new();
    for (number, rooms) in sold {
        let Some(item) = pack.item(number)? else {
            continue;
        };
        let v: Value = serde_json::from_str(&item.raw).unwrap_or(Value::Null);
        out.insert(
            number,
            ExitItem {
                number,
                name: item.name,
                charges: v.get("charges").and_then(Value::as_i64).unwrap_or(-1),
                price: u64::try_from(item.cost.max(0)).unwrap_or(0) * unit(item.currency),
                shops: rooms.into_iter().collect(),
            },
        );
    }
    Ok(out)
}
