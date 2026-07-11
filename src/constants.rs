//! Constants shared across item modules.

/// Config `effect_max_distance` values are expressed in attack-range units;
/// multiply by this to convert to the raw game distance units that `distance_sq`
/// works in.
pub(crate) const DISTANCE_UNITS_PER_RANGE: usize = 1000;

/// Each point of Adaptive Force grants 1 Ability Power, or this much Attack
/// Damage, whichever the recipient favors.
pub(crate) const ADAPTIVE_FORCE_AD_RATIO: f64 = 0.6;

/// A stat bonus that must track a changing value up AND down is granted as a
/// fixed-duration `Time` buff and re-applied on a slightly shorter cycle than it
/// lasts, so a fresh buff is always in place before the old one expires.
pub(crate) const BUFF_REFRESH_DURATION_TICKS: usize = 60;
pub(crate) const BUFF_REFRESH_PERIOD_TICKS: usize = 58;
