use serde::Deserialize;
use std::collections::HashMap;
use std::ffi::{c_void, OsString};
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

#[derive(Deserialize, Default)]
pub struct ItemConfig {
    pub price: Option<usize>,
    pub hp: Option<i32>,
    pub hp_regen: Option<i32>,
    pub attack: Option<i32>,
    pub attack_mult: Option<i32>,
    pub magic_power: Option<i32>,
    pub magic_power_mult: Option<i32>,
    pub adaptive_force: Option<i32>,
    pub crit_chance: Option<i32>,
    pub attack_speed_mult: Option<i32>,
    pub move_speed_mult: Option<i32>,
    pub defence: Option<i32>,
    pub magic_resistance: Option<i32>,
    pub magic_resistance_mult: Option<i32>,
    pub toughness: Option<usize>,
    pub defence_penetration: Option<usize>,
    pub magic_resistance_penetration: Option<usize>,
    pub skill_cooldown_mult: Option<i32>,
    pub ult_cooldown_mult: Option<i32>,
    pub skill_damaged_reduce: Option<usize>,
    pub vamp: Option<i32>,

    pub effect_adaptive_force: Option<i32>,
    pub effect_vamp: Option<i32>,
    pub effect_move_speed_mult: Option<i32>,
    pub effect_duration_seconds: Option<f64>,
    pub effect_heal_reduce: Option<usize>,
    pub effect_minion_damage_cap: Option<usize>,
    pub effect_ap_percent_damage: Option<f64>,
    pub effect_ad_percent_damage: Option<f64>,
    pub effect_crit_percent_damage: Option<f64>,
    pub effect_bonus_flat_damage: Option<usize>,
    pub effect_min_bonus_damage: Option<usize>,
    pub effect_max_bonus_damage: Option<usize>,
    pub effect_bonus_magic_damage: Option<usize>,
    pub effect_stack_magic_damage: Option<usize>,
    pub effect_hp_percent_damage: Option<f64>,
    pub effect_enemy_max_hp_damage: Option<usize>,
    pub effect_caster_hp_percent_damage: Option<f64>,
    pub effect_caster_hp_percent_power: Option<f64>,
    pub effect_caster_hp_percent_attack: Option<f64>,
    pub effect_caster_defence_percent_hp: Option<f64>,
    pub effect_armor_pen_per_stack: Option<usize>,
    pub effect_magic_pen_per_stack: Option<usize>,
    pub effect_slow_amount: Option<i32>,
    pub effect_attack_speed_mult: Option<i32>,
    pub effect_attack_speed_reduce: Option<i32>,
    pub effect_damaged_amplify: Option<usize>,
    pub effect_lethality: Option<usize>,
    pub effect_bonus_lethality: Option<usize>,
    pub effect_bonus_defence: Option<i32>,
    pub effect_bonus_magic_resistance: Option<i32>,
    pub effect_skill_damaged_reduce: Option<usize>,
    pub effect_magic_resistance_per_reduce: Option<usize>,
    pub effect_max_skill_damaged_reduce: Option<usize>,
    pub effect_bonus_hp_regen: Option<i32>,
    pub effect_minion_percent: Option<f64>,
    pub effect_stacks_per_second: Option<usize>,
    pub effect_move_speed_per_stack: Option<f64>,
    pub effect_out_of_combat_seconds: Option<f64>,
    pub effect_bonus_flat_attack: Option<i32>,
    pub effect_stack_attack: Option<i32>,
    pub effect_stack_attack_mult: Option<i32>,
    pub effect_stack_attack_speed_mult: Option<i32>,
    pub effect_stack_crit_chance: Option<i32>,
    pub effect_flurry_attack_speed_mult: Option<i32>,
    pub effect_stack_defence_mult: Option<i32>,
    pub effect_stack_magic_power: Option<i32>,
    pub effect_stack_magic_resistance_mult: Option<i32>,
    pub effect_damage_conversion: Option<f64>,
    pub effect_min_stacks: Option<usize>,
    pub effect_max_stacks: Option<usize>,
    pub effect_hp_per_stack: Option<usize>,
    pub effect_attack_interval: Option<usize>,
    pub effect_bonus_flat_hp: Option<i32>,
    pub effect_hp_percent_boost: Option<f64>,
    pub effect_hp_percent_threshold: Option<f64>,
    pub effect_bonus_damage_when_low: Option<f64>,
    pub effect_bonus_hp_percent_of_damage: Option<f64>,
    pub effect_cooldown_seconds: Option<f64>,
    pub effect_shield_seconds: Option<f64>,
    pub effect_bonus_flat_shield: Option<usize>,
    pub effect_ad_percent_shield: Option<f64>,
    pub effect_caster_hp_percent_shield: Option<f64>,
    pub effect_heal_mult: Option<f64>,
    pub effect_bonus_flat_heal: Option<i32>,
    pub effect_min_heal: Option<usize>,
    pub effect_max_heal: Option<usize>,
    pub effect_ad_percent_heal: Option<f64>,
    pub effect_ap_percent_heal: Option<f64>,
    pub effect_caster_hp_percent_heal: Option<f64>,
    pub effect_caster_ap_percent_heal: Option<f64>,
    pub effect_crit_damage_percent_heal: Option<f64>,
    pub effect_crit_damage_bonus: Option<i32>,
    pub effect_delayed_damage_percent: Option<f64>,
    pub effect_burn_hp_percent_cap: Option<f64>,
    pub effect_kill_heal_missing_percent: Option<f64>,
    pub effect_percent_armor_shred: Option<i32>,
    pub effect_percent_mr_shred: Option<i32>,
    pub effect_bonus_gold: Option<usize>,
    pub effect_max_distance: Option<usize>,
    pub effect_max_percent_bonus: Option<f64>,
    pub effect_percent_bonus_damage: Option<f64>,
    pub effect_min_shield: Option<usize>,
    pub effect_max_shield: Option<usize>,
    pub effect_ult_cooldown_mult: Option<i32>,
    pub effect_ult_cooldown_per_lethality: Option<f64>,
}

