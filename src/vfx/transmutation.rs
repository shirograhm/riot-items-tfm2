use mod_api_stable::*;

use crate::{is_monster, percent_of, refresh_buff, ticks, DOT_TICK_RATE, TICKS_PER_SECOND};

// Transmutation, Philosopher's Medallion's and Philosopher's Stone's: killing a
// monster grants Ability Haste and bonus health regeneration for a while, and a
// monster that damages the carrier burns for a share of its maximum health as
// true damage over a few seconds, capped per second. One buff name for every
// tier, so the Stone's takes over from the Medallion's instead of adding up.
const TRANSMUTATION_BUFF: &str = "philosophers_transmutation";

// The numbers, from the item that holds it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TransmutationNumbers {
    pub(crate) haste: i32,
    pub(crate) hp_regen: i32,
    pub(crate) seconds: f64,
    pub(crate) burn_percent: f64,
    pub(crate) burn_seconds: f64,
    pub(crate) burn_cap_per_second: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Transmutation {
    // Monsters burning now, as `(entity, ticks left, ticks until the next tick,
    // damage a tick)`.
    burns: Vec<(usize, usize, usize, usize)>,
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
                skill_cooldown_mult: numbers.haste,
                hp_regen: numbers.hp_regen,
                ..BuffV1::timed(TRANSMUTATION_BUFF, ticks(numbers.seconds))
            },
        );
    }

    // The burn, on a monster that damaged the carrier: started, or started
    // over. A tick is an even share of the whole burn, held to the cap's share
    // of a second.
    pub(crate) fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        attacker: usize,
        damage: usize,
        numbers: &TransmutationNumbers,
    ) {
        if damage == 0 {
            return;
        }
        let Some(max_hp) = ctx
            .get_entity(attacker)
            .filter(|attacker_ref| attacker_ref.is_alive() && is_monster(attacker_ref))
            .map(|attacker_ref| attacker_ref.hp().1)
        else {
            return;
        };
        let duration = ticks(numbers.burn_seconds).max(DOT_TICK_RATE);
        let instances = (duration / DOT_TICK_RATE).max(1);
        let share = percent_of(max_hp, numbers.burn_percent) as f64 / instances as f64;
        let cap = numbers.burn_cap_per_second * DOT_TICK_RATE / TICKS_PER_SECOND as usize;
        let per_tick = (share.round() as usize).min(cap);
        match self.burns.iter_mut().find(|burn| burn.0 == attacker) {
            Some(burn) => {
                burn.1 = duration;
                burn.3 = per_tick;
            }
            None => self.burns.push((attacker, duration, DOT_TICK_RATE, per_tick)),
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
        for (id, remaining, until_next, damage) in std::mem::take(&mut self.burns) {
            let remaining = remaining.saturating_sub(1);
            let mut until_next = until_next.saturating_sub(1);
            if until_next == 0 {
                // Re-read every tick, since entity ids are recycled.
                if !ctx
                    .get_entity(id)
                    .is_some_and(|monster| monster.is_alive() && is_monster(&monster))
                {
                    continue;
                }
                if damage > 0 {
                    ctx.deal_damage_typed(
                        caster,
                        id,
                        damage,
                        DamageTypeV1::Fixed,
                        AttackTypeV1::Item,
                    );
                }
                until_next = DOT_TICK_RATE;
            }
            if remaining > 0 {
                kept.push((id, remaining, until_next, damage));
            }
        }
        self.burns = kept;
    }

    // Drops every burn: the carrier respawned (no `update` runs while dead).
    pub(crate) fn clear(&mut self) {
        self.burns.clear();
    }
}
