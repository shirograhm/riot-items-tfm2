//! Constants shared across item modules.

/// Config `effect_max_distance` values are expressed in attack-range units;
/// multiply by this to convert to the raw game distance units that `distance_sq`
/// works in. Shared by range-based items (diamond_tipped_spear, zekes_herald).
pub(crate) const DISTANCE_UNITS_PER_RANGE: usize = 1000;

/// Each point of Adaptive Force grants 1 Ability Power, or this much Attack
/// Damage, whichever the recipient favors.
pub(crate) const ADAPTIVE_FORCE_AD_RATIO: f64 = 0.6;

/// A stat bonus that must track a changing value up AND down is granted as a
/// fixed-duration `Time` buff and re-applied on a slightly shorter cycle than it
/// lasts, so a fresh buff is always in place before the old one expires (no
/// single-tick gap where the bonus would flicker off). This is that buff's
/// duration: 1 second. Recompute the amount each cycle. Used by protectors_vow,
/// overlords_bloodmail, zekes_herald, and warmogs_armor.
pub(crate) const BUFF_REFRESH_DURATION_TICKS: usize = 60;

/// Re-apply period for `BUFF_REFRESH_DURATION_TICKS` buffs: ~2 ticks shorter than
/// the duration, so during the brief overlap both buffs are live and the bonus
/// momentarily doubles -- harmless, since it only ever overshoots and never dips.
pub(crate) const BUFF_REFRESH_PERIOD_TICKS: usize = 58;
