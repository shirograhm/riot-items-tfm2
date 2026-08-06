use mod_api_stable::*;

use crate::{apply_config, config::ItemConfig, percent_of_i32, ItemMeta};

#[derive(Clone, Debug)]
pub struct SunderedSky {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    attack: i32,
    skill_cooldown_mult: i32,
    effect_percent_bonus_damage: f64,
    effect_bonus_flat_heal: i32,
    effect_caster_hp_percent_heal: f64,
    on_hit_cooldown_seconds: f64,
}

impl SunderedSky {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("sundered_sky", &["phage"], &["radiant_sundered_sky"]),
            price: 1400,
            hp: 400,
            attack: 30,
            skill_cooldown_mult: 10,
            effect_percent_bonus_damage: 20.0,
            effect_bonus_flat_heal: 60,
            effect_caster_hp_percent_heal: 6.0,
            on_hit_cooldown_seconds: 20.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_sundered_sky", &["sundered_sky"]),
            price: 2000,
            hp: 550,
            attack: 65,
            skill_cooldown_mult: 20,
            effect_caster_hp_percent_heal: 10.0,
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
                skill_cooldown_mult,
                effect_percent_bonus_damage,
                effect_bonus_flat_heal,
                effect_caster_hp_percent_heal,
                on_hit_cooldown_seconds
            ]
        );
        self
    }
}

impl Default for SunderedSky {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for SunderedSky {
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
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

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
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() {
            return;
        }
        // Only trigger on base attacks
        if attack_type != AttackTypeV1::BaseAttack {
            return;
        }

        let missing_health = (caster_ref.hp().1 - caster_ref.hp().0) as i32;
        let heal_amount = self.effect_bonus_flat_heal
            + percent_of_i32(missing_health, self.effect_caster_hp_percent_heal);

        let ratio = 1.0 + (self.effect_percent_bonus_damage / 100.0);
        *damage = (*damage as f64 * ratio) as usize;
        ctx.heal(caster, caster, heal_amount as usize);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::Ad, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}
