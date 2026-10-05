use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, is_monster, percent_of, upgrade_carry, ItemMeta, ProcQueue};

/// The upgrade line the drained power is noted under (`crate::upgrade_carry`),
/// so it follows the carrier into the Radiant item.
const BASE_KEY: &str = "grezs_spectral_lantern";

#[derive(Clone, Debug)]
pub struct GrezsSpectralLantern {
    meta: ItemMeta,
    spirit_drain_buff: &'static str,
    price: usize,
    hp: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_stack_magic_power: i32,
    effect_max_stacks: usize,
    effect_percent_bonus_damage: f64,
    effect_bonus_hp_percent_of_damage: f64,
    // Non-vital stats (internals)
    /// The Ability Power Spirit Drain has banked. Power and not a count of
    /// stacks, so what the base item drained carries into the Radiant one as
    /// it is, whatever each tier pays for a stack: the two pay the same by
    /// default (1), and a config that sets them apart would otherwise have
    /// the carried stacks paid for again at the Radiant rate.
    drained_power: usize,
    /// The power last noted for an upgrade to take over.
    noted_power: usize,
    /// Whether this instance has taken over the power of the item it replaced.
    inherited: bool,
    procs: ProcQueue,
}

impl GrezsSpectralLantern {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(BASE_KEY, &["spirit_stone"], &["radiant_grezs_spectral_lantern"]),
            spirit_drain_buff: "grezs_spectral_lantern_spirit_drain",
            price: 700,
            hp: 150,
            magic_power: 30,
            skill_cooldown_mult: 10,
            effect_stack_magic_power: 1,
            effect_max_stacks: 20,
            effect_percent_bonus_damage: 20.0,
            effect_bonus_hp_percent_of_damage: 4.0,
            // Non-vital stats (internals)
            drained_power: 0,
            noted_power: 0,
            inherited: false,
            procs: ProcQueue::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_grezs_spectral_lantern", &[BASE_KEY]),
            price: 1000,
            hp: 200,
            magic_power: 60,
            skill_cooldown_mult: 10,
            effect_stack_magic_power: 1,
            effect_max_stacks: 40,
            effect_percent_bonus_damage: 30.0,
            effect_bonus_hp_percent_of_damage: 6.0,
            ..Self::base()
        }
    }

    pub fn with_config(cfg: &ItemConfig) -> Self {
        Self::base().configured(cfg)
    }

    pub fn radiant_with_config(cfg: &ItemConfig) -> Self {
        Self::radiant().configured(cfg)
    }

    fn configured(mut self, cfg: &ItemConfig) -> Self {
        apply_config!(
            self,
            cfg,
            [
                price,
                hp,
                magic_power,
                skill_cooldown_mult,
                effect_stack_magic_power,
                effect_max_stacks,
                effect_percent_bonus_damage,
                effect_bonus_hp_percent_of_damage
            ]
        );
        self
    }

    /// The most Ability Power Spirit Drain banks: the tooltip's "up to".
    fn power_cap(&self) -> usize {
        self.effect_stack_magic_power.max(0) as usize * self.effect_max_stacks
    }

    /// Spirit Drain: one permanent Ability Power step, capped. The last step
    /// is cut to what the cap has left, so a total that a full step would
    /// carry past the cap ends on it instead (39 with a step of 2 goes to 40,
    /// not 41). The buff carries the step rather than the running total
    /// because same-name buffs stack, so the champion ends up wearing one
    /// `spirit_drain` per takedown.
    fn drain(&mut self, ctx: &mut StableSim<'_>, entity: usize) {
        let room = self.power_cap().saturating_sub(self.drained_power);
        let step = (self.effect_stack_magic_power.max(0) as usize).min(room);
        if step == 0 {
            return;
        }
        self.drained_power += step;
        ctx.add_buff(
            entity,
            &BuffV1 {
                magic_power: step as i32,
                ..BuffV1::named(self.spirit_drain_buff)
            },
        );
    }

    /// Keeps the drained power where an upgrade can find it. The Radiant item
    /// arrives as a fresh instance: it takes over the base item's power once,
    /// clamped to its own ceiling, and every instance notes its total when it
    /// grows. Only the number moves. The power drained this life is already on
    /// the champion as `spirit_drain` buffs, and `on_spawn` re-applies it from
    /// the total on the next respawn.
    fn carry_power(&mut self, ctx: &StableSim<'_>, player: usize) {
        if !std::mem::replace(&mut self.inherited, true) && self.meta.upgrades_from(BASE_KEY) {
            if let Some((_, power)) = upgrade_carry::latest(BASE_KEY, ctx, player) {
                let power = (power as usize).min(self.power_cap());
                self.drained_power = self.drained_power.max(power);
                self.noted_power = self.drained_power;
            }
        }
        if self.drained_power != self.noted_power {
            self.noted_power = self.drained_power;
            upgrade_carry::note(BASE_KEY, ctx, player, self.drained_power as u64);
        }
    }
}

