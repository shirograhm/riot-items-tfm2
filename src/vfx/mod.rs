//! Buffs and the effects drawn from them: the buff helpers every item uses,
//! the Immolate flames marker and burn, the shared Annul spell shield (whose
//! buff draws the shield bubble), the shared Spellblade (whose buff draws the
//! sparks) and Sunfire Cape's Immolate, which runs as the match hook.

mod annul;
mod spellblade;
pub(crate) mod sunfire;

use mod_api_stable::*;

pub(crate) use annul::Annul;
pub(crate) use spellblade::Spellblade;

pub(crate) fn refresh_buff(ctx: &mut StableSim<'_>, entity: usize, name: &str, buff: &BuffV1) {
    ctx.entity_remove_buff(entity, name);
    ctx.add_buff(entity, buff);
}

/// Statless marker on a champion whose Immolate is burning, shared by every
/// Immolate the mod runs (Bami's Cinder, Hollow Radiance, Sunfire Cape). It is
/// the `view_buffs` binding in `view/effects.view_effects` that draws flames at
/// the champion's feet.
const IMMOLATE_BUFF: &str = "riot_immolate";
/// Refreshed once a second as the burn ticks, so it never lapses between burns,s
/// and goes half a second after the last one: death, or the item leaving.
const IMMOLATE_MARKER_TICKS: usize = 90;

/// Puts up (or keeps up) the Immolate flames on `entity`. Replacing rather than
/// adding keeps exactly one instance however many Immolate items it holds, and
/// the view does not restart an animation whose buff was replaced in one tick.
pub(crate) fn mark_immolate(ctx: &mut StableSim<'_>, entity: usize) {
    refresh_buff(
        ctx,
        entity,
        IMMOLATE_BUFF,
        &BuffV1::timed(IMMOLATE_BUFF, IMMOLATE_MARKER_TICKS),
    );
}

/// How much bigger than usual `entity` is, in percent: the `radius_mult` of
/// every buff on it, which is how anything in this game changes a champion's
/// size (Heartsteel's Goliath, another mod's Cho'Gath). Never below zero, so a
/// shrunken champion keeps the ranges its tooltips state.
pub(crate) fn size_percent(ctx: &StableSim<'_>, entity: usize) -> u64 {
    let Some(entity) = ctx.get_entity(entity) else {
        return 0;
    };
    let total: i32 = (0..entity.buff_count())
        .filter_map(|index| entity.buff_at(index))
        .map(|buff| buff.radius_mult)
        .sum();
    total.max(0) as u64
}

/// A range measured from `carrier`, in world units: `range` (the range units
/// tooltips use) stretched by the carrier's size, so a champion 30% bigger
/// reaches 30% further. Every effect that covers an area around its carrier
/// goes through this; one measured from somewhere else (a cleave around the
/// target, an eruption where a unit died) does not.
///
/// It reads the carrier's buffs, so it is for effects that fire now and then
/// (an aura refresh, a proc, a burn once a second), not for every tick.
pub(crate) fn sized_range(ctx: &StableSim<'_>, carrier: usize, range: usize) -> u64 {
    (range * crate::DISTANCE_UNITS_PER_RANGE) as u64 * (100 + size_percent(ctx, carrier)) / 100
}

/// One second of Immolate from `caster`: every enemy unit within `range`,
/// stretched by the caster's size ([`sized_range`]), takes `champion_damage` as
/// magic damage if it is a champion and `other_damage` otherwise (minions and
/// monsters).
pub(crate) fn immolate_burn(
    ctx: &mut StableSim<'_>,
    caster: usize,
    caster_team: usize,
    range: usize,
    champion_damage: usize,
    other_damage: usize,
) {
    let reach = sized_range(ctx, caster, range);
    immolate_burn_between(ctx, caster, caster_team, 0, reach, champion_damage, other_damage);
}

/// [`immolate_burn`] over an explicit band, in world units: enemies further
/// than `beyond` and no further than `reach` (everyone within `reach` when
/// `beyond` is 0). Towers are enemy entities too, and Immolate is not meant
/// for them. A zero amount is skipped, which lets a caller hit only the
/// minions and monsters.
pub(crate) fn immolate_burn_between(
    ctx: &mut StableSim<'_>,
    caster: usize,
    caster_team: usize,
    beyond: u64,
    reach: u64,
    champion_damage: usize,
    other_damage: usize,
) {
    let (beyond_sq, reach_sq) = (beyond * beyond, reach * reach);
    let targets: Vec<(usize, bool)> = (0..ctx.entity_count())
        .filter_map(|index| ctx.entity_at(index))
        .filter(|e| e.is_alive() && !e.is_tower() && e.team() != caster_team)
        .map(|e| (e.id(), e.is_champion()))
        .filter(|&(id, _)| {
            let distance_sq = ctx.distance_sq(caster, id);
            distance_sq <= reach_sq && (beyond == 0 || distance_sq > beyond_sq)
        })
        .collect();

    for (target, is_champion) in targets {
        let damage = if is_champion {
            champion_damage
        } else {
            other_damage
        };
        if damage > 0 {
            ctx.deal_damage(caster, target, 0, damage, AttackTypeV1::Item);
        }
    }
}

/// Adds one stack of `buff` to `entity`, capped at `max`, and restarts the
/// duration of every stack already there. `buff` carries the value of ONE
/// stack; the engine counts same-name buffs as stacks and sums them.
///
/// The stacks live on the entity, not on the item. This replaces an item-side
/// counter that fed one buff of `per_stack * count`, which in game (2026-09-19,
/// Black Cleaver) only ever showed a single stack. On the entity the engine
/// owns the count and the expiry, and two holders of the same item share one
/// stack pool on a target instead of overwriting each other's buff -- the way
/// LoL caps a shred.
///
/// Returns the stack count after adding; 0 when `max` is 0. On a host older
/// than ABI level 8 (no `entity_stack_buff`) it falls back to one refreshed
/// stack.
pub(crate) fn add_stack(ctx: &mut StableSim<'_>, entity: usize, buff: &BuffV1, max: usize) -> usize {
    if max == 0 {
        return 0;
    }
    match ctx.entity_stack_buff(entity, buff, max, true) {
        0 => {
            refresh_buff(ctx, entity, buff.name(), buff);
            1
        }
        count => count,
    }
}