//! How fast a character gets its HP and mana back while resting (`kb/recovery.md` "Resting" and
//! "Background cycles", from `slow_update_character` LJNBKGND.C:1656-1716): for a party rest,
//! to tell whether a heal cast now shortens the rest or only moves the wait onto the healer's
//! mana. Gear and abilities that alter the rates ("Alter healing rate", "Alter Mana
//! Regeneration") are not known here, so the rates are the base ones.

/// The rates, per hour; `None` where the stats they need are not known yet.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Regen {
    /// HP per hour while resting.
    pub hp: Option<i64>,
    /// Mana per hour (resting or not).
    pub mana: Option<i64>,
}

/// Seconds in an hour.
const HOUR: i64 = 3600;

/// HP per hour while resting at `level` with `health`: one unit per 30-s slow tick, and three
/// units more every 21st second of rest, a unit being `max((level+20)*health/750, 1)`.
pub(crate) fn hp_per_hour(level: i64, health: i64) -> i64 {
    let unit = ((level + 20) * health / 750).max(1);
    unit * HOUR / 30 + 3 * unit * HOUR / 21
}

/// Mana per hour on the slow tick: `stat*(level+20)*(magery level+2)/1650` each 30 s, the
/// stat by magery (1 Mage: Int, 2 Priest: Wil, 3 Druid: (Int+Wil)/2, 4 Bard: Cha); a Kai
/// (5) gets one a tick. `None` for no magery or a stat not known.
pub(crate) fn mana_per_hour(
    magery: i64,
    magery_level: i64,
    level: i64,
    stats: &wayfarer_proto::Stats,
) -> Option<i64> {
    let stat = |s: Option<u32>| s.map(i64::from);
    let castval = match magery {
        1 => stat(stats.intellect)?,
        2 => stat(stats.willpower)?,
        3 => (stat(stats.intellect)? + stat(stats.willpower)?) / 2,
        4 => stat(stats.charm)?,
        5 => return Some(HOUR / 30),
        _ => return None,
    };
    Some(castval * (level + 20) * (magery_level + 2) / 1650 * HOUR / 30)
}

/// Seconds to get `short` points back at `per_hour`; `None` when it never comes back.
pub(crate) fn wait_secs(short: i64, per_hour: i64) -> Option<i64> {
    if short <= 0 {
        return Some(0);
    }
    (per_hour > 0).then(|| (short * HOUR + per_hour - 1) / per_hour)
}

/// One member of a resting party, for [`shortens`]: the HP it is short of its rest-until line
/// and its resting rate, its mana wait before and after the cast (the caster's changes), and
/// whether the cast heals it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Waiting {
    pub hp_short: i64,
    pub hp_rate: Option<i64>,
    pub mana_before: Option<i64>,
    pub mana_after: Option<i64>,
    pub healed: bool,
}

/// Whether a heal of `heal` points cast now leaves the party rest no longer than it is: the
/// rest lasts until the slowest member has its HP and its mana back. `None` when a rate is not
/// known.
pub(crate) fn shortens(members: &[Waiting], heal: i64) -> Option<bool> {
    let mut before = 0;
    let mut after = 0;
    for m in members {
        let rate = m.hp_rate?;
        let short_after = if m.healed {
            (m.hp_short - heal).max(0)
        } else {
            m.hp_short
        };
        before = before.max(wait_secs(m.hp_short, rate)?.max(m.mana_before?));
        after = after.max(wait_secs(short_after, rate)?.max(m.mana_after?));
    }
    Some(after <= before)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(hp_short: i64, mana: (i64, i64), healed: bool) -> Waiting {
        Waiting {
            hp_short,
            hp_rate: Some(600),
            mana_before: Some(mana.0),
            mana_after: Some(mana.1),
            healed,
        }
    }

    #[test]
    fn a_heal_pays_while_hp_is_the_wait() {
        // Lauren is 60 HP short (6 minutes at 600 an hour); the healer's mana would take two
        // minutes more after the cast, one before: the rest gets shorter.
        let party = [member(0, (60, 120), false), member(60, (0, 0), true)];
        assert_eq!(shortens(&party, 30), Some(true));
        // The cast would leave the healer's mana 10 minutes away: the rest gets longer.
        let party = [member(0, (60, 600), false), member(60, (0, 0), true)];
        assert_eq!(shortens(&party, 30), Some(false));
        // Someone else is the wait (Faye, 90 HP short, 9 minutes): healing Lauren does not
        // shorten it yet, and does not lengthen it either.
        let party = [
            member(0, (60, 300), false),
            member(60, (0, 0), true),
            member(90, (0, 0), false),
        ];
        assert_eq!(shortens(&party, 30), Some(true));
        let unknown = Waiting {
            hp_rate: None,
            ..member(10, (0, 0), true)
        };
        assert_eq!(shortens(&[unknown], 30), None);
    }

    #[test]
    fn hp_at_level_one_is_about_ten_a_minute() {
        // kb/recovery.md: a resting level-1 character gains about 3 HP per 21 s plus 1 per 30 s.
        let per_hour = hp_per_hour(1, 50);
        assert_eq!(per_hour, 120 + 514);
        assert!((600..700).contains(&per_hour));
        // A unit grows with level and health: level 30, health 75 is 5 HP a unit.
        assert_eq!(hp_per_hour(30, 75), 5 * 120 + 15 * 3600 / 21);
    }

    #[test]
    fn mana_by_magery() {
        let stats = wayfarer_proto::Stats {
            intellect: Some(60),
            willpower: Some(90),
            charm: Some(40),
            ..Default::default()
        };
        // A level-10 Priest (magery 2, level 3): 90*30*5/1650 = 8 a tick.
        assert_eq!(mana_per_hour(2, 3, 10, &stats), Some(8 * 120));
        // A Druid uses the mean of Int and Wil: 75*30*5/1650 = 6.
        assert_eq!(mana_per_hour(3, 3, 10, &stats), Some(6 * 120));
        assert_eq!(mana_per_hour(5, 1, 10, &stats), Some(120));
        assert_eq!(mana_per_hour(0, 0, 10, &stats), None);
        let unknown = wayfarer_proto::Stats::default();
        assert_eq!(mana_per_hour(1, 3, 10, &unknown), None);
    }

    #[test]
    fn waits() {
        assert_eq!(wait_secs(0, 100), Some(0));
        assert_eq!(wait_secs(10, 3600), Some(10));
        assert_eq!(wait_secs(1, 7200), Some(1));
        assert_eq!(wait_secs(10, 0), None);
    }
}
