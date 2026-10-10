//! The balance as a match reads it: everything the patch state comes to,
//! worked out once when it changes and read from the simulation, the build
//! hooks and the tooltips without working anything out again.
//!
//! # How a patched number reaches a match
//!
//! - **One of this mod's items, a passive's number.** Every item is built
//!   from an `ItemConfig`, and the game copies the registered item for each
//!   purchase. The wrapper every item registers in (`perf::Timed`) builds it
//!   again from the patched config the first time any hook of the copy runs
//!   ([`config_for`]), so a copy carries one balance for as long as it lives.
//! - **One of this mod's items, a flat stat.** The game holds its own copy of
//!   an item's stats from the day it registered and never asks again, so the
//!   difference is kept on the holder as one buff, [`DELTA_BUFF`], the sum
//!   over what it holds ([`on_match_tick`]). A buff cannot take away from the
//!   stats the game keeps unsigned, which is why those are never patched
//!   down on a mod item (`fields::Rule::unsigned`).
//! - **One of the game's own thirty.** Its stats are server settings, which
//!   `item_stats::sync_server_items` already writes at every server start;
//!   it writes the patched numbers ([`game_stats`]).
//! - **What sits outside the item.** Axiom Arc reads every lethality item's
//!   number from a table, and the Spellblade items theirs from another; both
//!   are told (`crate::set_lethality_patches`, `Base::refreshes`).

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Instant;

use mod_api_stable::*;
use serde_json::Value;

use super::base::Base;
use super::state::State;
use super::{fields, text};
use crate::config::ItemConfig;

/// The buff a champion holds for the patched flat stats of its items.
const DELTA_BUFF: &str = "riot_item_patch";

/// Ticks between two looks at what each champion should hold of it: a third
/// of a second. A buff just added does not show among an entity's buffs for
/// a few ticks, so this must stay well over that or it would be added twice.
const DELTA_CHECK_TICKS: usize = 20;

/// How far an item's patches have to have moved it, as the sum of the
/// logarithms of its fields' ratios, to lean a build choice as hard as they
/// can: a quarter, about what five patches of one field come to.
const LEANING_FULL: f64 = 0.25;

/// The numbers of a stat block, in [`STAT_NAMES`] order.
const STATS: usize = 32;
type Numbers = [i64; STATS];

/// From this index on, the game keeps the stat unsigned.
const UNSIGNED_FROM: usize = 18;

const STAT_NAMES: [&str; STATS] = [
    "attack",
    "attack_mult",
    "magic_power",
    "magic_power_mult",
    "defence",
    "defence_mult",
    "hp",
    "hp_regen",
    "magic_resistance",
    "magic_resistance_mult",
    "vamp",
    "hp_mult",
    "move_speed_mult",
    "attack_speed_mult",
    "skill_cooldown_mult",
    "ult_cooldown_mult",
    "radius_mult",
    "crit_chance",
    "damage_reflect",
    "damaged_amplify",
    "damaged_reduce",
    "defence_penetration",
    "magic_resistance_penetration",
    "toughness",
    "heal_reduce",
    "range",
    "base_attack_enemy_max_hp_damage",
    "self_max_hp_damage",
    "skill_enemy_max_hp_damage",
    "dot_amplify",
    "base_attack_damaged_reduce",
    "skill_damaged_reduce",
];

fn numbers(stat: &BuffV1) -> Numbers {
    [
        stat.attack as i64,
        stat.attack_mult as i64,
        stat.magic_power as i64,
        stat.magic_power_mult as i64,
        stat.defence as i64,
        stat.defence_mult as i64,
        stat.hp as i64,
        stat.hp_regen as i64,
        stat.magic_resistance as i64,
        stat.magic_resistance_mult as i64,
        stat.vamp as i64,
        stat.hp_mult as i64,
        stat.move_speed_mult as i64,
        stat.attack_speed_mult as i64,
        stat.skill_cooldown_mult as i64,
        stat.ult_cooldown_mult as i64,
        stat.radius_mult as i64,
        stat.crit_chance as i64,
        stat.damage_reflect as i64,
        stat.damaged_amplify as i64,
        stat.damaged_reduce as i64,
        stat.defence_penetration as i64,
        stat.magic_resistance_penetration as i64,
        stat.toughness as i64,
        stat.heal_reduce as i64,
        stat.range as i64,
        stat.base_attack_enemy_max_hp_damage as i64,
        stat.self_max_hp_damage as i64,
        stat.skill_enemy_max_hp_damage as i64,
        stat.dot_amplify as i64,
        stat.base_attack_damaged_reduce as i64,
        stat.skill_damaged_reduce as i64,
    ]
}

