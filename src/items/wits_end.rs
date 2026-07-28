use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, try_proc_on_hit, ItemMeta};

fn apply_fray(
    ctx: &mut GameCtx,
    caster: usize,
    target: usize,
    magic_damage: usize,
    cooldown_seconds: f64,
    cooldown_buff: &str,
) {
    let is_tower = ctx.get_entity(target).map(|t| t.is_tower()).unwrap_or(true);
    if is_tower {
        return;
    }
    if !try_proc_on_hit(ctx, target, cooldown_buff, cooldown_seconds) {
        return;
    }
    ctx.deal_damage(caster, target, 0, magic_damage, AttackType::BaseAttack);
}

#[derive(Clone, Debug)]
pub struct WitsEnd {
    meta: ItemMeta,
    price: usize,
    attack_speed_mult: i32,
    magic_resistance: i32,
    toughness: usize,
    effect_bonus_magic_damage: usize,
    on_hit_cooldown_seconds: f64,
}

impl WitsEnd {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("wits_end", &["scouts_slingshot"], &["radiant_wits_end"]),
            price: 1400,
            attack_speed_mult: 40,
            magic_resistance: 80,
            toughness: 20,
            effect_bonus_magic_damage: 45,
            on_hit_cooldown_seconds: 0.5,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_wits_end", &["wits_end"]),
            price: 2000,
            attack_speed_mult: 65,
            magic_resistance: 130,
            toughness: 30,
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
                attack_speed_mult,
                magic_resistance,
                toughness,
                effect_bonus_magic_damage,
                on_hit_cooldown_seconds
            ]
        );
        self
    }
}

impl Default for WitsEnd {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for WitsEnd {
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
            attack_speed_mult: self.attack_speed_mult,
            magic_resistance: self.magic_resistance,
            toughness: self.toughness,
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
        apply_fray(
            ctx,
            caster,
            target,
            self.effect_bonus_magic_damage,
            self.on_hit_cooldown_seconds,
            "wits_end_on_hit_cooldown",
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AS, ItemTag::MagicResistance, ItemTag::Toughness]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}
