use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, ItemMeta};

#[derive(Clone, Debug)]
pub struct KrakenSlayer {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
    move_speed_mult: i32,
    effect_bonus_flat_damage: usize,
    effect_max_percent_bonus: f64,
    effect_hp_percent_threshold: f64,
    effect_attack_interval: usize,
    attack_count: usize,
}

fn bring_it_down_damage(
    ctx: &mut StableSim<'_>,
    target: usize,
    flat: usize,
    max_percent_bonus: f64,
    hp_percent_threshold: f64,
) -> usize {
    let Some(target_ref) = ctx.get_entity(target) else {
        return 0;
    };
    if target_ref.is_tower() {
        return 0;
    }

    let (hp_current, hp_max) = target_ref.hp();
    if hp_max == 0 {
        return flat;
    }
    let hp_ratio = (hp_current as f64 / hp_max as f64).clamp(0.0, 1.0);
    let threshold = (hp_percent_threshold / 100.0).clamp(0.0, 1.0);
    let ratio = if threshold >= 1.0 {
        1.0
    } else {
        ((1.0 - hp_ratio) / (1.0 - threshold)).clamp(0.0, 1.0)
    };
    let scaling = 1.0 + (max_percent_bonus / 100.0) * ratio;
    (flat as f64 * scaling).round() as usize
}

impl KrakenSlayer {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "kraken_slayer",
                &["scouts_slingshot"],
                &["radiant_kraken_slayer"],
            ),
            price: 1400,
            attack: 45,
            attack_speed_mult: 25,
            move_speed_mult: 4,
            effect_bonus_flat_damage: 150,
            effect_max_percent_bonus: 75.0,
            effect_hp_percent_threshold: 25.0,
            effect_attack_interval: 3,
            attack_count: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_kraken_slayer", &["kraken_slayer"]),
            price: 2000,
            attack: 75,
            attack_speed_mult: 45,
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
                attack,
                attack_speed_mult,
                move_speed_mult,
                effect_bonus_flat_damage,
                effect_max_percent_bonus,
                effect_hp_percent_threshold,
                effect_attack_interval,
            ]
        );
        self
    }
}

impl Default for KrakenSlayer {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for KrakenSlayer {
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
            attack: self.attack,
            attack_speed_mult: self.attack_speed_mult,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.attack_count = 0;
    }

    fn on_base_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        caster: usize,
        target: usize,
    ) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        if self.attack_count == self.effect_attack_interval - 1 {
            let final_damage = bring_it_down_damage(
                ctx,
                target,
                self.effect_bonus_flat_damage,
                self.effect_max_percent_bonus,
                self.effect_hp_percent_threshold,
            );
            ctx.deal_damage(caster, target, final_damage, 0, AttackTypeV1::Item);

            self.attack_count = 0;
        } else {
            self.attack_count += 1;
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::AttackSpeed, ItemTagV1::MoveSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::AttackSpeed
    }
}
