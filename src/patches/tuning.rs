//! The patch system's settings, from `config-autopatch.json` beside the DLL.
//!
//! ```json
//! {
//!     "enabled": true,
//!     "step_size": { "health": 50, "attack_damage": 5, "attack_speed": 1 },
//!     "limits": {
//!         "min_games": 16, "max_nerfs": 4, "max_buffs": 5,
//!         "min_total_percent": 65, "max_total_percent": 140
//!     }
//! }
//! ```
//!
//! Read once. A value the file does not give, or gives as something that
//! cannot be used, keeps its default, and so does everything where there is
//! no file. A step of 0 is no step at all: no patch moves that stat.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use serde_json::Value;

use super::fields;

const FILE: &str = "config-autopatch.json";

pub(crate) struct Tuning {
    // Off, no patch is made or applied. Tooltips still follow `config.json`.
    pub enabled: bool,
    // Config field -> the step its number moves in. One not named here has
    // the step of its kind (`fields::step`).
    pub steps: HashMap<String, f64>,
    // Config fields given a step of 0: numbers no patch moves. One a patch
    // has already moved stays where that patch left it.
    pub frozen: HashSet<String>,
    // Games an item has to have been held in on a version to be patched.
    pub min_games: u32,
    pub max_nerfs: usize,
    pub max_buffs: usize,
    // The band of an item's total, as shares of its unpatched numbers.
    pub min_ratio: f64,
    pub max_ratio: f64,
}

// The names `step_size` goes by, and the config field each is. A name can
// be several fields: lethality is the lethality an item gives and the bonus
// some give on top of it. A config field's own name is taken too, a
// passive's number among them (`effect_slow_amount`).
const STAT_NAMES: [(&str, &str); 23] = [
    ("health", "hp"),
    ("health_regen", "hp_regen"),
    ("attack_damage", "attack"),
    ("ability_power", "magic_power"),
    ("armor", "defence"),
    ("magic_resist", "magic_resistance"),
    ("ability_haste", "skill_cooldown_mult"),
    ("ultimate_haste", "ult_cooldown_mult"),
    ("attack_speed", "attack_speed_mult"),
    ("movement_speed", "move_speed_mult"),
    ("move_speed", "move_speed_mult"),
    ("crit_chance", "crit_chance"),
    ("armor_penetration", "defence_penetration"),
    ("magic_penetration", "magic_resistance_penetration"),
    ("lethality", "effect_lethality"),
    ("lethality", "effect_bonus_lethality"),
    ("omnivamp", "vamp"),
    ("tenacity", "toughness"),
    ("skill_damage_reduction", "skill_damaged_reduce"),
    ("basic_attack_damage_reduction", "base_attack_damaged_reduce"),
    ("attack_damage_mult", "attack_mult"),
    ("ability_power_mult", "magic_power_mult"),
    ("magic_resist_mult", "magic_resistance_mult"),
];

pub(crate) fn tuning() -> &'static Tuning {
    static TUNING: OnceLock<Tuning> = OnceLock::new();
    TUNING.get_or_init(read)
}

fn read() -> Tuning {
    let root = super::base::read_object(FILE);
    let mut tuning = Tuning {
        enabled: true,
        steps: HashMap::new(),
        frozen: HashSet::new(),
        min_games: 16,
        max_nerfs: 4,
        max_buffs: 5,
        min_ratio: fields::MIN_RATIO,
        max_ratio: fields::MAX_RATIO,
    };
    if let Some(enabled) = root.get("enabled").and_then(Value::as_bool) {
        tuning.enabled = enabled;
    }
    if let Some(Value::Object(steps)) = root.get("step_size") {
        for (name, step) in steps {
            let Some(step) = step.as_f64().filter(|step| step.is_finite() && *step >= 0.0) else {
                continue;
            };
            let mut named: Vec<&str> = STAT_NAMES
                .iter()
                .filter(|(known, _)| *known == name)
                .map(|(_, field)| *field)
                .collect();
            if named.is_empty() {
                named.push(name.as_str());
            }
            for field in named {
                if step == 0.0 {
                    tuning.frozen.insert(field.to_string());
                    continue;
                }
                // A stat of the stat block is a whole number, and so is its
                // step.
                let step = if fields::FLAT.contains(&field) {
                    step.round().max(1.0)
                } else {
                    step
                };
                tuning.steps.insert(field.to_string(), step);
            }
        }
    }
    if let Some(Value::Object(limits)) = root.get("limits") {
        let count = |name: &str| limits.get(name).and_then(Value::as_u64);
        let percent = |name: &str| {
            limits
                .get(name)
                .and_then(Value::as_f64)
                .filter(|percent| percent.is_finite())
        };
        if let Some(games) = count("min_games") {
            tuning.min_games = games.min(u64::from(u32::MAX)) as u32;
        }
        if let Some(items) = count("max_nerfs") {
            tuning.max_nerfs = items as usize;
        }
        if let Some(items) = count("max_buffs") {
            tuning.max_buffs = items as usize;
        }
        if let Some(low) = percent("min_total_percent").filter(|low| (1.0..=100.0).contains(low)) {
            tuning.min_ratio = low / 100.0;
        }
        if let Some(high) = percent("max_total_percent").filter(|high| *high >= 100.0) {
            tuning.max_ratio = high / 100.0;
        }
    }
    super::log("patch.tuning", || {
        let mut steps: Vec<String> = tuning
            .steps
            .iter()
            .map(|(field, step)| format!("{field}={step}"))
            .collect();
        steps.sort();
        let mut frozen: Vec<&String> = tuning.frozen.iter().collect();
        frozen.sort();
        format!(
            "{FILE} read={}: enabled={} min_games={} max_nerfs={} max_buffs={} band={}..{} never moved: {frozen:?} steps: {}",
            !root.is_empty(),
            tuning.enabled,
            tuning.min_games,
            tuning.max_nerfs,
            tuning.max_buffs,
            tuning.min_ratio,
            tuning.max_ratio,
            steps.join(" ")
        )
    });
    tuning
}