impl Default for GrezsSpectralLantern {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for GrezsSpectralLantern {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        self.meta.key.to_string()
    }

    fn icon(&self) -> String {
        self.meta.key.to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        self.meta.tier
    }

    fn previous_tier(&self) -> Vec<String> {
        self.meta.previous_tier()
    }

    fn next_tier(&self) -> Vec<String> {
        self.meta.next_tier()
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            hp: self.hp,
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    /// Spirit Drain is permanent, so the Ability Power earned so far is
    /// re-applied each spawn from the banked total.
    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        // Ahead of the early return below: a proc left over from the last
        // fight has to go whether or not any power has been drained yet.
        self.procs.clear();
        self.carry_power(ctx, player);

        if self.drained_power == 0 {
            return;
        }
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };
        let champion_id = champion_ref.id();
        // Same-name buffs stack and this carries the whole banked total, so
        // the previous life's copies go before the new one lands. Without the
        // remove, every respawn added another full total on top of the ones
        // already worn and the cap stopped meaning anything.
        ctx.entity_remove_buff(champion_id, self.spirit_drain_buff);
        ctx.add_buff(
            champion_id,
            &BuffV1 {
                magic_power: self.drained_power as i32,
                ..BuffV1::named(self.spirit_drain_buff)
            },
        );
    }

    /// Butcher. `on_attack` covers auto-attacks and skills alike, which is what
    /// "your damage dealt" means here, and the heal is taken off the whole hit,
    /// bonus included, so the two halves of the passive read as one effect.
    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        // The bonus is dealt through the engine, so it comes back around as an
        // `Item` hit. Without this it would butcher itself, forever.
        if attack_type == AttackTypeV1::Item {
            return;
        }
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !is_monster(&target_ref) {
            return;
        }

        let bonus = percent_of(*damage, self.effect_percent_bonus_damage);
        let heal = percent_of(*damage + bonus, self.effect_bonus_hp_percent_of_damage);

        // The heal is taken off the whole hit and lands with it; only the
        // bonus damage waits, so it reads as its own number on the monster.
        self.procs.push_magic(ctx, target, bonus);
        ctx.heal(caster, caster, heal);
    }

    /// Lands the Butcher bonus whose delay has run out.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.carry_power(ctx, player);
        self.procs.update(ctx, player);
    }

    fn on_kill(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        _player: usize,
        entity: usize,
        victim: usize,
    ) {
        let Some(victim_ref) = ctx.get_entity(victim) else {
            return;
        };
        // Champion and monster kills drain; minions do not, which is what
        // `is_monster` rules out on top of champions and towers.
        if victim_ref.is_champion() || is_monster(&victim_ref) {
            self.drain(ctx, entity);
        }
    }

    fn on_assist(&mut self, ctx: &mut StableSim<'_>, _player: usize, entity: usize) {
        self.drain(ctx, entity);
    }

    /// Drained power is bought, not earned twice: what the base item banked
    /// survives the Radiant upgrade, clamped to the successor's own ceiling.
    ///
    /// The host of game 0.6.2 never calls these two (`crate::upgrade_carry`);
    /// `carry_power` does the carrying, and they stay for a host that does.
    fn on_upgrade(&mut self, next_key: &str) -> u64 {
        if self.meta.upgrades_to(next_key) {
            self.drained_power as u64
        } else {
            0
        }
    }

    fn on_upgraded_from(&mut self, prev_key: &str, carry: u64) {
        if self.meta.upgrades_from(prev_key) {
            self.drained_power = (carry as usize).min(self.power_cap());
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::Ap, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
