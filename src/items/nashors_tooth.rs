use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, try_proc_on_hit, ItemMeta};

#[derive(Clone, Debug)]
pub struct NashorsTooth {
    meta: ItemMeta,
    price: usize,
    magic_power: i32,
    attack_speed_mult: i32,
    effect_bonus_flat_damage: usize,
    effect_ap_percent_damage: f64,
    on_hit_cooldown_seconds: f64,
}

impl NashorsTooth {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "nashors_tooth",
                &["needlessly_large_rod", "wind_dagger"],
                &["radiant_nashors_tooth"],
            ),
            price: 1450,
            magic_power: 115,
            attack_speed_mult: 25,
            effect_bonus_flat_damage: 35,
            effect_ap_percent_damage: 3.0,
            on_hit_cooldown_seconds: 0.75,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_nashors_tooth", &["nashors_tooth"]),
            price: 2050,
            magic_power: 180,
            attack_speed_mult: 40,
            effect_bonus_flat_damage: 50,
            effect_ap_percent_damage: 5.0,
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
                attack_speed_mult,
                effect_bonus_flat_damage,
                effect_ap_percent_damage,
                on_hit_cooldown_seconds
            ]
        );
        self
    }
}

impl Default for NashorsTooth {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for NashorsTooth {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        self.meta.key
    }

    fn icon(&self) -> &str {
        self.meta.key
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

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
            attack_speed_mult: self.attack_speed_mult,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        let bonus_damage = self.effect_bonus_flat_damage
            + percent_of(caster_ref.stat().magic_power, self.effect_ap_percent_damage);
        if try_proc_on_hit(
            ctx,
            target,
            "nashors_tooth_on_hit_cooldown",
            self.on_hit_cooldown_seconds,
        ) {
            ctx.deal_damage(caster, target, 0, bonus_damage, AttackType::Item);
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}
