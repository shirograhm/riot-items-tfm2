//! Buff helpers every item uses, and the shared buff-driven effects: Immolate,
//! Annul, Spellblade, Energized, Eternity and Lifeline, plus Sunfire Cape's
//! Immolate in the match hook.

mod annul;
mod energized;
mod eternity;
mod lifeline;
mod spellblade;
pub(crate) mod sunfire;

use mod_api_stable::*;

pub(crate) use annul::Annul;
pub(crate) use energized::Energized;
pub(crate) use eternity::Eternity;
pub(crate) use lifeline::Lifeline;
pub(crate) use spellblade::{Spellblade, SpellbladeBonus};

pub(crate) fn refresh_buff(ctx: &mut StableSim<'_>, entity: usize, name: &str, buff: &BuffV1) {
    ctx.entity_remove_buff(entity, name);
    ctx.add_buff(entity, buff);
}

// Statless marker on a champion whose Immolate is burning (Bami's Cinder,
// Hollow Radiance, Sunfire Cape); its `view_buffs` binding draws flames at the
// champion's feet.
const IMMOLATE_BUFF: &str = "riot_immolate";
// Refreshed every second while the burn ticks, so it lapses half a second after
// the last one.
const IMMOLATE_MARKER_TICKS: usize = 90;

// Puts up or keeps up the Immolate flames on `entity`. Replacing keeps one
// instance however many Immolate items it holds, and does not restart the
// animation.
pub(crate) fn mark_immolate(ctx: &mut StableSim<'_>, entity: usize) {
    refresh_buff(
        ctx,
        entity,
        IMMOLATE_BUFF,
        &BuffV1::timed(IMMOLATE_BUFF, IMMOLATE_MARKER_TICKS),
    );
}

// How much bigger than usual `entity` is, in percent: the sum of its buffs'
// `radius_mult`, never below zero.
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

// A range around `carrier` in world units, stretched by the carrier's size (30%
// bigger reaches 30% further). It reads the carrier's buffs, so use it for
// occasional effects, not every tick.
pub(crate) fn sized_range(ctx: &StableSim<'_>, carrier: usize, range: usize) -> u64 {
    (range * crate::DISTANCE_UNITS_PER_RANGE) as u64 * (100 + size_percent(ctx, carrier)) / 100
}

// One second of Immolate from `caster`: enemies within `range` (stretched by
// size) take `champion_damage` if they are champions, else `other_damage`.
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

// [`immolate_burn`] over a band in world units: enemies past `beyond` and
// within `reach`. Towers and zero amounts are skipped, so a caller can hit only
// minions and monsters.
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

// Adds one stack of `buff` to `entity`, capped at `max`, refreshing every
// stack's duration; `buff` carries one stack's value. Stacks live on the entity
// so the engine shows the count and two holders share one pool; before ABI 8
// this falls back to one refreshed stack.
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