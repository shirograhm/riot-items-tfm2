//! Energized: moving and basic attacking fill a meter, and the basic attack
//! that lands with it full spends it. Shared by Statikk Shiv, Stormrazor and
//! Voltaic Cyclosword; each item owns its numbers and what its charged hit
//! does.
//!
//! There is no hook for moving, so the meter gains a stack as time passes and
//! a few more on every basic attack. While the carrier is charged they hold
//! `CHARGED_BUFF`, which draws the lightning crackling round them.
//!
//! # One charge for the champion
//!
//! Every Energized item keeps its own meter, since an item cannot see into
//! another, and `CHARGED_BUFF` is what ties them together: an item whose meter
//! fills puts it up, and an item that finds it up counts as charged whatever
//! its own meter says. So a champion holding two of them is charged once, the
//! charged attack sets off both, both meters start over together, and from
//! then on they fill in step.
//!
//! A buff is read a few ticks late, both as it goes up and as it comes down.
//! So an item that has just spent a charge goes by its own meter alone for
//! `SHARE_LOCKOUT_TICKS`, which is shorter than the meter takes to fill.

use mod_api_stable::*;

use crate::{has_buff, refresh_buff};

// Statless marker on a charged champion. It is the `view_buffs` binding in
// `view/effects.view_effects` that draws the crackle
// (`effects/energized_charge`).
const CHARGED_BUFF: &str = "riot_energized";
// Refreshed every `MARK_EVERY_TICKS` while an item is charged, so it never
// lapses between refreshes, and gone within a second of the last one: the
// champion died, or the item left.
const CHARGED_TICKS: usize = 60;
const MARK_EVERY_TICKS: usize = 20;
// The meter gains one stack each time this many ticks have gone by since the
// last, which is a stack every 13 ticks.
const STACK_EVERY_TICKS: usize = 12;
// Stacks a basic attack adds.
const ATTACK_STACKS: usize = 5;
// How long an item that has spent a charge ignores `CHARGED_BUFF`. Filling a
// 100-stack meter takes well over five seconds at any attack speed.
const SHARE_LOCKOUT_TICKS: usize = 240;

#[derive(Clone, Debug, Default)]
pub(crate) struct Energized {
    stacks: usize,
    // Ticks since the meter last gained its stack for time passing.
    idle_ticks: usize,
    // This item has put `CHARGED_BUFF` up for the charge it holds now.
    marked: bool,
    // The tick this item last spent a charge.
    spent_at: Option<usize>,
    // A charge was spent since the last `update`, which takes the marker down.
    take_down: bool,
}

impl Energized {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn stacks(&self) -> usize {
        self.stacks
    }

    pub(crate) fn set_stacks(&mut self, stacks: usize) {
        self.stacks = stacks;
    }

    // Fills the meter as time passes and keeps the crackle up while it is
    // full. Call once per `update`, with the item's stack limit.
    pub(crate) fn update(&mut self, ctx: &mut StableSim<'_>, player: usize, max: usize) {
        if self.idle_ticks >= STACK_EVERY_TICKS {
            self.stacks = (self.stacks + 1).min(max);
            self.idle_ticks = 0;
        } else {
            self.idle_ticks += 1;
        }

        let Some(champion) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .filter(|c| c.is_alive())
            .map(|c| c.id())
        else {
            return;
        };
        // Not in `spend`: every Energized item the champion holds has to see
        // the marker during the attack that spends it.
        if self.take_down {
            self.take_down = false;
            ctx.entity_remove_buff(champion, CHARGED_BUFF);
        }
        if self.stacks >= max && (!self.marked || ctx.tick() % MARK_EVERY_TICKS == 0) {
            self.marked = true;
            refresh_buff(
                ctx,
                champion,
                CHARGED_BUFF,
                &BuffV1::timed(CHARGED_BUFF, CHARGED_TICKS),
            );
        }
    }

    // Whether `caster`'s next hit is a charged one: this item's meter is
    // full, or another Energized item's is.
    pub(crate) fn is_charged(&self, ctx: &StableSim<'_>, caster: usize, max: usize) -> bool {
        if self.stacks >= max {
            return true;
        }
        let rested = self
            .spent_at
            .is_none_or(|at| ctx.tick().saturating_sub(at) >= SHARE_LOCKOUT_TICKS);
        rested
            && ctx
                .get_entity(caster)
                .is_some_and(|caster_ref| has_buff(&caster_ref, CHARGED_BUFF))
    }

    // Spends the charge: the meter starts over and the crackle comes down on
    // the next `update`. For an Energized item's `on_attack`, alongside what
    // its charged hit does.
    pub(crate) fn spend(&mut self, ctx: &StableSim<'_>) {
        self.stacks = 0;
        self.marked = false;
        self.spent_at = Some(ctx.tick());
        self.take_down = true;
    }

    // A basic attack landed: the meter gains its stacks for it.
    pub(crate) fn basic_attack(&mut self, max: usize) {
        self.stacks = (self.stacks + ATTACK_STACKS).min(max);
    }
}
