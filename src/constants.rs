pub(crate) const DISTANCE_UNITS_PER_RANGE: usize = 1000;

// Attacks landed from within this many range units count as melee. It sits in
// the empty band between the champion sheet's two attack-range clusters --
// melee tops out at 30 and ranged starts at 40 -- so no champion falls on the
// boundary. See `is_melee`.
pub(crate) const MELEE_DISTANCE: usize = 35;

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
