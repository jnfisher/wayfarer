//! The character model: what the game has shown of one character, from its typed events
//! (docs/plan/03-engine.md section 2), and the merge patch between two snapshots.

use serde_json::{Map, Value};
use wayfarer_parse::Event;
use wayfarer_parse::helpers::Carried;
use wayfarer_parse::typed::{GameEvent, PromptForm, Rest};
use wayfarer_proto::{CharacterState, CoinCount, Coins, Encumbrance, ParseCounts, Stats};

/// One character as the brain knows it. [`Character::snapshot`] is what goes on the wire; the
/// items are kept here with their slots and counts too, though the snapshot has only names.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Character {
    state: CharacterState,
    /// The last line counted in `parse.lines`.
    last_line: Option<u64>,
    pub items: Vec<Carried>,
}

fn u32_of(n: Option<i64>) -> Option<u32> {
    n.and_then(|n| u32::try_from(n).ok())
}

fn u64_of(n: Option<i64>) -> Option<u64> {
    n.and_then(|n| u64::try_from(n).ok())
}

fn i32_of(n: Option<i64>) -> Option<i32> {
    n.and_then(|n| i32::try_from(n).ok())
}

/// Set `slot` to `value` when there is one.
fn set<T>(slot: &mut Option<T>, value: Option<T>) {
    if value.is_some() {
        *slot = value;
    }
}

/// Whether `who`, as `drops_unconscious` names someone (by first name), is the character
/// called `name`.
fn is_me(name: Option<&str>, who: &str) -> bool {
    name.is_some_and(|name| name == who || name.split_whitespace().next() == Some(who))
}

fn form_name(form: PromptForm, has_mana: bool) -> &'static str {
    match form {
        PromptForm::Std if has_mana => "hp_mana",
        PromptForm::Std => "hp",
        PromptForm::Custom => "custom",
        PromptForm::Generic => "generic",
        PromptForm::Off => "off",
    }
}

impl Character {
    pub fn new() -> Self {
        Self::default()
    }

    /// Count `ev`'s line, and apply what it says. Whether the snapshot changed.
    pub fn apply_event(&mut self, ev: &Event) -> bool {
        let before = self.state.clone();
        if ev.name != "prompt" && self.last_line.is_none_or(|n| ev.line_no > n) {
            self.last_line = Some(ev.line_no);
            self.parse_counts().lines += 1;
        }
        let typed = GameEvent::from_event(ev);
        if matches!(typed, GameEvent::Unknown { .. }) {
            self.parse_counts().unclassified += 1;
        }
        self.apply(&typed);
        self.state != before
    }

    /// The parser started again (its line numbers with it): count its lines from scratch.
    pub fn parser_reset(&mut self) {
        self.last_line = None;
    }

    fn parse_counts(&mut self) -> &mut ParseCounts {
        self.state.parse.get_or_insert_with(ParseCounts::default)
    }

    fn status(&mut self, name: &str) {
        let statuses = self.state.statuses.get_or_insert_with(Vec::new);
        if !statuses.iter().any(|s| s == name) {
            statuses.push(name.to_owned());
        }
    }

    fn clear_status(&mut self, name: &str) {
        if let Some(statuses) = self.state.statuses.as_mut() {
            statuses.retain(|s| s != name);
        }
    }