/// Overwrites the listed fields of an item with whatever the config file set,
/// leaving the item's built-in default in place for every key the file omits.
///
/// The field names are identical on both sides — `ItemConfig` mirrors the item
/// structs — so the list is just the set of stats a given item exposes to config:
///
/// ```ignore
/// fn configured(mut self, cfg: &ItemConfig) -> Self {
///     apply_config!(self, cfg, [price, attack, effect_duration_seconds]);
///     self
/// }
/// ```
#[macro_export]
macro_rules! apply_config {
    ($item:ident, $cfg:ident, [$($field:ident),* $(,)?]) => {
        $(
            if let Some(value) = $cfg.$field {
                $item.$field = value;
            }
        )*
    };
}

pub fn load() -> HashMap<String, ItemConfig> {
    let path = dll_dir()
        .map(|d| d.join("config.json"))
        .unwrap_or_else(|| PathBuf::from("config.json"));

    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Directory the mod's data files live in: [`dll_dir`], or the process working
/// directory when that cannot be resolved.
///
/// Everything that reads or writes a mod file goes through here so the client
/// and the hook always agree on one directory. They run in the same process, so
/// they resolve identically — but a silent `None` in one caller and a fallback
/// in another would put reads and writes in different folders.
pub(crate) fn mod_dir() -> PathBuf {
    dll_dir()
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

// Returns the directory containing this DLL by passing a static address within
// it to GetModuleHandleExW (FROM_ADDRESS flag), then resolving the full path.
//
// Resolved once and cached. The DLL cannot move while the process runs, and this
// sits under every file path the mod builds — including two on the item-build
// route hook, which fires from parallel sim workers. Each uncached call was two
// Win32 calls plus a 64 KB stack buffer to zero.
pub(crate) fn dll_dir() -> Option<PathBuf> {
    static CACHED: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    CACHED.get_or_init(resolve_dll_dir).clone()
}

fn resolve_dll_dir() -> Option<PathBuf> {
    extern "system" {
        fn GetModuleHandleExW(flags: u32, name: *const u16, module: *mut *mut c_void) -> i32;
        fn GetModuleFileNameW(module: *mut c_void, filename: *mut u16, size: u32) -> u32;
    }

    #[used]
    static ANCHOR: u8 = 0;
    let mut hmodule: *mut c_void = std::ptr::null_mut();
    // Flags 0x6 = FROM_ADDRESS | UNCHANGED_REFCOUNT
    let ok = unsafe { GetModuleHandleExW(0x6, &ANCHOR as *const u8 as *const u16, &mut hmodule) };
    if ok == 0 {
        return None;
    }

    let mut buf = [0u16; 32768];
    let len = unsafe { GetModuleFileNameW(hmodule, buf.as_mut_ptr(), buf.len() as u32) };
    if len == 0 {
        return None;
    }

    let os_str = OsString::from_wide(&buf[..len as usize]);
    std::path::Path::new(&os_str)
        .parent()
        .map(|p| p.to_path_buf())
}