/// A permanent buff named `name` that gives `of`.
fn buff_of(name: &str, of: &Numbers) -> BuffV1 {
    let unsigned = |index: usize| of[index].max(0) as usize;
    BuffV1 {
        attack: of[0] as i32,
        attack_mult: of[1] as i32,
        magic_power: of[2] as i32,
        magic_power_mult: of[3] as i32,
        defence: of[4] as i32,
        defence_mult: of[5] as i32,
        hp: of[6] as i32,
        hp_regen: of[7] as i32,
        magic_resistance: of[8] as i32,
        magic_resistance_mult: of[9] as i32,
        vamp: of[10] as i32,
        hp_mult: of[11] as i32,
        move_speed_mult: of[12] as i32,
        attack_speed_mult: of[13] as i32,
        skill_cooldown_mult: of[14] as i32,
        ult_cooldown_mult: of[15] as i32,
        radius_mult: of[16] as i32,
        crit_chance: of[17] as i32,
        damage_reflect: unsigned(18),
        damaged_amplify: unsigned(19),
        damaged_reduce: unsigned(20),
        defence_penetration: unsigned(21),
        magic_resistance_penetration: unsigned(22),
        toughness: unsigned(23),
        heal_reduce: unsigned(24),
        range: unsigned(25),
        base_attack_enemy_max_hp_damage: unsigned(26),
        self_max_hp_damage: unsigned(27),
        skill_enemy_max_hp_damage: unsigned(28),
        dot_amplify: unsigned(29),
        base_attack_damaged_reduce: unsigned(30),
        skill_damaged_reduce: unsigned(31),
        ..BuffV1::named(name)
    }
}

/// One stat of a stat block by its field name, or nothing for a name that
/// is not a stat.
pub(crate) fn stat_number(stat: &BuffV1, field: &str) -> Option<i64> {
    let index = STAT_NAMES.iter().position(|name| *name == field)?;
    Some(numbers(stat)[index])
}

pub(crate) struct Live {
    /// Mod item -> its config with the patch laid over the player's own.
    configs: HashMap<String, Arc<ItemConfig>>,
    /// Mod item -> what its patched stats add to the ones the game holds.
    deltas: HashMap<String, Numbers>,
    /// Game item -> stat field -> patched number.
    game: HashMap<String, Vec<(String, i64)>>,
    /// Item -> what its tooltip has to say that the game will not.
    pub display: HashMap<String, text::Display>,
    /// Item -> how its patches lean a build choice, -1 (nerfed as far as it
    /// counts) to 1.
    leaning: HashMap<String, f32>,
}

static LIVE: RwLock<Option<Arc<Live>>> = RwLock::new(None);
static ANY_CONFIGS: AtomicBool = AtomicBool::new(false);
static ANY_DELTAS: AtomicBool = AtomicBool::new(false);
static ANY_LEANING: AtomicBool = AtomicBool::new(false);
static ANY_GAME: AtomicBool = AtomicBool::new(false);

/// The balance now in force, or nothing while no number is patched.
pub(crate) fn current() -> Option<Arc<Live>> {
    LIVE.read().ok()?.clone()
}