    /// Apply one typed event. Whether the snapshot changed.
    pub fn apply(&mut self, ev: &GameEvent) -> bool {
        let before = self.state.clone();
        let s = &mut self.state;
        match ev {
            GameEvent::Prompt {
                hp,
                hp_max,
                mana,
                mana_max,
                resting,
                form,
                exp,
                ..
            } => {
                set(&mut s.hp, i32_of(*hp));
                set(&mut s.hp_max, i32_of(*hp_max));
                set(&mut s.mana, i32_of(*mana));
                set(&mut s.mana_max, i32_of(*mana_max));
                set(&mut s.exp, u64_of(*exp));
                if let Some(rest) = resting {
                    s.resting = Some(*rest != Rest::Awake);
                }
                s.prompt_form = Some(form_name(*form, mana.is_some()).to_owned());
            }
            GameEvent::StatusName { name, lives, cp } => {
                s.name = Some(name.clone());
                set(&mut s.lives, u32_of(*lives));
                set(&mut s.cp, u32_of(*cp));
                // The status screen lists every status the character has; it starts here.
                s.statuses = Some(Vec::new());
            }
            GameEvent::StatusRace { race, exp, .. } => {
                s.race = Some(race.clone());
                set(&mut s.exp, u64_of(*exp));
            }
            GameEvent::StatusClass { class, level, .. } => {
                s.class = Some(class.clone());
                set(&mut s.level, u32_of(*level));
            }
            GameEvent::StatusHits { hp, hp_max, .. } | GameEvent::Health { hp, hp_max, .. } => {
                set(&mut s.hp, i32_of(*hp));
                set(&mut s.hp_max, i32_of(*hp_max));
                if let GameEvent::Health { mana, mana_max, .. } = ev {
                    set(&mut s.mana, i32_of(*mana));
                    set(&mut s.mana_max, i32_of(*mana_max));
                }
            }
            GameEvent::StatusMana { mana, mana_max, .. } => {
                set(&mut s.mana, i32_of(*mana));
                set(&mut s.mana_max, i32_of(*mana_max));
            }
            GameEvent::StatusStat { stats } => {
                let st = s.stats.get_or_insert_with(Stats::default);
                for (label, n) in stats {
                    let slot = match label.as_str() {
                        "Strength" => &mut st.strength,
                        "Intellect" => &mut st.intellect,
                        "Willpower" => &mut st.willpower,
                        "Agility" => &mut st.agility,
                        "Health" => &mut st.health,
                        "Charm" => &mut st.charm,
                        _ => continue,
                    };
                    set(slot, u32_of(Some(*n)));
                }
            }
            GameEvent::ExpScreen {
                exp,
                level,
                needed,
                percent,
                ..
            } => {
                set(&mut s.exp, u64_of(*exp));
                set(&mut s.level, u32_of(*level));
                set(&mut s.exp_next, u64_of(*needed));
                set(&mut s.exp_percent, u32_of(*percent));
            }
            GameEvent::RoomTitle { title } => {
                s.room_title = Some(title.clone());
                // The new room's exits follow its title; the old ones are not its.
                s.room_exits = None;
            }
            GameEvent::RoomExits { exits } => {
                s.room_exits = Some(
                    exits
                        .iter()
                        .map(|e| {
                            e.dir
                                .clone()
                                .or_else(|| e.state.clone())
                                .unwrap_or_default()
                        })
                        .collect(),
                );
            }
            GameEvent::RoomDark => {
                s.room_title = None;
                s.room_exits = None;
            }
            GameEvent::CombatEngaged => s.in_combat = Some(true),
            GameEvent::CombatOff => s.in_combat = Some(false),
            GameEvent::TookDamage { amount } => {
                // Until the next prompt says what it is.
                if let (Some(hp), Ok(dmg)) = (s.hp.as_mut(), i32::try_from(*amount)) {
                    *hp = hp.saturating_sub(dmg);
                }
            }
            GameEvent::Healed => {
                if let Some(max) = s.hp_max {
                    s.hp = Some(max);
                }
            }
            GameEvent::MonsterDeath { .. } => {
                // No target is tracked yet (the bot's cards add one); the count is kept.
                *s.kills.get_or_insert(0) += 1;
            }
            GameEvent::ExpGain { amount } => {
                if let (Some(exp), Ok(gain)) = (s.exp.as_mut(), u64::try_from(*amount)) {
                    *exp += gain;
                    if let Some(next) = s.exp_next.as_mut() {
                        *next = next.saturating_sub(gain);
                    }
                }
            }
            GameEvent::Resting { .. } => s.resting = Some(true),
            GameEvent::RestEnd => s.resting = Some(false),
            GameEvent::YouKilled => {
                s.in_combat = Some(false);
                s.resting = Some(false);
                s.statuses = Some(Vec::new());
            }
            GameEvent::Dead { lives_left } => set(&mut s.lives, u32_of(*lives_left)),
            GameEvent::Unconscious { who } if is_me(s.name.as_deref(), who) => {
                s.hp = Some(s.hp.map_or(0, |hp| hp.min(0)));
                s.in_combat = Some(false);
            }
            GameEvent::Poisoned | GameEvent::PoisonTick => self.status("poisoned"),
            GameEvent::Cured => self.clear_status("poisoned"),
            GameEvent::Blind => self.status("blind"),
            GameEvent::Inventory { coins, items } => {
                let carried = coins
                    .iter()
                    .map(|(coin, count)| CoinCount {
                        coin: coin.clone(),
                        count: *count,
                    })
                    .collect();
                s.coins.get_or_insert_with(Coins::default).carried = carried;
                s.carrying = Some(items.iter().map(|i| i.name.clone()).collect());
                self.items = items.clone();
            }
            GameEvent::Wealth { coins } => {
                s.coins.get_or_insert_with(Coins::default).copper = u64_of(Some(*coins));
            }
            GameEvent::Encumbrance {
                current,
                max,
                level,
                ..
            } => {
                s.encumbrance = Some(Encumbrance {
                    current: u32_of(*current),
                    max: u32_of(*max),
                    level: (!level.is_empty()).then(|| level.clone()),
                });
            }
            _ => {}
        }
        self.state != before
    }

