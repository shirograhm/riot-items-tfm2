use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, is_monster, percent_of, ItemMeta};

// Spirit Drain: Grants 2 Ability Power for each champion takedown, up to 60.
// Butcher: Deal 30% bonus damage against monsters and heal for 6% of the damage
// dealt.
#[derive(Clone, Debug)]
pub struct SpiritOfTheSpectralWraith {
    meta: ItemMeta,
    drain_buff: &'static str,
    price: usize,
    magic_power: i32,
    hp_regen: i32,
    skill_cooldown_mult: i32,
    effect_stack_magic_power: i32,
    effect_max_stacks: usize,
    effect_percent_bonus_damage: f64,
    effect_bonus_hp_percent_of_damage: f64,
    // Non-vital stats (internals)
    drain_stacks: usize,
}

impl SpiritOfTheSpectralWraith {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "spirit_of_the_spectral_wraith",
                &["grezs_spectral_lantern"],
                &["radiant_spirit_of_the_spectral_wraith"],
            ),
            drain_buff: "spirit_of_the_spectral_wraith_drain",
            price: 1400,
            magic_power: 110,
            hp_regen: 8,
            skill_cooldown_mult: 15,
            effect_stack_magic_power: 2,
            effect_max_stacks: 30,
            effect_percent_bonus_damage: 30.0,
            effect_bonus_hp_percent_of_damage: 6.0,
            drain_stacks: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant(
                "radiant_spirit_of_the_spectral_wraith",
                &["spirit_of_the_spectral_wraith"],
            ),
            // A distinct buff name so a base holder and a radiant holder keep
            // their own stacks, the way `night_harvester` separates its two
            // variants.
            drain_buff: "radiant_spirit_of_the_spectral_wraith_drain",
            price: 2000,
            magic_power: 175,
            hp_regen: 12,
            skill_cooldown_mult: 20,
            effect_percent_bonus_damage: 40.0,
            effect_bonus_hp_percent_of_damage: 8.0,
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
                magic_power,
                hp_regen,
                skill_cooldown_mult,
                effect_stack_magic_power,
                effect_max_stacks,
                effect_percent_bonus_damage,
                effect_bonus_hp_percent_of_damage
            ]
        );
        self
    }

    // Spirit Drain. A takedown is a kill or an assist, so both hooks feed this.
    // The stable API cannot tell an epic monster from an ordinary camp (see
    // `is_monster`), so only champion takedowns count.
    //
    // Same-name buffs stack rather than replace, so each takedown adds one more
    // instance worth `effect_stack_magic_power` and the cap is enforced by the
    // counter rather than by rewriting a single buff.
    fn add_stack(&mut self, ctx: &mut StableSim<'_>, entity: usize) {
        if self.drain_stacks >= self.effect_max_stacks {
            return;
        }
        self.drain_stacks += 1;
        ctx.add_buff(
            entity,
            &BuffV1 {
                magic_power: self.effect_stack_magic_power,
                ..BuffV1::named(self.drain_buff)
            },
        );
    }
}

impl Default for SpiritOfTheSpectralWraith {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for SpiritOfTheSpectralWraith {
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
            magic_power: self.magic_power,
            hp_regen: self.hp_regen,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    // Stacks are per-match, and the buffs they added outlive the match, so both
    // have to be cleared the way `hubris` clears its own.
    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        self.drain_stacks = 0;
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };
        let champion_id = champion_ref.id();
        ctx.entity_remove_buff(champion_id, self.drain_buff);
    }

    // Butcher. Only a basic attack's damage can be amplified — ability damage is
    // dealt by the game and never reaches a mod — so the bonus and the heal it
    // feeds both ride the auto-attack.
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
        if attack_type != AttackTypeV1::BaseAttack {
            return;
        }
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !is_monster(&target_ref) {
            return;
        }

        *damage += percent_of(*damage, self.effect_percent_bonus_damage);
        let heal = percent_of(*damage, self.effect_bonus_hp_percent_of_damage);
        ctx.heal(caster, caster, heal);
    }

    fn on_kill(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        _player: usize,
        entity: usize,
        victim: usize,
    ) {
        if ctx.get_entity(victim).is_some_and(|v| v.is_champion()) {
            self.add_stack(ctx, entity);
        }
    }

    fn on_assist(&mut self, ctx: &mut StableSim<'_>, _player: usize, entity: usize) {
        self.add_stack(ctx, entity);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Ap,
            ItemTagV1::HpRegen,
            ItemTagV1::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}
