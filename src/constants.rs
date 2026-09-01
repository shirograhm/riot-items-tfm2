//! Constants shared across item modules.

/// Config `effect_max_distance` values are expressed in attack-range units;
/// multiply by this to convert to the raw game distance units that `distance_sq`
/// works in.
pub(crate) const DISTANCE_UNITS_PER_RANGE: usize = 1000;

/// Each point of Adaptive Force grants 1 Ability Power, or this much Attack
/// Damage, whichever the recipient favors.
pub(crate) const ADAPTIVE_FORCE_AD_RATIO: f64 = 0.6;

/// The simulation runs at a fixed 60 ticks per second. Buff durations are
/// expressed in ticks, config durations in seconds; [`crate::ticks`] converts.
pub(crate) const TICKS_PER_SECOND: f64 = 60.0;

/// A stat bonus that must track a changing value up AND down is granted as a
/// fixed-duration `Time` buff and re-applied on a slightly shorter cycle than it
/// lasts, so a fresh buff is always in place before the old one expires. The
/// re-application is unconditional: the 2-tick overlap briefly doubles the bonus,
/// which is the price of never having a gap.
pub(crate) const BUFF_REFRESH_DURATION_TICKS: usize = 60;
pub(crate) const BUFF_REFRESH_PERIOD_TICKS: usize = 58;

/// How long an on-hit effect waits before its own damage or payout lands.
///
/// An effect resolved straight out of `on_attack` or `on_kill` happens in the
/// same instant as the event carrying it, so the two read as one number on
/// screen instead of the item's proc reading as its own. A fifth of a second is
/// enough to separate them without the proc feeling detached from the hit.
pub(crate) const PROC_DELAY_SECONDS: f64 = 0.2;

/// Ticks between the instances of a damage-over-time effect — five a second at
/// [`TICKS_PER_SECOND`].
///
/// A DoT is modelled as a fixed number of instances rather than a per-tick
/// trickle, so this also sets how finely a duration divides: the instance count
/// is the duration over this, and a duration shorter than one interval still
/// lands one instance.
pub(crate) const DOT_TICK_RATE: usize = 12;

/// Timings for an *aura* buff — one an item grants to other entities based on
/// where they are standing, rather than to its own carrier.
///
/// An aura cannot use the overlap above: a doubled bonus on someone else is
/// visible (and for a debuff, wrong), so an aura refresh removes the previous
/// instance before adding the new one, both within the same tick. That makes the
/// cycle safe to run well inside the duration instead of right at its edge.
///
/// Refreshing at a third of the duration leaves the buff two full refreshes of
/// headroom, so it never lapses while its target is still in range. Do NOT
/// re-derive this as "skip targets that already have the buff": that gate can
/// only replace the buff once it has already expired, which leaves the target
/// unaffected for most of every cycle — the visible flicker fixed on 2026-08-03.
///
/// The duration also sets how long an aura lingers after its cause goes away:
/// the target walking out of range, or the carrier dying.
pub(crate) const AURA_DURATION_TICKS: usize = 60;
pub(crate) const AURA_REFRESH_TICKS: usize = 20;
