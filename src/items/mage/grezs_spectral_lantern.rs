use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, is_monster, percent_of, ItemMeta, ProcQueue};

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
    accumulated_stacks: usize,
    procs: ProcQueue,
}

impl GrezsSpectralLantern {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "grezs_spectral_lantern",
                &["haunting_guise"],
                &["radiant_grezs_spectral_lantern"],
            ),
            spirit_drain_buff: "grezs_spectral_lantern_spirit_drain",
            price: 1400,
            hp: 250,
            magic_power: 60,
            skill_cooldown_mult: 10,
            effect_stack_magic_power: 2,
            effect_max_stacks: 20,
            effect_percent_bonus_damage: 20.0,
            effect_bonus_hp_percent_of_damage: 4.0,
            // Non-vital stats (internals)
            accumulated_stacks: 0,
            procs: ProcQueue::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant(
                "radiant_grezs_spectral_lantern",
                &["grezs_spectral_lantern"],
            ),
            price: 2000,
            hp: 350,
            magic_power: 120,
            skill_cooldown_mult: 10,
            effect_stack_magic_power: 2,
            effect_max_stacks: 30,
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

    /// Spirit Drain: one permanent Ability Power step, capped. The buff carries
    /// the step rather than the running total because same-name buffs stack, so
    /// the champion ends up wearing one `spirit_drain` per takedown.
    fn drain(&mut self, ctx: &mut StableSim<'_>, entity: usize) {
        if self.accumulated_stacks >= self.effect_max_stacks {
            return;
        }
        self.accumulated_stacks += 1;
        ctx.add_buff(
            entity,
            &BuffV1 {
                magic_power: self.effect_stack_magic_power,
                ..BuffV1::named(self.spirit_drain_buff)
            },
        );
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
            ult_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    /// Spirit Drain is permanent, so the Ability Power earned so far is
    /// re-applied each spawn from the banked count.
    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        // Ahead of the early return below: a proc left over from the last
        // fight has to go whether or not any power has been drained yet.
        self.procs.clear();

        if self.accumulated_stacks == 0 {
            return;
        }
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };
        ctx.add_buff(
            champion_ref.id(),
            &BuffV1 {
                magic_power: self.accumulated_stacks as i32 * self.effect_stack_magic_power,
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

    /// Drained power is bought, not earned twice: the stacks banked on the base
    /// item survive the Radiant upgrade, clamped to the successor's own ceiling
    /// in case the config gives the two variants different caps.
    fn on_upgrade(&mut self, next_key: &str) -> u64 {
        if self.meta.upgrades_to(next_key) {
            self.accumulated_stacks as u64
        } else {
            0
        }
    }

    fn on_upgraded_from(&mut self, prev_key: &str, carry: u64) {
        if self.meta.upgrades_from(prev_key) {
            self.accumulated_stacks = (carry as usize).min(self.effect_max_stacks);
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::Ap, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