/// Works the whole of [`Live`] out from the patch state, and tells whatever
/// keeps an item's numbers outside the item. With an empty state, which is
/// what leaving a save comes to, everything is as the mod loaded it.
pub(crate) fn rebuild(base: &Base, state: &State) {
    let texts = text::texts();
    // (item, field) -> patched number, where that is not the unpatched one.
    let mut values: HashMap<(String, String), f64> = HashMap::new();
    let mut leaning: HashMap<String, f32> = HashMap::new();
    for (id, ratios) in &state.ratios {
        let Some(family) = base.families.get(id) else {
            continue;
        };
        let mut level = 0.0f64;
        for (field, &ratio) in ratios {
            let Some(rule) = fields::rule(field) else {
                continue;
            };
            let mut moved = false;
            for member in &family.members {
                let Some(&unpatched) = member.values.get(field) else {
                    continue;
                };
                let whole = member.whole.contains(field);
                let value = fields::quantize(unpatched, unpatched * ratio, whole);
                if value != unpatched {
                    values.insert((member.key.clone(), field.clone()), value);
                    moved = true;
                }
            }
            if moved && ratio > 0.0 {
                level += if rule.lower_is_stronger {
                    -ratio.ln()
                } else {
                    ratio.ln()
                };
            }
        }
        let lean = (level / LEANING_FULL).clamp(-1.0, 1.0) as f32;
        if lean != 0.0 {
            for member in &family.members {
                leaning.insert(member.key.clone(), lean);
            }
        }
    }

    let mut by_item: HashMap<&str, Vec<(&str, f64)>> = HashMap::new();
    for ((key, field), value) in &values {
        by_item
            .entry(key.as_str())
            .or_default()
            .push((field.as_str(), *value));
    }

    let mut configs: HashMap<String, Arc<ItemConfig>> = HashMap::new();
    let mut deltas: HashMap<String, Numbers> = HashMap::new();
    let mut game: HashMap<String, Vec<(String, i64)>> = HashMap::new();
    let mut display: HashMap<String, text::Display> = HashMap::new();
    let mut lethality: HashMap<String, usize> = HashMap::new();
    for (key, patched) in &by_item {
        let Some(item) = base.mod_items.get(*key) else {
            game.insert(
                key.to_string(),
                patched
                    .iter()
                    .map(|(field, value)| (field.to_string(), *value as i64))
                    .collect(),
            );
            continue;
        };
        let mut object = base.raw_config(key);
        for (field, value) in patched {
            let number = if value.fract() == 0.0 {
                Value::from(*value as i64)
            } else {
                Value::from(*value)
            };
            object.insert(field.to_string(), number);
            if *field == "effect_lethality" {
                lethality.insert(key.to_string(), value.max(0.0) as usize);
            }
        }
        let Ok(config) = serde_json::from_value::<ItemConfig>(Value::Object(object)) else {
            super::log(&format!("patch.config.{key}"), || {
                "its patched config does not read as an item config; left unpatched".to_string()
            });
            continue;
        };
        let before = numbers(&item.registered);
        let after = numbers(&(item.stat_of)(&config));
        let mut delta = [0i64; STATS];
        let mut flats = Vec::new();
        for index in 0..STATS {
            let mut change = after[index] - before[index];
            if index >= UNSIGNED_FROM {
                change = change.max(0);
            }
            delta[index] = change;
            if change != 0 && texts.has_line(STAT_NAMES[index]) {
                flats.push((
                    STAT_NAMES[index].to_string(),
                    before[index],
                    before[index] + change,
                ));
            }
        }
        if delta.iter().any(|change| *change != 0) {
            deltas.insert(key.to_string(), delta);
        }
        if !flats.is_empty() {
            display.entry(key.to_string()).or_default().flats = flats;
        }
        configs.insert(key.to_string(), Arc::new(config));
    }

    // Every effect text that quotes a patched number, the item's own or
    // another's.
    let value_now = |item: &str, field: &str| {
        values
            .get(&(item.to_string(), field.to_string()))
            .copied()
            .unwrap_or_else(|| base.value(item, field))
    };
    let mut quoted: HashSet<&str> = HashSet::new();
    for (key, field) in values.keys() {
        if let Some(shown) = texts.shown(key, field) {
            quoted.extend(shown.texts.iter().map(String::as_str));
        }
    }
    for key in quoted {
        let options = texts.options_of(key, &value_now);
        if !options.is_empty() {
            display.entry(key.to_string()).or_default().options = options;
        }
    }

    for (key, refresh) in &base.refreshes {
        match configs.get(*key) {
            Some(config) => refresh(&**config),
            None => {
                let unpatched = Value::Object(base.raw_config(key));
                if let Ok(config) = serde_json::from_value::<ItemConfig>(unpatched) {
                    refresh(&config);
                }
            }
        }
    }
    crate::set_lethality_patches(lethality);

    let flags = [
        (&ANY_CONFIGS, !configs.is_empty()),
        (&ANY_DELTAS, !deltas.is_empty()),
        (&ANY_LEANING, !leaning.is_empty()),
        (&ANY_GAME, !game.is_empty()),
    ];
    let live = (!values.is_empty()).then(|| {
        Arc::new(Live {
            configs,
            deltas,
            game,
            display,
            leaning,
        })
    });
    if let Ok(mut slot) = LIVE.write() {
        *slot = live;
    }
    for (flag, set) in flags {
        flag.store(set, Ordering::Relaxed);
    }
}

