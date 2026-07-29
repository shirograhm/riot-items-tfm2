use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, try_proc_on_hit, ItemMeta};

fn bring_it_down_damage(
    ctx: &mut GameCtx,
    target: usize,
    flat: usize,
    max_percent_bonus: f64,
    hp_percent_threshold: f64,
) -> usize {
    let Some(target_ref) = ctx.get_entity(target) else {
        return 0;
    };
    let hp = target_ref.hp();
    if hp.max == 0 {
        return flat;
    }
    let hp_ratio = (hp.current as f64 / hp.max as f64).clamp(0.0, 1.0);
    let threshold = (hp_percent_threshold / 100.0).clamp(0.0, 1.0);
    let ratio = if threshold >= 1.0 {
        1.0
    } else {
        ((1.0 - hp_ratio) / (1.0 - threshold)).clamp(0.0, 1.0)
    };
    let scaling = 1.0 + (max_percent_bonus / 100.0) * ratio;
    (flat as f64 * scaling).round() as usize
}
fn tick_bring_it_down(
    ctx: &mut GameCtx,
    target: usize,
    attack_count: &mut usize,
    interval: usize,
    flat: usize,
    max_percent_bonus: f64,
    hp_percent_threshold: f64,
    cooldown_seconds: f64,
) -> usize {
    let is_tower = ctx.get_entity(target).map(|t| t.is_tower()).unwrap_or(true);
    if is_tower {
        return 0;
    }
    let interval = interval.max(1);
    // Bring It Down only pays out once per `interval` attacks, so the shared
    // on-hit cooldown is scaled up to cover the whole cycle.
    if !try_proc_on_hit(
        ctx,
        target,
        "kraken_slayer_on_hit_cooldown",
        cooldown_seconds * interval as f64,
    ) {
        return 0;
    }

    *attack_count += 1;
    if *attack_count < interval {
        return 0;
    }
    *attack_count = 0;
    bring_it_down_damage(ctx, target, flat, max_percent_bonus, hp_percent_threshold)
}

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
    on_hit_cooldown_seconds: f64,
    attack_count: usize,
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
            on_hit_cooldown_seconds: 0.75,
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
                on_hit_cooldown_seconds
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

impl ModItemInfo for KrakenSlayer {
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
            attack: self.attack,
            attack_speed_mult: self.attack_speed_mult,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut GameCtx, _player: usize) {
        self.attack_count = 0;
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let bonus = tick_bring_it_down(
            ctx,
            target,
            &mut self.attack_count,
            self.effect_attack_interval,
            self.effect_bonus_flat_damage,
            self.effect_max_percent_bonus,
            self.effect_hp_percent_threshold,
            self.on_hit_cooldown_seconds,
        );
        if bonus > 0 {
            ctx.deal_damage(caster, target, bonus, 0, AttackType::Item);
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AS, ItemTag::MoveSpeed]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}
