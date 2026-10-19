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

// Whether `target` is still alive. Entity ids are reused slot indices with no
// generation, so the queue re-checks every tick and drops a proc the moment its
// target dies, before a stranger takes the slot.
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

    // Queues a hit to land in a few ticks. Landing procs come back through
    // `on_attack` as `AttackTypeV1::Item`, so a caller that queues off an
    // `Item` hit cycles forever: gate on `attack_type` first (Serpent's Fang
    // flags its own procs instead).
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

    // Physical damage on-hit, added to the swing's own `damage` unless the
    // swing crit (the bonus must not share in it) or is of another damage type;
    // those are queued as a proc.
    pub(crate) fn on_hit_physical(
        &mut self,
        ctx: &mut StableSim<'_>,
        target: usize,
        damage: &mut usize,
        swing: DamageTypeV1,
        is_crit: bool,
        physical: usize,
    ) {
        if swing == DamageTypeV1::Ad && !is_crit {
            *damage += physical;
        } else {
            self.push_physical(ctx, target, physical);
        }
    }

    // Magic damage on-hit; see [`ProcQueue::on_hit_physical`].
    pub(crate) fn on_hit_magic(
        &mut self,
        ctx: &mut StableSim<'_>,
        target: usize,
        damage: &mut usize,
        swing: DamageTypeV1,
        is_crit: bool,
        magic: usize,
    ) {
        if swing == DamageTypeV1::Ap && !is_crit {
            *damage += magic;
        } else {
            self.push_magic(ctx, target, magic);
        }
    }

    pub(crate) fn clear(&mut self) {
        self.pending.clear();
    }

    // Lands the procs whose delay has run out. Call once per `update`.
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
