use mod_api_stable::*;

pub(crate) fn refresh_buff(ctx: &mut StableSim<'_>, entity: usize, name: &str, buff: &BuffV1) {
    ctx.entity_remove_buff(entity, name);
    ctx.add_buff(entity, buff);
}

/// Statless marker on a champion whose Immolate is burning, shared by every
/// Immolate the mod runs (Bami's Cinder, Hollow Radiance, Sunfire Cape). It is
/// the `view_buffs` binding in `view/effects.view_effects` that draws flames at
/// the champion's feet.
const IMMOLATE_BUFF: &str = "riot_immolate";
/// Refreshed once a second as the burn ticks, so it never lapses between burns,
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