/// The patched config of one of this mod's items, for a copy of it to be
/// built from. Nothing for an item no patch has touched: one atomic read.
pub(crate) fn config_for(key: &str) -> Option<Arc<ItemConfig>> {
    if !ANY_CONFIGS.load(Ordering::Relaxed) {
        return None;
    }
    current()?.configs.get(key).cloned()
}

/// How `key`'s patches lean a build choice: see [`Live::leaning`]. 0 for an
/// item no patch has moved.
pub(crate) fn leaning(key: &str) -> f32 {
    if !ANY_LEANING.load(Ordering::Relaxed) {
        return 0.0;
    }
    current()
        .and_then(|live| live.leaning.get(key).copied())
        .unwrap_or(0.0)
}

/// The patched stats of one of the game's own items, by stat field.
pub(crate) fn game_stats(key: &str) -> Option<Vec<(String, i64)>> {
    if !ANY_GAME.load(Ordering::Relaxed) {
        return None;
    }
    current()?.game.get(key).cloned()
}

/// What a tooltip should show for `key`'s flat stat `field`, where a patch
/// has moved it.
pub(crate) fn shown_flat(key: &str, field: &str) -> Option<i64> {
    let live = current()?;
    if let Some(stats) = live.game.get(key) {
        return stats
            .iter()
            .find(|(known, _)| known == field)
            .map(|(_, value)| *value);
    }
    live.display
        .get(key)?
        .flats
        .iter()
        .find(|(known, _, _)| known == field)
        .map(|(_, _, after)| *after)
}

// -- in the match ---------------------------------------------------------------

fn clock() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// When any simulation last ticked, on [`clock`].
static LAST_TICK: AtomicU64 = AtomicU64::new(0);

/// Milliseconds since any match simulation last ticked.
pub(crate) fn quiet_millis() -> u64 {
    clock().saturating_sub(LAST_TICK.load(Ordering::Relaxed))
}

/// Keeps every champion's [`DELTA_BUFF`] at the sum of what its items'
/// patched flat stats add. From the match hook, every tick of every
/// simulation; while no flat stat is patched it is a clock read every eighth
/// tick and an atomic read.
///
/// One buff for all of a champion's items and not one an item, so nothing is
/// left behind when an item is built into the next: the sum is taken from
/// what the champion holds now. It is only touched when it is not what it
/// should be, which after a purchase is once.
pub(crate) fn on_match_tick(sim: &mut StableSim<'_>) {
    let tick = sim.tick();
    if tick % 8 == 0 {
        LAST_TICK.store(clock(), Ordering::Relaxed);
    }
    if !ANY_DELTAS.load(Ordering::Relaxed) || tick % DELTA_CHECK_TICKS != 0 || sim.is_end() {
        return;
    }
    let Some(live) = current() else {
        return;
    };
    // (champion, what it should hold, whether it holds any now)
    let mut fixes: Vec<(usize, Option<Numbers>, bool)> = Vec::new();
    for index in 0..sim.player_count() {
        let Some(player) = sim.player_at(index) else {
            continue;
        };
        let Some(champion) = player.champion().filter(|champion| champion.is_alive()) else {
            continue;
        };
        let mut want = [0i64; STATS];
        for key in player.item_keys() {
            if let Some(delta) = live.deltas.get(&key) {
                for (sum, change) in want.iter_mut().zip(delta) {
                    *sum += change;
                }
            }
        }
        let wanted = want.iter().any(|change| *change != 0);
        let mut held = 0usize;
        let mut right = false;
        for slot in 0..champion.buff_count() {
            let Some(buff) = champion.buff_at(slot) else {
                continue;
            };
            if buff.name() == DELTA_BUFF {
                held += 1;
                right = numbers(&buff) == numbers(&buff_of(DELTA_BUFF, &want));
            }
        }
        if (wanted && held == 1 && right) || (!wanted && held == 0) {
            continue;
        }
        fixes.push((champion.id(), wanted.then_some(want), held > 0));
    }
    for (champion, want, held) in fixes {
        if held {
            sim.entity_remove_buff(champion, DELTA_BUFF);
        }
        if let Some(want) = want {
            sim.add_buff(champion, &buff_of(DELTA_BUFF, &want));
        }
    }
}