    /// Mark the snapshot as reflecting output `seq`.
    /// The fight lapsed without a word from the game (the session's quiet rule).
    pub fn combat_lapsed(&mut self) {
        self.state.in_combat = Some(false);
    }

    pub fn set_updated_seq(&mut self, seq: u64) {
        self.state.updated_seq = Some(seq);
    }

    /// Mark a finished re-sync (RFC 3339).
    pub fn set_synced_at(&mut self, at: String) {
        self.state.synced_at = Some(at);
    }

    pub fn snapshot(&self) -> CharacterState {
        self.state.clone()
    }

    /// The snapshot, borrowed.
    pub fn state(&self) -> &CharacterState {
        &self.state
    }
}

/// The JSON merge patch (RFC 7386) that turns `old` into `new`: changed fields with their new
/// values, objects patched field by field, `null` for a field `new` lacks. `None` when they
/// are the same.
pub fn merge_patch(old: &Value, new: &Value) -> Option<Value> {
    match (old, new) {
        (Value::Object(old), Value::Object(new)) => {
            let mut patch = Map::new();
            for (k, v) in new {
                match old.get(k) {
                    Some(o) => {
                        if let Some(p) = merge_patch(o, v) {
                            patch.insert(k.clone(), p);
                        }
                    }
                    None => {
                        patch.insert(k.clone(), v.clone());
                    }
                }
            }
            for k in old.keys().filter(|k| !new.contains_key(*k)) {
                patch.insert(k.clone(), Value::Null);
            }
            (!patch.is_empty()).then_some(Value::Object(patch))
        }
        _ if old == new => None,
        _ => Some(new.clone()),
    }
}

/// Apply a merge patch (RFC 7386) to `target`.
pub fn apply_patch(target: &mut Value, patch: &Value) {
    let Value::Object(patch) = patch else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = Value::Object(Map::new());
    }
    let obj = target.as_object_mut().expect("an object");
    for (k, v) in patch {
        if v.is_null() {
            obj.remove(k);
        } else {
            apply_patch(obj.entry(k.clone()).or_insert(Value::Null), v);
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_patch_turns_old_into_new() {
        let old = json!({"hp": 30, "stats": {"strength": 60, "agility": 50}, "room_title": "A", "room_exits": ["n"]});
        let new = json!({"hp": 23, "stats": {"strength": 60, "charm": 40}, "room_exits": ["n", "s"], "in_combat": true});
        let patch = merge_patch(&old, &new).unwrap();
        assert_eq!(
            patch,
            json!({"hp": 23, "stats": {"agility": null, "charm": 40}, "room_title": null, "room_exits": ["n", "s"], "in_combat": true})
        );
        let mut applied = old.clone();
        apply_patch(&mut applied, &patch);
        assert_eq!(applied, new);
        assert_eq!(merge_patch(&new, &new), None);
    }

    #[test]
    fn exp_gain_counts_down_the_next_level() {
        let mut c = Character::new();
        c.apply(&GameEvent::ExpScreen {
            exp: Some(100),
            level: Some(1),
            needed: Some(50),
            next_total: None,
            percent: Some(66),
        });
        assert!(c.apply(&GameEvent::ExpGain { amount: 20 }));
        let s = c.snapshot();
        assert_eq!((s.exp, s.exp_next), (Some(120), Some(30)));
    }

    #[test]
    fn the_status_screen_resets_statuses() {
        let mut c = Character::new();
        c.apply(&GameEvent::PoisonTick);
        c.apply(&GameEvent::Blind);
        assert_eq!(
            c.snapshot().statuses,
            Some(vec!["poisoned".into(), "blind".into()])
        );
        c.apply(&GameEvent::StatusName {
            name: "Kasimir".into(),
            lives: Some(9),
            cp: Some(0),
        });
        c.apply(&GameEvent::Poisoned);
        assert_eq!(c.snapshot().statuses, Some(vec!["poisoned".into()]));
    }
}
