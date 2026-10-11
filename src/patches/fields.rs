use serde_json::{json, Value};

use crate::config::ItemConfig;

pub(crate) const MIN_RATIO: f64 = 0.65;
pub(crate) const MAX_RATIO: f64 = 1.4;
pub(crate) const MAX_SINGLE_CHANGE: f64 = 0.25;

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

const HEALTH_STEP: f64 = 50.0;
const FLAT_STEP: f64 = 5.0;
const PERCENT_STEP: f64 = 1.0;

// The flat stats that are health.
const HEALTH_FLAT: &[&str] = &["hp"];

// The flat stats a tooltip writes as a percentage.
const PERCENT_FLAT: &[&str] = &[
    "attack_mult",
    "magic_power_mult",
    "magic_resistance_mult",
    "crit_chance",
    "attack_speed_mult",
    "move_speed_mult",
    "toughness",
    "defence_penetration",
    "magic_resistance_penetration",
    "skill_damaged_reduce",
    "base_attack_damaged_reduce",
    "vamp",
];

const UNIT_FLAT: &[&str] = &["hp_regen"];

const KEEPS_ITS_THREE: &[(&str, &[&str])] = &[("trinity_force", &["price", "attack"])];
const THREE_STEP: f64 = 10.0;

fn keeps_its_three(item: &str, field: &str, base: f64) -> bool {
    let family = crate::build_config::base_slug(item);
    base.fract() == 0.0
        && (base.abs() as u64) % 10 == 3
        && KEEPS_ITS_THREE
            .iter()
            .any(|(known, fields)| *known == family && fields.contains(&field))
}

const UNSIGNED_FLAT: &[&str] = &[
    "toughness",
    "defence_penetration",
    "magic_resistance_penetration",
    "skill_damaged_reduce",
    "base_attack_damaged_reduce",
];

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

// Fields where the smaller number is the stronger item.
const LOWER_IS_STRONGER: &[&str] = &["effect_cooldown_seconds", "effect_out_of_combat_seconds"];

// The two ends of a range that grows with level. They move together, or a
// patch could put the low end over the high one.
const PAIRS: &[(&str, &str)] = &[
    ("effect_min_bonus_damage", "effect_max_bonus_damage"),
    ("effect_min_heal", "effect_max_heal"),
    ("effect_min_shield", "effect_max_shield"),
    ("effect_min_bonus_hp", "effect_max_bonus_hp"),
];

// How one field is patched.
#[derive(Clone, Copy)]
pub(crate) struct Rule {
    // A buff makes the number smaller.
    pub lower_is_stronger: bool,
    // See [`UNSIGNED_FLAT`].
    pub unsigned: bool,
    // One of [`FLAT`].
    pub flat: bool,
}

// The rule for `field`, or nothing for one a patch never moves.
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

// The field that has to move with `field`, where there is one.
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

// Whether an item keeps `field` as a whole number, asked of the config
// type itself: a field that is one refuses a fraction. So the list of
// fields is written down once, in `utils/config.rs`.
pub(crate) fn is_whole(field: &str) -> bool {
    serde_json::from_value::<ItemConfig>(json!({ field: 0.5 })).is_err()
}

// Whether `field` is one an item can be configured with at all.
pub(crate) fn is_config_field(field: &str) -> bool {
    serde_json::from_value::<ItemConfig>(json!({ field: Value::String(String::new()) })).is_err()
}

// The step a flat stat is patched in, or nothing for a field that is not
// one.
fn flat_step(field: &str) -> Option<f64> {
    if !FLAT.contains(&field) {
        return None;
    }
    Some(if HEALTH_FLAT.contains(&field) {
        HEALTH_STEP
    } else if PERCENT_FLAT.contains(&field) || UNIT_FLAT.contains(&field) {
        PERCENT_STEP
    } else {
        FLAT_STEP
    })
}

pub(crate) fn step(item: &str, field: &str, base: f64, whole: bool) -> f64 {
    if keeps_its_three(item, field, base) {
        return THREE_STEP;
    }
    if let Some(step) = flat_step(field) {
        return step;
    }
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

pub(crate) fn quantize(item: &str, field: &str, base: f64, target: f64, whole: bool) -> f64 {
    if base == 0.0 {
        return 0.0;
    }
    if (target - base).abs() < 1e-9 {
        return base;
    }
    let step = step(item, field, base, whole);
    let from_base = keeps_its_three(item, field, base);
    let stepped = if from_base {
        base + ((target - base) / step).round() * step
    } else {
        (target / step).round() * step
    };
    // Four places: what is left of a product of doubles after that is noise.
    let stepped = (stepped * 10_000.0).round() / 10_000.0;
    let stepped = if target > base {
        stepped.max(base)
    } else {
        stepped.min(base)
    };
    // The least it can be: a step, or all of a number smaller than one; and
    // of a number counted from its base, the last point before zero.
    let least = if from_base {
        base.abs() % step
    } else {
        step.min(base.abs())
    };
    if base > 0.0 {
        stepped.max(least)
    } else {
        stepped.min(-least)
    }
}
