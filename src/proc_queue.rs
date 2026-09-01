//! Delayed, staggered delivery of on-hit item damage.
//!
//! An item that deals its damage straight out of `on_attack` resolves in the
//! same instant as the attack carrying it, so the two read as one number on
//! screen. [`ProcQueue`] holds the damage for [`PROC_DELAY_SECONDS`] instead and
//! lands it from `update`, and it spaces procs that would otherwise land on the
//! same tick one tick apart — see [`reserve_landing`].

use mod_api_stable::*;
use std::cell::Cell;

use crate::{ticks, PROC_DELAY_SECONDS, PROC_STAGGER_MAX_TICKS};

/// Damage that has been dealt but has not landed yet.
#[derive(Clone, Copy, Debug)]
struct PendingProc {
    /// Ticks left before it lands.
    remaining: usize,
    target: usize,
    physical: usize,
    magic: usize,
}

/// The last landing tick handed out by [`reserve_landing`], and who it was
/// against.
#[derive(Clone, Copy)]
struct ProcSlot {
    target: usize,
    landing_tick: usize,
}

thread_local! {
    /// Thread-local because parallel match simulations resolve attacks
    /// concurrently. The items of a single attack always run consecutively on
    /// one thread, so this needs no lock — and a proc that does cross threads
    /// simply lands on the undelayed schedule, which is what it did before.
    static LAST_PROC: Cell<Option<ProcSlot>> = Cell::new(None);
}

/// Ticks from now until `target`'s next free damage slot.
///
/// Every on-hit item in a build procs off the same attack, so a carrier holding
/// three of them puts three numbers on one target in one tick, which the game
/// draws on top of each other. Each proc therefore reserves its landing tick
/// here: the first off an attack lands after the full [`PROC_DELAY_SECONDS`],
/// and each further proc against the same target lands one tick after the one
/// before it, so they tick up the screen as separate numbers.
///
/// The reservation is only pushed back by procs still *ahead* of the natural
/// delay, so the spacing resets between attacks rather than drifting later and
/// later over a fight. [`PROC_STAGGER_MAX_TICKS`] caps how far a single attack
/// can push, so an implausible pile-up collides on screen rather than landing
/// visibly late.
fn reserve_landing(ctx: &mut StableSim<'_>, target: usize) -> usize {
    let now = ctx.tick();
    let earliest = now + ticks(PROC_DELAY_SECONDS);

    let landing = LAST_PROC.with(|cell| {
        let landing = match cell.get() {
            Some(prev) if prev.target == target && prev.landing_tick >= earliest => {
                (prev.landing_tick + 1).min(earliest + PROC_STAGGER_MAX_TICKS)
            }
            _ => earliest,
        };
        cell.set(Some(ProcSlot {
            target,
            landing_tick: landing,
        }));
        landing
    });

    (landing - now).max(1)
}

/// One item's queue of procs waiting out their delay. Held on the item itself,
/// so it is per carrier and per item the way the effect is.
#[derive(Clone, Debug, Default)]
pub(crate) struct ProcQueue {
    pending: Vec<PendingProc>,
}

impl ProcQueue {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Queues `physical` + `magic` damage against `target`. Both are the values
    /// the hit earned at the moment it landed: anything that has to be read off
    /// the target (its current health, its armor) must be resolved by the caller
    /// now, not when the damage arrives.
    pub(crate) fn push(
        &mut self,
        ctx: &mut StableSim<'_>,
        target: usize,
        physical: usize,
        magic: usize,
    ) {
        if physical == 0 && magic == 0 {
            return;
        }
        let remaining = reserve_landing(ctx, target);
        self.pending.push(PendingProc {
            remaining,
            target,
            physical,
            magic,
        });
    }

    pub(crate) fn push_physical(
        &mut self,
        ctx: &mut StableSim<'_>,
        target: usize,
        physical: usize,
    ) {
        self.push(ctx, target, physical, 0);
    }

    pub(crate) fn push_magic(&mut self, ctx: &mut StableSim<'_>, target: usize, magic: usize) {
        self.push(ctx, target, 0, magic);
    }

    /// Drops everything still in flight. Call from `on_spawn` so a proc queued
    /// in the last fight cannot land in the next one.
    pub(crate) fn clear(&mut self) {
        self.pending.clear();
    }

    /// Lands the procs whose delay has run out. Call once per `update`.
    pub(crate) fn update(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        if self.pending.is_empty() {
            return;
        }
        let Some(caster) = ctx
            .get_player(player)
            .and_then(|player_ref| player_ref.champion())
            .map(|champion| champion.id())
        else {
            return;
        };

        // Collected first: the drain cannot deal damage while it still holds the
        // list, and `ctx` is needed for both.
        let mut landed = Vec::new();
        self.pending.retain_mut(|hit| {
            hit.remaining = hit.remaining.saturating_sub(1);
            if hit.remaining > 0 {
                return true;
            }
            landed.push(*hit);
            false
        });

        for hit in landed {
            // The target can die inside the delay, and damage dealt to a corpse
            // still lands in the damage statistics.
            if ctx
                .get_entity(hit.target)
                .is_some_and(|target_ref| target_ref.is_alive())
            {
                ctx.deal_damage(caster, hit.target, hit.physical, hit.magic, AttackTypeV1::Item);
            }
        }
    }
}
