use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, has_buff, percent_of, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct TrinityForce {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    attack: i32,
    attack_speed_mult: i32,
    skill_cooldown_mult: i32,
    effect_bonus_flat_damage: usize,
    effect_ad_percent_damage: f64,
    effect_cooldown_seconds: f64,
    spellblade_ready: bool,
}

impl TrinityForce {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "trinity_force",
                &["sheen", "phage"],
                &["radiant_trinity_force"],
            ),
            price: 1400,
            hp: 200,
            attack: 30,
            attack_speed_mult: 15,
            skill_cooldown_mult: 10,
            effect_bonus_flat_damage: 33,
            effect_ad_percent_damage: 33.0,
            effect_cooldown_seconds: 3.5,
            // Non-vital stats (internals)
            spellblade_ready: false,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_trinity_force", &["trinity_force"]),
            price: 2000,
            hp: 333,
            attack: 33,
            attack_speed_mult: 33,
            skill_cooldown_mult: 20,
            effect_bonus_flat_damage: 33,
            effect_ad_percent_damage: 33.0,
            effect_cooldown_seconds: 3.5,
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
                attack,
                attack_speed_mult,
                skill_cooldown_mult,
                effect_bonus_flat_damage,
                effect_ad_percent_damage,
                effect_cooldown_seconds
            ]
        );
        self
    }
}

impl Default for TrinityForce {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for TrinityForce {
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
            attack: self.attack,
            attack_speed_mult: self.attack_speed_mult,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.spellblade_ready = false;
    }

    fn on_skill_hit(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        caster: usize,
        target: usize,
        is_ally: bool,
    ) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() || is_ally {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let on_cooldown = has_buff(&caster_ref, "spellblade_cooldown");
        if !on_cooldown {
            self.spellblade_ready = true;
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if !self.spellblade_ready || attack_type != AttackTypeV1::BaseAttack {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let bonus_damage = self.effect_bonus_flat_damage
            + percent_of(caster_ref.stat().attack, self.effect_ad_percent_damage);

        ctx.deal_damage(caster, target, bonus_damage, 0, AttackTypeV1::Item);
        ctx.add_buff(
            caster,
            &BuffV1::timed("spellblade_cooldown", ticks(self.effect_cooldown_seconds)),
        );
        self.spellblade_ready = false;
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::Ad,
            ItemTagV1::AttackSpeed,
            ItemTagV1::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
