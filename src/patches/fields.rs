//! Which of an item's numbers a balance patch may move, which way is a buff,
//! and in what steps.
//!
//! The shape is the base game's own, read off its champion patches
//! (`setting/patch_setting`): every field has a floor and a cap on what all
//! patches together may make of it, a smallest step, and a flag for the
//! fields where lower is stronger. The numbers here are this mod's picks
//! (2026-10-10), and this file is where to tune them.

use serde_json::{json, Value};

use crate::config::ItemConfig;

/// The lowest a field goes, as a share of its unpatched value, however many
/// patches nerf it.
pub(crate) const MIN_RATIO: f64 = 0.75;
/// And the highest.
pub(crate) const MAX_RATIO: f64 = 1.3;
/// The most one patch moves one number, as a share of what it was. A number
/// whose smallest step is more than this (3 stacks to 4) is left alone.
pub(crate) const MAX_SINGLE_CHANGE: f64 = 0.25;

/// Config fields that are an item's flat stats: what `StableItem::stat`
/// hands the game, by the name `BuffV1` and the game's own settings give it.
pub(crate) const FLAT: &[&str] = &[
    "hp",
    "hp_regen",
    "attack",
    "attack_mult",
    "magic_power",
    "magic_power_mult",
    "crit_chance",
    "attack_speed_mult",
    "move_speed_mult",
    "defence",
    "magic_resistance",
    "magic_resistance_mult",
    "toughness",
    "defence_penetration",
    "magic_resistance_penetration",
    "skill_cooldown_mult",
    "ult_cooldown_mult",
    "skill_damaged_reduce",
    "base_attack_damaged_reduce",
    "vamp",
];

/// Flat stats the game keeps as unsigned numbers. A patch reaches an item's
/// flat stats as a buff on its holder ([`super::on_match_tick`]), and a buff
/// cannot take away from an unsigned stat, so these are never patched under
/// what the game holds for the item.
const UNSIGNED_FLAT: &[&str] = &[
    "toughness",
    "defence_penetration",
    "magic_resistance_penetration",
    "skill_damaged_reduce",
    "base_attack_damaged_reduce",
];

/// Never patched, though they are numbers in an item's text.
///
/// Counts and caps that are part of how a passive works (stacks, targets),
/// reaches and sizes (the effects drawn for them are sized to match), waits
/// that are not a cooldown, what only minions, monsters and towers feel, the
/// World Atlas line's gold (paid from the match hook at the rate the item
/// registered with), and thresholds, where which way is a buff depends on
/// the sentence.
const FROZEN: &[&str] = &[
    "price",
    "effect_max_stacks",
    "effect_min_stacks",
    "effect_max_targets",
    "effect_max_growth_stacks",
    "effect_stacks_per_second",
    "effect_hp_per_stack",
    "effect_attack_interval",
    "effect_melee_distance",
    "effect_max_distance",
    "effect_explosion_distance",
    "effect_max_size_percent",
    "effect_size_per_thousand_hp",
    "effect_bonus_gold",
    "effect_gold_interval_seconds",
    "effect_growth_interval_seconds",
    "effect_heal_duration_seconds",
    "effect_heal_interval_seconds",
    "effect_charge_seconds",
    "effect_delay_seconds",
    "effect_ranged_percent",
    "effect_minion_percent",
    "effect_minion_bonus_percent",
    "effect_minion_damage_cap",
    "effect_tower_ad_percent_damage",
    "effect_tower_caster_hp_percent_damage",
    "effect_hp_percent_threshold",
    "effect_magic_resistance_per_reduce",
    "effect_max_skill_damaged_reduce",
];

/// Fields where the smaller number is the stronger item.
const LOWER_IS_STRONGER: &[&str] = &["effect_cooldown_seconds", "effect_out_of_combat_seconds"];

/// The two ends of a range that grows with level. They move together, or a
/// patch could put the low end over the high one.
const PAIRS: &[(&str, &str)] = &[
    ("effect_min_bonus_damage", "effect_max_bonus_damage"),
    ("effect_min_heal", "effect_max_heal"),
    ("effect_min_shield", "effect_max_shield"),
    ("effect_min_bonus_hp", "effect_max_bonus_hp"),
];

/// How one field is patched.
#[derive(Clone, Copy)]
pub(crate) struct Rule {
    /// A buff makes the number smaller.
    pub lower_is_stronger: bool,
    /// See [`UNSIGNED_FLAT`].
    pub unsigned: bool,
    /// One of [`FLAT`].
    pub flat: bool,
}

/// The rule for `field`, or nothing for one a patch never moves.
pub(crate) fn rule(field: &str) -> Option<Rule> {
    if FROZEN.contains(&field) {
        return None;
    }
    let flat = FLAT.contains(&field);
    if !flat && !field.starts_with("effect_") && field != "adaptive_force" {
        return None;
    }
    Some(Rule {
        lower_is_stronger: LOWER_IS_STRONGER.contains(&field),
        unsigned: UNSIGNED_FLAT.contains(&field),
        flat,
    })
}

/// The field that has to move with `field`, where there is one.
pub(crate) fn partner(field: &str) -> Option<&'static str> {
    PAIRS.iter().find_map(|&(low, high)| {
        if low == field {
            Some(high)
        } else if high == field {
            Some(low)
        } else {
            None
        }
    })
}

/// Whether an item keeps `field` as a whole number, asked of the config
/// type itself: a field that is one refuses a fraction. So the list of
/// fields is written down once, in `utils/config.rs`.
pub(crate) fn is_whole(field: &str) -> bool {
    serde_json::from_value::<ItemConfig>(json!({ field: 0.5 })).is_err()
}

/// Whether `field` is one an item can be configured with at all.
pub(crate) fn is_config_field(field: &str) -> bool {
    serde_json::from_value::<ItemConfig>(json!({ field: Value::String(String::new()) })).is_err()
}

/// The smallest change to a number that reads `base` unpatched: one for a
/// small whole number, and enough to keep a round number round for a big
/// one (150 health goes to 155, not 151).
pub(crate) fn step(base: f64, whole: bool) -> f64 {
    let size = base.abs();
    let divides = |by: f64| (size / by).fract() == 0.0;
    if whole {
        if size >= 500.0 && divides(25.0) {
            25.0
        } else if size >= 200.0 && divides(10.0) {
            10.0
        } else if size >= 50.0 && divides(5.0) {
            5.0
        } else {
            1.0
        }
    } else if size >= 20.0 {
        1.0
    } else if size >= 5.0 {
        0.5
    } else if size >= 1.0 {
        if (size * 4.0).fract() == 0.0 {
            0.25
        } else {
            0.1
        }
    } else {
        0.05
    }
}

/// `target` on `base`'s grid of steps, and never the other side of zero.
pub(crate) fn quantize(base: f64, target: f64, whole: bool) -> f64 {
    let step = step(base, whole);
    let stepped = (target / step).round() * step;
    // Four places: what is left of a product of doubles after that is noise.
    let stepped = (stepped * 10_000.0).round() / 10_000.0;
    if base > 0.0 {
        stepped.max(step)
    } else if base < 0.0 {
        stepped.min(-step)
    } else {
        0.0
    }
}
