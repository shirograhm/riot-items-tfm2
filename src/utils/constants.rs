pub(crate) const DISTANCE_UNITS_PER_RANGE: usize = 1000;
pub(crate) const ADAPTIVE_FORCE_AD_RATIO: f64 = 0.6;
pub(crate) const TICKS_PER_SECOND: f64 = 60.0;

pub(crate) const BUFF_REFRESH_DURATION_TICKS: usize = 60;
pub(crate) const BUFF_REFRESH_PERIOD_TICKS: usize = 58;

// Collector gold proc delay
pub(crate) const PROC_DELAY_SECONDS: f64 = 0.15;

// Other procs delay after the hit in sequence
pub(crate) const PROC_STAGGER_STEP_TICKS: usize = 2;
pub(crate) const PROC_STAGGER_MAX_TICKS: usize = 12;

pub(crate) const DOT_TICK_RATE: usize = 12;

pub(crate) const AURA_DURATION_TICKS: usize = 60;
pub(crate) const AURA_REFRESH_TICKS: usize = 20;

// The unit Zz'Rot Portal summons. The name is what binds its art: the view
// draws an entity from `aseprite_resources/champions/<name>`, which
// `mod.override_info` maps to this mod's Voidspawn sheet and animations.
pub(crate) const VOIDSPAWN_UNIT: &str = "riot_voidspawn";
