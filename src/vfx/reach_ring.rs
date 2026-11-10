//! Rings drawn on the ground at a champion's attack reach while an item
//! lengthens it (Rapid Firecannon's Sharpshooter, Hexoptics C44's Arcane Aim).
//!
//! The SDK cannot read a champion's attack range and a ring's art is fixed, so
//! each item binds one `view_buffs` ring per reach in [`RING_REACHES`] and puts
//! up the buff for the reach nearest the champion's own plus the bonus. The
//! same buff carries the range, so the ring is up exactly while the range is.

use mod_api_stable::*;

use crate::DISTANCE_UNITS_PER_RANGE;

// The reaches, in range, every ring family has a sheet for.
pub(crate) const RING_REACHES: [usize; 8] = [40, 50, 60, 70, 80, 90, 100, 110];
// A champion whose reach nothing states is taken for a ranged one.
const DEFAULT_REACH: usize = 60;

// Of an item's ring buffs (one per [`RING_REACHES`] entry), the one for
// `champion`'s reach with `bonus` more range.
pub(crate) fn ring_for(
    ctx: &StableSim<'_>,
    champion: usize,
    bonus: usize,
    rings: &[&'static str; RING_REACHES.len()],
) -> &'static str {
    let reach = ctx
        .get_entity(champion)
        .and_then(|c| c.name())
        .and_then(|key| crate::champion_traits::traits(&key)?.attack_range)
        .map_or(DEFAULT_REACH, |range| range / DISTANCE_UNITS_PER_RANGE)
        + bonus;
    let index = (0..RING_REACHES.len())
        .min_by_key(|&index| RING_REACHES[index].abs_diff(reach))
        .unwrap_or(0);
    rings[index]
}

// Takes every ring of the family off `champion`, so a ring of another size
// (left by another tier, say) never shows beside the new one.
pub(crate) fn clear_rings(
    ctx: &mut StableSim<'_>,
    champion: usize,
    rings: &[&'static str; RING_REACHES.len()],
) {
    for name in rings {
        ctx.entity_remove_buff(champion, name);
    }
}

// The timed buff `name` with `bonus` more attack range.
pub(crate) fn range_buff(name: &str, bonus: usize, ticks: usize) -> BuffV1 {
    let mut buff = BuffV1::timed(name, ticks);
    buff.range = bonus * DISTANCE_UNITS_PER_RANGE;
    buff
}
