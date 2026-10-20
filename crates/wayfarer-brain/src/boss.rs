//! Boss status: whether a boss should be up, from the character's own last kill of it and the
//! hours the boss takes to come back (`regenhrs`). Information for the player; it never changes
//! what a loop does (the bot walks through the room and fights what is there).

use wayfarer_proto::BossStatus;

/// The status of boss `name`, which comes back `hours` after a kill, last killed by the
/// character at `killed_at` (unix seconds; `None` if never), at `now`. `up_in_minutes` is
/// absent when it should be up already.
pub(crate) fn boss_status(name: &str, hours: i64, killed_at: Option<i64>, now: i64) -> BossStatus {
    let up_in_minutes = killed_at.and_then(|at| {
        let left = at + hours * 3600 - now;
        (left > 0).then(|| u32::try_from((left + 59) / 60).unwrap_or(u32::MAX))
    });
    BossStatus {
        name: name.to_owned(),
        regen_hours: u32::try_from(hours).unwrap_or(0),
        up_in_minutes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_792_411_200;

    #[test]
    fn a_boss_never_killed_should_be_up() {
        let s = boss_status("leo the quick", 2, None, T0);
        assert_eq!((s.regen_hours, s.up_in_minutes), (2, None));
    }

    #[test]
    fn a_boss_killed_lately_should_be_up_when_its_hours_pass() {
        let kill = Some(T0);
        // Just killed: the whole two hours.
        assert_eq!(boss_status("leo", 2, kill, T0).up_in_minutes, Some(120));
        // Half an hour on, rounded up to whole minutes.
        assert_eq!(
            boss_status("leo", 2, kill, T0 + 1800).up_in_minutes,
            Some(90)
        );
        assert_eq!(
            boss_status("leo", 2, kill, T0 + 1801).up_in_minutes,
            Some(90)
        );
        assert_eq!(
            boss_status("leo", 2, kill, T0 + 7199).up_in_minutes,
            Some(1)
        );
        // Its time has come, or gone.
        assert_eq!(boss_status("leo", 2, kill, T0 + 7200).up_in_minutes, None);
        assert_eq!(boss_status("leo", 2, kill, T0 + 99_999).up_in_minutes, None);
    }
}
