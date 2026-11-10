use mod_api_stable::*;

use crate::{is_monster, percent_of, refresh_buff, ticks, DOT_TICK_RATE, TICKS_PER_SECOND};

// Transmutation, Philosopher's Medallion's and Philosopher's Stone's: killing a
// monster grants bonus health regeneration for a while, and a
// monster the carrier damages burns for a share of the carrier's maximum
// health as true damage over a few seconds, capped per second. One buff name for every
// tier, so the Stone's takes over from the Medallion's instead of adding up.
const TRANSMUTATION_BUFF: &str = "philosophers_transmutation";
// Statless marker on a burning monster: the `view_buffs` binding of the same
// name draws a small red-and-blue ring of fire on the ground at its feet,
// behind it (z -1). One per size, since monsters stand 23 to 106 px wide with
// their feet 18 (jungle camps), 37 (Serpen) or 55 (epic) px under the unit.
const BURN_RING: &str = "riot_transmutation_burn";
const SERPEN_BURN_RING: &str = "riot_transmutation_burn_serpen";
const EPIC_BURN_RING: &str = "riot_transmutation_burn_epic";
// How much longer than the burn the ring is put up for, and how long it is
// left before being put up again (every hit restarts the burn; replacing the
// buff that often is wasted work). Liandry's flames do the same.
const RING_SLACK_TICKS: usize = DOT_TICK_RATE;

// The numbers, from the item that holds it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TransmutationNumbers {
    pub(crate) hp_regen: i32,
    pub(crate) seconds: f64,
    pub(crate) burn_percent: f64,
    pub(crate) burn_seconds: f64,
    pub(crate) burn_cap_per_second: usize,
}

// One monster burning now.
#[derive(Clone, Copy, Debug)]
struct Burn {
    target: usize,
    // Ticks the burn has left.
    remaining: usize,
    // Ticks until its next damage tick.
    until_next: usize,
    // Damage a tick.
    damage: usize,
    // Ticks the ring's marker has left. Never less than `remaining`.
    marker: usize,
    // Which ring, by the monster's size. Kept here because the monster may
    // be gone by the time its ring is to come down.
    ring: &'static str,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Transmutation {
    burns: Vec<Burn>,
}

// The ring that fits this monster.
fn ring_for(monster: &StableEntity<'_, '_>) -> &'static str {
    match monster.kind() {
        Some(EntityKindV1::Epic) => EPIC_BURN_RING,
        Some(EntityKindV1::Serpen) => SERPEN_BURN_RING,
        _ => BURN_RING,
    }
}

impl Transmutation {
    // The buff, on a monster kill; another kill starts its time over.
    pub(crate) fn on_kill(
        &self,
        ctx: &mut StableSim<'_>,
        entity: usize,
        victim: usize,
        numbers: &TransmutationNumbers,
    ) {
        if !ctx
            .get_entity(victim)
            .is_some_and(|victim_ref| is_monster(&victim_ref))
        {
            return;
        }
        refresh_buff(
            ctx,
            entity,
            TRANSMUTATION_BUFF,
            &BuffV1 {
                hp_regen: numbers.hp_regen,
                ..BuffV1::timed(TRANSMUTATION_BUFF, ticks(numbers.seconds))
            },
        );
    }

    // The burn, on a monster the carrier damaged: started, or started over. It
    // is a share of the CARRIER's maximum health. A tick is an even share of
    // the whole burn, held to the cap's share of a second. Item damage (the
    // burn's own ticks among it) never relights it.
    pub(crate) fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: usize,
        attack_type: AttackTypeV1,
        numbers: &TransmutationNumbers,
    ) {
        if damage == 0 || attack_type == AttackTypeV1::Item {
            return;
        }
        let Some(ring) = ctx
            .get_entity(target)
            .filter(|target_ref| target_ref.is_alive() && is_monster(target_ref))
            .map(|target_ref| ring_for(&target_ref))
        else {
            return;
        };
        let Some(max_hp) = ctx.get_entity(caster).map(|caster_ref| caster_ref.hp().1) else {
            return;
        };
        let duration = ticks(numbers.burn_seconds).max(DOT_TICK_RATE);
        let instances = (duration / DOT_TICK_RATE).max(1);
        let share = percent_of(max_hp, numbers.burn_percent) as f64 / instances as f64;
        let cap = numbers.burn_cap_per_second * DOT_TICK_RATE / TICKS_PER_SECOND as usize;
        let per_tick = (share.round() as usize).min(cap);
        let index = match self.burns.iter().position(|burn| burn.target == target) {
            Some(index) => index,
            None => {
                self.burns.push(Burn {
                    target,
                    remaining: 0,
                    until_next: DOT_TICK_RATE,
                    damage: 0,
                    marker: 0,
                    ring,
                });
                self.burns.len() - 1
            }
        };
        let burn = &mut self.burns[index];
        burn.remaining = duration;
        burn.damage = per_tick;
        // Put up for the whole burn in one call, and again only once the burn
        // would outlast it.
        if burn.marker < duration {
            burn.marker = duration + RING_SLACK_TICKS;
            let marker = burn.marker;
            refresh_buff(ctx, target, ring, &BuffV1::timed(ring, marker));
        }
    }

    // Lands the burns' ticks, from the item's `update`. A dead carrier burns
    // nothing, and a list held for its next life would relight ids that may
    // belong to other units by then.
    pub(crate) fn update(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        if self.burns.is_empty() {
            return;
        }
        let Some(caster) = ctx
            .get_player(player)
            .and_then(|player_ref| player_ref.champion())
            .filter(|champion| champion.is_alive())
            .map(|champion| champion.id())
        else {
            self.burns.clear();
            return;
        };
        let mut kept = Vec::with_capacity(self.burns.len());
        for mut burn in std::mem::take(&mut self.burns) {
            burn.remaining = burn.remaining.saturating_sub(1);
            burn.until_next = burn.until_next.saturating_sub(1);
            burn.marker = burn.marker.saturating_sub(1);
            if burn.until_next == 0 {
                // Re-read every tick, since entity ids are recycled.
                if !ctx
                    .get_entity(burn.target)
                    .is_some_and(|monster| monster.is_alive() && is_monster(&monster))
                {
                    // Dead or gone, and its ring goes with the burn rather
                    // than sitting on the body or whatever takes its slot.
                    ctx.entity_remove_buff(burn.target, burn.ring);
                    continue;
                }
                if burn.damage > 0 {
                    ctx.deal_damage_typed(
                        caster,
                        burn.target,
                        burn.damage,
                        DamageTypeV1::Fixed,
                        AttackTypeV1::Item,
                    );
                }
                burn.until_next = DOT_TICK_RATE;
            }
            if burn.remaining > 0 {
                kept.push(burn);
            }
        }
        self.burns = kept;
    }

    // Drops every burn: the carrier respawned (no `update` runs while dead).
    pub(crate) fn clear(&mut self) {
        self.burns.clear();
    }
}
