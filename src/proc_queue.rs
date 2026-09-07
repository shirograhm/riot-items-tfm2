// Delayed delivery of on-hit item damage.
use mod_api_stable::*;
use std::cell::Cell;

use crate::{PROC_STAGGER_MAX_TICKS, PROC_STAGGER_STEP_TICKS};

#[derive(Clone, Copy, Debug)]
struct PendingProc {
    remaining: usize,
    target: usize,
    physical: usize,
    magic: usize,
}

#[derive(Clone, Copy)]
struct ProcSlot {
    target: usize,
    landing_tick: usize,
}

thread_local! {
    static LAST_PROC: Cell<Option<ProcSlot>> = Cell::new(None);
}

fn reserve_landing(ctx: &mut StableSim<'_>, target: usize) -> usize {
    let now = ctx.tick();
    let earliest = now + PROC_STAGGER_STEP_TICKS;

    let landing = LAST_PROC.with(|cell| {
        let landing = match cell.get() {
            Some(prev) if prev.target == target && prev.landing_tick >= earliest => {
                (prev.landing_tick + PROC_STAGGER_STEP_TICKS).min(earliest + PROC_STAGGER_MAX_TICKS)
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

/// Whether `target` is still the entity the proc was queued against.
///
/// Entity ids are bare slot indices -- `EntityHandleV1::from_id` is just
/// `id + 1`, with no generation counter -- so the id of something that died
/// is free to be handed to whatever spawns next. A proc that only checks
/// this at its landing tick can hit a stranger sitting in the dead target's
/// slot, which is why the queue re-checks every tick and drops the proc the
/// moment its target goes down.
fn target_still_valid(ctx: &StableSim<'_>, target: usize) -> bool {
    ctx.get_entity(target)
        .is_some_and(|target_ref| target_ref.is_alive())
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ProcQueue {
    pending: Vec<PendingProc>,
}

impl ProcQueue {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Queues a hit to land in a few ticks.
    ///
    /// Landing procs go out through `deal_damage` as `AttackTypeV1::Item`,
    /// so they come back around through `on_attack`. A caller that queues
    /// off an `Item` hit feeds its own procs and cycles forever -- the
    /// stagger cap only paces that loop, it does not end it. Gate on
    /// `attack_type` first: most items take `BaseAttack` only, and the ones
    /// that want skills too still have to reject `Item` explicitly.
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

        let mut landed = Vec::new();
        self.pending.retain_mut(|hit| {
            if !target_still_valid(ctx, hit.target) {
                return false;
            }
            hit.remaining = hit.remaining.saturating_sub(1);
            if hit.remaining > 0 {
                return true;
            }
            landed.push(*hit);
            false
        });

        for hit in landed {
            // Still worth re-testing: an earlier proc in this same batch can
            // have killed a target that a later one also points at.
            if target_still_valid(ctx, hit.target) {
                ctx.deal_damage(
                    caster,
                    hit.target,
                    hit.physical,
                    hit.magic,
                    AttackTypeV1::Item,
                );
            }
        }
    }
}
